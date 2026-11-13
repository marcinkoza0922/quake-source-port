//! Export the engine's original portal cues for auditioning.
//! rustc --edition=2024 tools/portal_cues.rs -o target/portal-cues
//! target/portal-cues target/portal-audio
use std::{fs, io::Write, path::PathBuf};
#[allow(dead_code)]
pub struct Sfx { name: String, data: Vec<i16>, loop_start: Option<usize> }
#[path = "../crates/qsound/src/portal.rs"]
mod portal;
fn main() -> std::io::Result<()> {
    let dir = PathBuf::from(std::env::args().nth(1).unwrap_or("target/portal-audio".into()));
    fs::create_dir_all(&dir)?;
    for (kind,name) in ["fire","placed","dissipate","traverse"].iter().enumerate() {
        for slot in 0..2 {
            let sfx = portal::render(kind,slot,48000);
            let bytes = (sfx.data.len()*2) as u32;
            let mut out = fs::File::create(dir.join(format!("{name}-{slot}.wav")))?;
            out.write_all(b"RIFF")?; out.write_all(&(36+bytes).to_le_bytes())?;
            out.write_all(b"WAVEfmt ")?; out.write_all(&16u32.to_le_bytes())?;
            out.write_all(&1u16.to_le_bytes())?; out.write_all(&1u16.to_le_bytes())?;
            out.write_all(&48000u32.to_le_bytes())?; out.write_all(&96000u32.to_le_bytes())?;
            out.write_all(&2u16.to_le_bytes())?; out.write_all(&16u16.to_le_bytes())?;
            out.write_all(b"data")?; out.write_all(&bytes.to_le_bytes())?;
            for sample in sfx.data { out.write_all(&sample.to_le_bytes())?; }
        }
    }
    Ok(())
}
