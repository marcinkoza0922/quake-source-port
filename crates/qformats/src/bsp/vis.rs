//! PVS rows.

/// Decompress a run-length encoded PVS row. Returns `(numleafs + 7) / 8` bytes;
/// if `visofs < 0` or out of range, all leafs are visible (0xff).
pub fn decompress_vis(visdata: &[u8], visofs: i32, numleafs: usize) -> Vec<u8> {
    let row = numleafs.div_ceil(8);
    if visofs < 0 || visofs as usize >= visdata.len() {
        return vec![0xff; row];
    }
    let mut out = Vec::with_capacity(row);
    let mut i = visofs as usize;
    while out.len() < row {
        let Some(&b) = visdata.get(i) else { break };
        i += 1;
        if b != 0 {
            out.push(b);
            continue;
        }
        let Some(&c) = visdata.get(i) else { break };
        i += 1;
        let c = (c as usize).min(row - out.len());
        out.resize(out.len() + c, 0);
    }
    out.resize(row, 0);
    out
}
