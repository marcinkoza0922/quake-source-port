//! Packet transports: UDP (net_udp.c over std::net) and in-process loopback.

use std::collections::VecDeque;
use std::io;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::{Arc, Mutex};

/// Unconnected datagram transport.
pub trait Transport {
    fn send_to(&mut self, data: &[u8], addr: SocketAddr) -> io::Result<()>;
    /// Non-blocking receive; `Ok(None)` when nothing is pending.
    fn recv_from(&mut self, buf: &mut [u8]) -> io::Result<Option<(usize, SocketAddr)>>;
    fn local_addr(&self) -> SocketAddr;
}

/// Non-blocking UDP socket.
pub struct UdpTransport {
    sock: UdpSocket,
}

impl UdpTransport {
    /// Bind to `addr` (port 0 = any) and set non-blocking.
    pub fn bind(addr: SocketAddr) -> io::Result<Self> {
        let sock = UdpSocket::bind(addr)?;
        sock.set_nonblocking(true)?;
        Ok(Self { sock })
    }
    pub fn set_broadcast(&self, on: bool) -> io::Result<()> {
        self.sock.set_broadcast(on)
    }
}

impl Transport for UdpTransport {
    fn send_to(&mut self, data: &[u8], addr: SocketAddr) -> io::Result<()> {
        match self.sock.send_to(data, addr) {
            Ok(_) => Ok(()),
            // Like UDP_Write: a full socket buffer drops the packet.
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => Ok(()),
            Err(e) => Err(e),
        }
    }
    fn recv_from(&mut self, buf: &mut [u8]) -> io::Result<Option<(usize, SocketAddr)>> {
        loop {
            match self.sock.recv_from(buf) {
                Ok((n, a)) => return Ok(Some((n, a))),
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => return Ok(None),
                // Windows reports ICMP port unreachable here; ignore like net_udp.c.
                Err(e) if e.kind() == io::ErrorKind::ConnectionReset => continue,
                Err(e) => return Err(e),
            }
        }
    }
    fn local_addr(&self) -> SocketAddr {
        self.sock.local_addr().unwrap_or_else(|_| (Ipv4Addr::UNSPECIFIED, 0).into())
    }
}

type Queue = Arc<Mutex<VecDeque<Vec<u8>>>>;

/// One end of an in-process packet pipe. Lossless and ordered; each end
/// sees the other's fixed loopback address.
pub struct LoopbackTransport {
    me: SocketAddr,
    peer: SocketAddr,
    tx: Queue,
    rx: Queue,
}

impl LoopbackTransport {
    /// `(client, server)` ends.
    pub fn pair() -> (Self, Self) {
        let a: Queue = Arc::default();
        let b: Queue = Arc::default();
        let ca: SocketAddr = (Ipv4Addr::LOCALHOST, 1).into();
        let sa: SocketAddr = (Ipv4Addr::LOCALHOST, 2).into();
        (
            Self { me: ca, peer: sa, tx: a.clone(), rx: b.clone() },
            Self { me: sa, peer: ca, tx: b, rx: a },
        )
    }
    pub fn peer_addr(&self) -> SocketAddr {
        self.peer
    }
}

impl Transport for LoopbackTransport {
    fn send_to(&mut self, data: &[u8], addr: SocketAddr) -> io::Result<()> {
        if addr == self.peer {
            self.tx.lock().unwrap_or_else(|e| e.into_inner()).push_back(data.to_vec());
        }
        Ok(())
    }
    fn recv_from(&mut self, buf: &mut [u8]) -> io::Result<Option<(usize, SocketAddr)>> {
        let Some(p) = self.rx.lock().unwrap_or_else(|e| e.into_inner()).pop_front() else { return Ok(None) };
        let n = p.len().min(buf.len());
        buf[..n].copy_from_slice(&p[..n]);
        Ok(Some((n, self.peer)))
    }
    fn local_addr(&self) -> SocketAddr {
        self.me
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dgram::{NetSocket, Received};

    fn exchange<T: Transport>(mut c: NetSocket<T>, mut s: NetSocket<T>) {
        c.send_reliable(&[7u8; 3000], 0.0).unwrap();
        c.send_unreliable(b"move").unwrap();
        let mut got = Vec::new();
        for i in 0..200 {
            got.extend(s.receive(i as f64 * 0.01).unwrap());
            c.receive(i as f64 * 0.01).unwrap();
            if got.len() >= 2 && c.conn.idle() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert!(got.contains(&Received::Reliable(vec![7; 3000])), "{}", got.len());
        assert!(got.contains(&Received::Unreliable(b"move".to_vec())));
        assert!(c.conn.idle());
    }

    #[test]
    fn loopback() {
        let (a, b) = LoopbackTransport::pair();
        let (pa, pb) = (a.peer_addr(), b.peer_addr());
        exchange(NetSocket::new(a, pa), NetSocket::new(b, pb));
    }

    #[test]
    fn udp() {
        let any: SocketAddr = (Ipv4Addr::LOCALHOST, 0).into();
        let (Ok(a), Ok(b)) = (UdpTransport::bind(any), UdpTransport::bind(any)) else { return };
        let (aa, ba) = (a.local_addr(), b.local_addr());
        exchange(NetSocket::new(a, ba), NetSocket::new(b, aa));
    }
}
