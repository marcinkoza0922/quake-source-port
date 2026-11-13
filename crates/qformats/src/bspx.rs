//! BSPX extension lumps (Mod_LoadBSPX): a "BSPX" directory appended after the
//! last standard lump, 4-byte aligned, holding named extra lumps such as
//! `RGBLIGHTING` (coloured light, 3 bytes per mono lightmap sample).

use crate::bsp::HEADER_LUMPS;

/// Find a BSPX lump by name in a whole `.bsp` file. Returns `None` for files
/// without a BSPX directory or with a malformed one.
pub fn find_lump<'a>(data: &'a [u8], name: &str) -> Option<&'a [u8]> {
    let u32_at = |o: usize| data.get(o..o + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap()) as usize);
    // Directory starts after the furthest standard lump.
    let mut end = 0usize;
    for i in 0..HEADER_LUMPS {
        let ofs = u32_at(4 + i * 8)?;
        let len = u32_at(8 + i * 8)?;
        end = end.max(ofs.checked_add(len)?);
    }
    let dir = end.checked_add(3)? & !3;
    if data.get(dir..dir + 4)? != b"BSPX" {
        return None;
    }
    let count = u32_at(dir + 4)?;
    (0..count.min(1024)).find_map(|i| {
        let e = dir + 8 + i * 32;
        let raw = data.get(e..e + 24)?;
        let lname = raw.split(|&b| b == 0).next()?;
        if !lname.eq_ignore_ascii_case(name.as_bytes()) {
            return None;
        }
        let (ofs, len) = (u32_at(e + 24)?, u32_at(e + 28)?);
        data.get(ofs..ofs.checked_add(len)?)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bsp_with_bspx(lumps: &[(&str, &[u8])]) -> Vec<u8> {
        // Header with every standard lump empty at offset 0, then 2 pad bytes
        // to exercise alignment.
        let mut d = 29i32.to_le_bytes().to_vec();
        d.extend(std::iter::repeat_n(0u8, HEADER_LUMPS * 8));
        // Point lump 0 at the header end + 2 bytes of payload.
        let hdr = d.len() as u32;
        d[4..8].copy_from_slice(&hdr.to_le_bytes());
        d[8..12].copy_from_slice(&2u32.to_le_bytes());
        d.extend([0xaa, 0xbb]);
        while d.len() % 4 != 0 {
            d.push(0);
        }
        d.extend(b"BSPX");
        d.extend((lumps.len() as u32).to_le_bytes());
        let mut ofs = d.len() + lumps.len() * 32;
        for (n, p) in lumps {
            let mut nm = [0u8; 24];
            nm[..n.len()].copy_from_slice(n.as_bytes());
            d.extend(nm);
            d.extend((ofs as u32).to_le_bytes());
            d.extend((p.len() as u32).to_le_bytes());
            ofs += p.len();
        }
        for (_, p) in lumps {
            d.extend(*p);
        }
        d
    }

    #[test]
    fn finds_named_lump() {
        let d = bsp_with_bspx(&[("LMSHIFT", &[4]), ("RGBLIGHTING", &[1, 2, 3])]);
        assert_eq!(find_lump(&d, "RGBLIGHTING"), Some(&[1, 2, 3][..]));
        assert_eq!(find_lump(&d, "LMSHIFT"), Some(&[4][..]));
        assert_eq!(find_lump(&d, "DECOUPLED_LM"), None);
    }

    #[test]
    fn absent_or_truncated() {
        let mut d = bsp_with_bspx(&[("RGBLIGHTING", &[1, 2, 3])]);
        d.truncate(d.len() - 2);
        assert_eq!(find_lump(&d, "RGBLIGHTING"), None);
        assert_eq!(find_lump(&d[..64], "RGBLIGHTING"), None);
    }
}
