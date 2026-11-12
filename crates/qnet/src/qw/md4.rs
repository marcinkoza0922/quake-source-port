//! MD4 (RFC 1320) and Com_BlockChecksum.

fn md4(data: &[u8]) -> [u32; 4] {
    let mut s = [0x6745_2301u32, 0xefcd_ab89, 0x98ba_dcfe, 0x1032_5476];
    let mut msg = data.to_vec();
    let bits = (data.len() as u64).wrapping_mul(8);
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bits.to_le_bytes());
    let f = |x: u32, y: u32, z: u32| (x & y) | (!x & z);
    let g = |x: u32, y: u32, z: u32| (x & y) | (x & z) | (y & z);
    let h = |x: u32, y: u32, z: u32| x ^ y ^ z;
    for block in msg.as_chunks::<64>().0 {
        let x: Vec<u32> = block.as_chunks::<4>().0.iter().map(|w| u32::from_le_bytes(*w)).collect();
        let [mut a, mut b, mut c, mut d] = s;
        for &i in &[0, 4, 8, 12] {
            a = a.wrapping_add(f(b, c, d)).wrapping_add(x[i]).rotate_left(3);
            d = d.wrapping_add(f(a, b, c)).wrapping_add(x[i + 1]).rotate_left(7);
            c = c.wrapping_add(f(d, a, b)).wrapping_add(x[i + 2]).rotate_left(11);
            b = b.wrapping_add(f(c, d, a)).wrapping_add(x[i + 3]).rotate_left(19);
        }
        const K2: u32 = 0x5a82_7999;
        for &i in &[0, 1, 2, 3] {
            a = a.wrapping_add(g(b, c, d)).wrapping_add(x[i]).wrapping_add(K2).rotate_left(3);
            d = d.wrapping_add(g(a, b, c)).wrapping_add(x[i + 4]).wrapping_add(K2).rotate_left(5);
            c = c.wrapping_add(g(d, a, b)).wrapping_add(x[i + 8]).wrapping_add(K2).rotate_left(9);
            b = b.wrapping_add(g(c, d, a)).wrapping_add(x[i + 12]).wrapping_add(K2).rotate_left(13);
        }
        const K3: u32 = 0x6ed9_eba1;
        for &i in &[0, 2, 1, 3] {
            a = a.wrapping_add(h(b, c, d)).wrapping_add(x[i]).wrapping_add(K3).rotate_left(3);
            d = d.wrapping_add(h(a, b, c)).wrapping_add(x[i + 8]).wrapping_add(K3).rotate_left(9);
            c = c.wrapping_add(h(d, a, b)).wrapping_add(x[i + 4]).wrapping_add(K3).rotate_left(11);
            b = b.wrapping_add(h(c, d, a)).wrapping_add(x[i + 12]).wrapping_add(K3).rotate_left(15);
        }
        s[0] = s[0].wrapping_add(a);
        s[1] = s[1].wrapping_add(b);
        s[2] = s[2].wrapping_add(c);
        s[3] = s[3].wrapping_add(d);
    }
    s
}

/// Com_BlockChecksum: the MD4 digest's four little-endian words XORed.
pub fn block_checksum(data: &[u8]) -> u32 {
    let d = md4(data);
    d[0] ^ d[1] ^ d[2] ^ d[3]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(d: [u32; 4]) -> String {
        d.iter().flat_map(|w| w.to_le_bytes()).map(|b| format!("{b:02x}")).collect()
    }

    #[test]
    fn rfc1320_vectors() {
        assert_eq!(hex(md4(b"")), "31d6cfe0d16ae931b73c59d7e0c089c0");
        assert_eq!(hex(md4(b"abc")), "a448017aaf21d8525fc10ae87aa6729d");
        assert_eq!(
            hex(md4(b"12345678901234567890123456789012345678901234567890123456789012345678901234567890")),
            "e33b4ddc9c38f2199c3e7b164fcc0536"
        );
    }
}
