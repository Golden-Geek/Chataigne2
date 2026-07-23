use std::{io::Write, path::Path};

use tempfile::{Builder, NamedTempFile};

pub fn sine_wave_file(channels: u16, sample_rate: u32, frames: u32) -> NamedTempFile {
    let mut file = Builder::new().suffix(".wav").tempfile().unwrap();
    write_sine_wave(file.as_file_mut(), channels, sample_rate, frames);
    file
}

pub fn rewrite_sine_wave(path: &Path, channels: u16, sample_rate: u32, frames: u32) {
    let mut file = std::fs::File::create(path).unwrap();
    write_sine_wave(&mut file, channels, sample_rate, frames);
}

fn write_sine_wave(writer: &mut impl Write, channels: u16, sample_rate: u32, frames: u32) {
    let bits_per_sample = 16_u16;
    let bytes_per_sample = u32::from(bits_per_sample / 8);
    let data_bytes = frames
        .saturating_mul(u32::from(channels))
        .saturating_mul(bytes_per_sample);
    writer.write_all(b"RIFF").unwrap();
    writer.write_all(&(36_u32 + data_bytes).to_le_bytes()).unwrap();
    writer.write_all(b"WAVEfmt ").unwrap();
    writer.write_all(&16_u32.to_le_bytes()).unwrap();
    writer.write_all(&1_u16.to_le_bytes()).unwrap();
    writer.write_all(&channels.to_le_bytes()).unwrap();
    writer.write_all(&sample_rate.to_le_bytes()).unwrap();
    writer
        .write_all(
            &(sample_rate
                .saturating_mul(u32::from(channels))
                .saturating_mul(bytes_per_sample))
            .to_le_bytes(),
        )
        .unwrap();
    writer
        .write_all(&(channels.saturating_mul(bits_per_sample / 8)).to_le_bytes())
        .unwrap();
    writer.write_all(&bits_per_sample.to_le_bytes()).unwrap();
    writer.write_all(b"data").unwrap();
    writer.write_all(&data_bytes.to_le_bytes()).unwrap();
    for frame in 0..frames {
        let phase = frame as f32 * 440.0 * std::f32::consts::TAU / sample_rate as f32;
        let sample = (phase.sin() * 16_000.0) as i16;
        for _ in 0..channels {
            writer.write_all(&sample.to_le_bytes()).unwrap();
        }
    }
    writer.flush().unwrap();
}
