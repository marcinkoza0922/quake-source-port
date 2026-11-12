use super::*;

/// Quake64 lightmaps: 16-bit samples packed RRRRRGGG GGBBBBBB (first byte
/// high bits), expanded like ironwail. Returns (mono, rgb).
pub(super) fn q64_lighting(lump: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let n = lump.len() / 2;
    let mut mono = Vec::with_capacity(n);
    let mut rgb = Vec::with_capacity(n * 3);
    for &[b0, b1] in lump.as_chunks::<2>().0 {
        let c = [b0 & 0xf8, ((b0 & 0x07) << 5) + ((b1 & 0xc0) >> 5), (b1 & 0x3f) << 2];
        rgb.extend_from_slice(&c);
        mono.push(((c[0] as u32 + c[1] as u32 + c[2] as u32) / 3) as u8);
    }
    (mono, rgb)
}

pub(super) fn parse_textures(lump: &[u8], q64: bool) -> Result<Vec<Option<MipTex>>> {
    if lump.is_empty() {
        return Ok(Vec::new());
    }
    let mut r = Reader::new(lump, FMT);
    let n = r.count("miptex", 4)?;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let ofs = r.i32()?;
        if ofs < 0 {
            out.push(None);
            continue;
        }
        let mut t = Reader::new(lump, FMT);
        t.seek(ofs as usize)
            .map_err(|e| Error::new(FMT, format!("miptex {i}: {}", e.msg)))?;
        let name = t.name::<16>()?;
        let width = t.u32()?;
        let height = t.u32()?;
        let shift = if q64 { t.u32()? } else { 0 };
        let offsets = [t.u32()?, t.u32()?, t.u32()?, t.u32()?];
        if width > 16384 || height > 16384 {
            return Err(Error::new(
                FMT,
                format!("miptex {name} has bad size {width}x{height}"),
            ));
        }
        let mut mips = Vec::new();
        if offsets[0] != 0 {
            // Texel data past the end of the lump appears in the wild
            // (e.g. jam2_tronyn, qbj_grue). Like ironwail, clamp mip 0 to the
            // lump (zero-filling the rest) and derive broken smaller mips by
            // point sampling the previous level.
            for (lvl, &o) in offsets.iter().enumerate() {
                let (w, h) = ((width >> lvl) as usize, (height >> lvl) as usize);
                let sz = w * h;
                let start = ofs as usize + o as usize;
                // Quake64 stores only mip 0 (every offset points at it).
                let own = lvl == 0 || !q64;
                if let Some(px) = lump.get(start..start + sz).filter(|_| own) {
                    mips.push(px.to_vec());
                } else if let Some(prev) = mips.last() {
                    let pw = (width >> (lvl - 1)) as usize;
                    mips.push((0..sz).map(|i| prev[(i / w) * 2 * pw + (i % w) * 2]).collect());
                } else {
                    let mut px = lump.get(start.min(lump.len())..).unwrap_or(&[]).to_vec();
                    px.resize(sz, 0);
                    mips.push(px);
                }
            }
        }
        let kind = TextureKind::from_name(&name);
        out.push(Some(MipTex {
            name,
            width,
            height,
            kind,
            mips,
            shift,
        }));
    }
    Ok(out)
}
