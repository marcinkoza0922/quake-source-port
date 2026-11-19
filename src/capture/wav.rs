//! 16-bit stereo PCM WAV written as it goes; sizes are patched on finish.

use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

pub struct Writer {
    file: std::io::BufWriter<std::fs::File>,
    path: PathBuf,
    bytes: u32,
}

impl Writer {
    pub fn create(path: &Path, rate: u32) -> std::io::Result<Self> {
        let mut file = std::io::BufWriter::new(std::fs::File::create(path)?);
        let mut h = Vec::with_capacity(44);
        h.extend(b"RIFF");
        h.extend(0u32.to_le_bytes());
        h.extend(b"WAVEfmt ");
        h.extend(16u32.to_le_bytes());
        h.extend(1u16.to_le_bytes()); // PCM
        h.extend(2u16.to_le_bytes());
        h.extend(rate.to_le_bytes());
        h.extend((rate * 4).to_le_bytes());
        h.extend(4u16.to_le_bytes());
        h.extend(16u16.to_le_bytes());
        h.extend(b"data");
        h.extend(0u32.to_le_bytes());
        file.write_all(&h)?;
        Ok(Self { file, path: path.to_path_buf(), bytes: 0 })
    }

    /// Interleaved stereo samples.
    pub fn write(&mut self, samples: &[i16]) -> std::io::Result<()> {
        let mut b = Vec::with_capacity(samples.len() * 2);
        for s in samples {
            b.extend(s.to_le_bytes());
        }
        self.file.write_all(&b)?;
        self.bytes = self.bytes.saturating_add(b.len() as u32);
        Ok(())
    }

    /// Patches the RIFF and data sizes; returns the path.
    pub fn finish(mut self) -> std::io::Result<PathBuf> {
        self.file.flush()?;
        let f = self.file.get_mut();
        f.seek(SeekFrom::Start(4))?;
        f.write_all(&(36 + self.bytes).to_le_bytes())?;
        f.seek(SeekFrom::Start(40))?;
        f.write_all(&self.bytes.to_le_bytes())?;
        f.flush()?;
        Ok(self.path)
    }
}
