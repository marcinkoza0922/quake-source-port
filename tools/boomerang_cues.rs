#![allow(dead_code)]
//! Export the engine's original boomerang cues for auditioning.
//! rustc --edition=2024 tools/boomerang_cues.rs -o target/boomerang-cues
//! target/boomerang-cues target/boomerang-audio
use std::{fs, io::Write, path::PathBuf};
#[allow(dead_code)]
pub struct Sfx { name: String, data: Vec<i16>, loop_start: Option<usize> }
#[path = "../crates/qsound/src/boomerang.rs"]
mod boomerang;
fn main() -> std::io::Result<()> {
    let dir = PathBuf::from(std::env::args().nth(1).unwrap_or("target/boomerang-audio".into()));
    fs::create_dir_all(&dir)?;
    for (kind,name) in ["throw","stun","catch","swish"].iter().enumerate() {
        {
            let sfx = boomerang::render(kind,48000);
            let mut out = fs::File::create(dir.join(format!("{name}.wav")))?;
            let bytes = (sfx.data.len()*2) as u32;
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
