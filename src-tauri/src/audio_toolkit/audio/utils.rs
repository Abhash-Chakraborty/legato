use anyhow::Result;
use hound::{WavReader, WavSpec, WavWriter};
use log::debug;
use std::path::Path;

/// Read a WAV file and return normalised f32 samples.
pub fn read_wav_samples<P: AsRef<Path>>(file_path: P) -> Result<Vec<f32>> {
    let reader = WavReader::open(file_path.as_ref())?;
    let samples = reader
        .into_samples::<i16>()
        .map(|s| s.map(|v| v as f32 / i16::MAX as f32))
        .collect::<Result<Vec<f32>, _>>()?;
    Ok(samples)
}

/// Verify a WAV file by reading it back and checking the sample count.
pub fn verify_wav_file<P: AsRef<Path>>(file_path: P, expected_samples: usize) -> Result<()> {
    let reader = WavReader::open(file_path.as_ref())?;
    let actual_samples = reader.len() as usize;
    if actual_samples != expected_samples {
        anyhow::bail!(
            "WAV sample count mismatch: expected {}, got {}",
            expected_samples,
            actual_samples
        );
    }
    Ok(())
}

/// A recording peaking below this is raised before a batch run.
const QUIET_PEAK: f32 = 0.5;
/// Where a raised recording peaks.
const LIFTED_PEAK: f32 = 0.9;
/// The most a recording is raised (about 26 dB), so near-silence stays quiet.
const MAX_LIFT: f32 = 20.0;

/// Raises a quiet recording so its peak sits near full scale, and returns
/// the gain applied (1.0 when it was loud enough). A batch run of Parakeet
/// heard nothing in a recording peaking at -22 dB that the same model,
/// streamed, wrote out in full; raised 8 times or more, the batch run heard
/// it too.
pub fn lift_quiet(samples: &mut [f32]) -> f32 {
    let peak = samples.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    if peak <= 0.0 || peak >= QUIET_PEAK {
        return 1.0;
    }
    let gain = (LIFTED_PEAK / peak).min(MAX_LIFT);
    for s in samples.iter_mut() {
        *s *= gain;
    }
    gain
}

/// Save audio samples as a WAV file
pub fn save_wav_file<P: AsRef<Path>>(file_path: P, samples: &[f32]) -> Result<()> {
    let spec = WavSpec {
        channels: 1,
        sample_rate: 16000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = WavWriter::create(file_path.as_ref(), spec)?;

    // Convert f32 samples to i16 for WAV
    for sample in samples {
        let sample_i16 = (sample * i16::MAX as f32) as i16;
        writer.write_sample(sample_i16)?;
    }

    writer.finalize()?;
    debug!("Saved WAV file: {:?}", file_path.as_ref());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::lift_quiet;

    #[test]
    fn a_quiet_recording_is_raised_to_near_full_scale() {
        let mut pcm = vec![0.0, 0.076, -0.05, 0.01];
        let gain = lift_quiet(&mut pcm);
        assert!((gain - 0.9 / 0.076).abs() < 1e-4);
        assert!((pcm[1] - 0.9).abs() < 1e-5);
        assert!((pcm[2] + 0.05 * gain).abs() < 1e-5);
    }

    #[test]
    fn a_loud_recording_and_silence_are_left_alone() {
        let mut loud = vec![0.2, -0.88, 0.5];
        assert_eq!(lift_quiet(&mut loud), 1.0);
        assert_eq!(loud, vec![0.2, -0.88, 0.5]);
        let mut silent = vec![0.0; 4];
        assert_eq!(lift_quiet(&mut silent), 1.0);
    }

    #[test]
    fn near_silence_is_raised_at_most_twenty_times() {
        let mut pcm = vec![0.001, -0.002];
        assert_eq!(lift_quiet(&mut pcm), 20.0);
        assert!((pcm[1] + 0.04).abs() < 1e-6);
    }
}
