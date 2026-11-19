//! Demo playback (CL_PlayDemo_f / CL_GetMessage). A .dem file is a text
//! line holding a CD track number, then blocks of
//! `i32 length, 3 × f32 view angles, length bytes of server message`.

pub mod index;
pub mod keyframes;
pub mod library;
pub mod playback;

pub struct Demo {
    data: std::sync::Arc<[u8]>,
    pos: usize,
    /// First block, after the CD track header.
    start: usize,
    /// `pause` during playback (cls.demopaused): no messages, client clock stopped.
    pub paused: bool,
    /// Speed, seeking, marks and the timeline (`playback`).
    pub play: playback::Playback,
}

/// `YYYYMMDD-HHMMSS` (UTC) for auto-demo and video file names.
pub fn stamp(seconds: u64) -> String {
    let days = (seconds / 86400) as i64 + 719468;
    let era = days / 146097;
    let doe = days - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}{month:02}{day:02}-{:02}{:02}{:02}", seconds / 3600 % 24, seconds / 60 % 60, seconds % 60)
}

pub fn stamp_now() -> String {
    let secs = qcore::time::SystemTime::now().duration_since(qcore::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    stamp(secs)
}

/// One recorded server message with the view angles at the time.
pub struct DemoMessage<'a> {
    pub angles: [f32; 3],
    pub data: &'a [u8],
}

/// Largest message a demo block may hold (MAX_MSGLEN in ironwail).
const MAX_MSGLEN: usize = 96_000;

impl Demo {
    pub fn new(data: Vec<u8>) -> Self {
        // The header (cls.forcetrack, unused: svc_cdtrack follows) is an
        // optional '-' and digits ending in '\n'.
        let pos = data.iter().take(16).position(|&b| b == b'\n').map_or(0, |e| e + 1);
        Demo { data: data.into(), pos, start: pos, paused: false, play: playback::Playback::default() }
    }

    /// The whole file, shared with the background indexer.
    pub fn data(&self) -> std::sync::Arc<[u8]> {
        self.data.clone()
    }

    /// Offset of the next block.
    pub fn pos(&self) -> usize {
        self.pos
    }

    /// Continue reading at block offset `pos` (a keyframe or the start).
    pub fn seek(&mut self, pos: usize) {
        self.pos = pos.clamp(self.start, self.data.len());
    }

    pub fn rewind(&mut self) {
        self.pos = self.start;
    }

    pub fn at_end(&self) -> bool {
        self.data.len() - self.pos < 16
    }

    /// The next message, or `None` at the end of the file or on a damaged block.
    pub fn next_message(&mut self) -> Option<DemoMessage<'_>> {
        let rest = self.data.get(self.pos..)?;
        if rest.len() < 16 {
            return None;
        }
        let word = |i: usize| <[u8; 4]>::try_from(&rest[i * 4..i * 4 + 4]).unwrap();
        let len = i32::from_le_bytes(word(0));
        if len < 0 || len as usize > MAX_MSGLEN || rest.len() < 16 + len as usize {
            return None;
        }
        let angles = [1, 2, 3].map(|i| f32::from_le_bytes(word(i)));
        let start = self.pos + 16;
        self.pos = start + len as usize;
        Some(DemoMessage { angles, data: &self.data[start..self.pos] })
    }
}

/// A demo being recorded (cls.demofile with cls.demorecording).
pub struct DemoWriter {
    path: std::path::PathBuf,
    buf: Vec<u8>,
}

/// Buffered bytes appended to the file at a time.
const DEMO_FLUSH: usize = 64 * 1024;

impl DemoWriter {
    /// Creates the file and writes the CD track header.
    pub fn create(path: &std::path::Path, track: i32) -> std::io::Result<Self> {
        qcore::vfs::write(path, format!("{track}\n").as_bytes())?;
        Ok(DemoWriter { path: path.to_path_buf(), buf: Vec::with_capacity(DEMO_FLUSH) })
    }

    /// CL_WriteDemoMessage.
    pub fn write(&mut self, angles: [f32; 3], data: &[u8]) -> std::io::Result<()> {
        self.buf.extend((data.len() as i32).to_le_bytes());
        for a in angles {
            self.buf.extend(a.to_le_bytes());
        }
        self.buf.extend(data);
        if self.buf.len() >= DEMO_FLUSH { self.flush() } else { Ok(()) }
    }

    pub fn flush(&mut self) -> std::io::Result<()> {
        if self.buf.is_empty() { return Ok(()); }
        let r = qcore::vfs::append(&self.path, &self.buf);
        self.buf.clear();
        r
    }

    /// Writes what was recorded so far to `dest`, ended with svc_disconnect
    /// so it plays on its own; recording continues.
    pub fn copy_to(&mut self, dest: &std::path::Path, angles: [f32; 3]) -> std::io::Result<()> {
        self.flush()?;
        let mut data = qcore::vfs::read(&self.path)?;
        data.extend(1i32.to_le_bytes());
        for a in angles {
            data.extend(a.to_le_bytes());
        }
        data.push(qnet::protocol::SVC_DISCONNECT);
        qcore::vfs::write(dest, &data)
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    pub fn finish(mut self) -> std::io::Result<()> {
        self.flush()
    }
}

impl Drop for DemoWriter {
    fn drop(&mut self) {
        let _ = self.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(angles: [f32; 3], msg: &[u8]) -> Vec<u8> {
        let mut v = (msg.len() as i32).to_le_bytes().to_vec();
        for a in angles {
            v.extend(a.to_le_bytes());
        }
        v.extend(msg);
        v
    }

    #[test]
    fn reads_header_and_blocks() {
        let mut d = b"-1\n".to_vec();
        d.extend(block([1.0, 2.0, 3.0], &[7, 8]));
        d.extend(block([0.0; 3], &[]));
        d.extend([1, 2, 3]); // truncated tail
        let mut demo = Demo::new(d);
        let m = demo.next_message().unwrap();
        assert_eq!((m.angles, m.data), ([1.0, 2.0, 3.0], &[7u8, 8][..]));
        assert!(demo.next_message().unwrap().data.is_empty());
        assert!(demo.next_message().is_none());
    }

    #[test]
    fn writer_round_trips() {
        let dir = std::env::temp_dir().join(format!("qsp-demo-{}", std::process::id()));
        let path = dir.join("t.dem");
        let mut w = DemoWriter::create(&path, 3).unwrap();
        w.write([4.0, 5.0, 6.0], &[1, 2, 3]).unwrap();
        w.finish().unwrap();
        let data = std::fs::read(&path).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(data.starts_with(b"3\n"));
        let mut d = Demo::new(data);
        let m = d.next_message().unwrap();
        assert_eq!((m.angles, m.data), ([4.0, 5.0, 6.0], &[1u8, 2, 3][..]));
    }

    #[test]
    fn stamps() {
        assert_eq!(stamp(0), "19700101-000000");
        assert_eq!(stamp(1_700_000_000), "20231114-221320");
    }

    #[test]
    fn rejects_oversized_block() {
        let mut d = b"2\n".to_vec();
        d.extend(i32::MAX.to_le_bytes());
        d.extend([0; 12]);
        let mut demo = Demo::new(d);
        assert!(demo.next_message().is_none());
    }
}
pub mod convert;
