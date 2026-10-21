use rustfft::{num_complex::Complex32, Fft, FftPlanner};
use std::sync::Arc;

// `db` below is not true dBFS: it's a per-bin average divided by the FFT
// window size, which lands ~20 dB low for speech. The meter starts from a
// window calibrated against measured room tone (~-48 dBFS), then follows the microphone: the room's quiet sets the
// bottom and the recent loudest moment the top. A fixed window left a quiet
// mic ~1 px above the floor (#1694) and pinned a loud one at the top, and
// either reads as a meter that doesn't follow the voice.
const DB_MIN: f32 = -68.0;
const CURVE_POWER: f32 = 0.8;
/// The top never comes closer to the room's quiet than this, so a silent
/// room doesn't fill the bars with its own noise.
const MIN_RANGE_DB: f32 = 16.0;
/// How far above the room's quiet the bars start.
const FLOOR_MARGIN_DB: f32 = 3.0;
/// How fast the top falls back after a loud moment, per frame (~30 a
/// second): about 4 dB a second.
const PEAK_DECAY_DB: f32 = 0.13;
/// How fast the quiet estimate follows a quieter frame, and creeps up to
/// a louder one (about 10 s), per frame.
const FLOOR_FALL: f32 = 0.2;
const FLOOR_RISE: f32 = 0.004;

pub struct AudioVisualiser {
    fft: Arc<dyn Fft<f32>>,
    window: Vec<f32>,
    bucket_ranges: Vec<(usize, usize)>,
    fft_input: Vec<Complex32>,
    noise_floor: Vec<f32>,
    /// The room's quiet and the recent loudest frame, in the `db` scale.
    floor: f32,
    peak: f32,
    buffer: Vec<f32>,
    window_size: usize,
    buckets: usize,
}

impl AudioVisualiser {
    pub fn new(
        sample_rate: u32,
        window_size: usize,
        buckets: usize,
        freq_min: f32,
        freq_max: f32,
    ) -> Self {
        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(window_size);

        // Pre-compute Hann window
        let window: Vec<f32> = (0..window_size)
            .map(|i| {
                0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / window_size as f32).cos())
            })
            .collect();

        // Pre-compute bucket frequency ranges
        let nyquist = sample_rate as f32 / 2.0;
        let freq_min = freq_min.min(nyquist);
        let freq_max = freq_max.min(nyquist);

        let mut bucket_ranges = Vec::with_capacity(buckets);

        for b in 0..buckets {
            // Use logarithmic spacing for better perceptual representation
            let log_start = (b as f32 / buckets as f32).powi(2);
            let log_end = ((b + 1) as f32 / buckets as f32).powi(2);

            let start_hz = freq_min + (freq_max - freq_min) * log_start;
            let end_hz = freq_min + (freq_max - freq_min) * log_end;

            let start_bin = ((start_hz * window_size as f32) / sample_rate as f32) as usize;
            let mut end_bin = ((end_hz * window_size as f32) / sample_rate as f32) as usize;

            // Ensure each bucket has at least one bin
            if end_bin <= start_bin {
                end_bin = start_bin + 1;
            }

            // Clamp to valid range
            let start_bin = start_bin.min(window_size / 2);
            let end_bin = end_bin.min(window_size / 2);

            bucket_ranges.push((start_bin, end_bin));
        }

        Self {
            fft,
            window,
            bucket_ranges,
            fft_input: vec![Complex32::new(0.0, 0.0); window_size],
            noise_floor: vec![-40.0; buckets], // Initialize to reasonable noise floor
            floor: DB_MIN,
            peak: DB_MIN + MIN_RANGE_DB,
            buffer: Vec::with_capacity(window_size * 2),
            window_size,
            buckets,
        }
    }

    pub fn feed(&mut self, samples: &[f32]) -> Option<Vec<f32>> {
        // Add new samples to buffer
        self.buffer.extend_from_slice(samples);

        // Only process if we have enough samples
        if self.buffer.len() < self.window_size {
            return None;
        }

        // Take the required window of samples
        let window_samples = &self.buffer[..self.window_size];

        // Remove DC component
        let mean = window_samples.iter().sum::<f32>() / self.window_size as f32;

        // Apply window function and prepare FFT input
        for (i, &sample) in window_samples.iter().enumerate() {
            let windowed_sample = (sample - mean) * self.window[i];
            self.fft_input[i] = Complex32::new(windowed_sample, 0.0);
        }

        // Perform FFT
        self.fft.process(&mut self.fft_input);

        // Compute power spectrum and bucket levels
        let mut buckets = vec![0.0; self.buckets];
        let mut dbs = vec![-80.0f32; self.buckets];

        for (bucket_idx, &(start_bin, end_bin)) in self.bucket_ranges.iter().enumerate() {
            if start_bin >= end_bin || end_bin > self.fft_input.len() / 2 {
                continue;
            }

            // Calculate average power in this frequency range
            let mut power_sum = 0.0;
            for bin_idx in start_bin..end_bin {
                let magnitude = self.fft_input[bin_idx].norm();
                power_sum += magnitude * magnitude;
            }

            let avg_power = power_sum / (end_bin - start_bin) as f32;

            // Convert to dB with proper scaling
            let db = if avg_power > 1e-12 {
                20.0 * (avg_power.sqrt() / self.window_size as f32).log10()
            } else {
                -80.0 // Very low floor for zero power
            };

            // Only update noise floor when signal is quiet (below current floor + 10dB)
            if db < self.noise_floor[bucket_idx] + 10.0 {
                const NOISE_ALPHA: f32 = 0.001; // Very slow adaptation
                self.noise_floor[bucket_idx] =
                    NOISE_ALPHA * db + (1.0 - NOISE_ALPHA) * self.noise_floor[bucket_idx];
            }

            dbs[bucket_idx] = db;
        }

        // Follow the microphone: the loudest band sets this frame's level.
        let frame_db = dbs.iter().copied().fold(f32::MIN, f32::max);
        let rate = if frame_db < self.floor {
            FLOOR_FALL
        } else {
            FLOOR_RISE
        };
        self.floor += (frame_db - self.floor) * rate;
        self.peak = if frame_db > self.peak {
            frame_db
        } else {
            self.peak - PEAK_DECAY_DB
        };
        let bottom = self.floor + FLOOR_MARGIN_DB;
        let top = self.peak.max(self.floor + MIN_RANGE_DB);
        for (level, db) in buckets.iter_mut().zip(&dbs) {
            let normalized = ((db - bottom) / (top - bottom)).clamp(0.0, 1.0);
            *level = normalized.powf(CURVE_POWER);
        }

        // Apply light smoothing to reduce jitter
        for i in 1..buckets.len() - 1 {
            buckets[i] = buckets[i] * 0.7 + buckets[i - 1] * 0.15 + buckets[i + 1] * 0.15;
        }

        // Clear processed samples from buffer
        self.buffer.clear();

        Some(buckets)
    }

    pub fn reset(&mut self) {
        self.buffer.clear();
        // Reset noise floor to initial values
        self.noise_floor.fill(-40.0);
        self.floor = DB_MIN;
        self.peak = DB_MIN + MIN_RANGE_DB;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Bars while "speaking" (a chord) and in the gaps (faint noise), after
    /// the meter had a few seconds to settle, for a voice at `amplitude`.
    fn speech_and_gaps(amplitude: f32) -> (f32, f32) {
        let mut v = AudioVisualiser::new(16_000, 512, 16, 400.0, 4000.0);
        let mut noise = 12345u32;
        let (mut loud, mut quiet) = (Vec::new(), Vec::new());
        for chunk in 0..(8 * 16_000 / 512) {
            // 384 ms on, 384 ms off.
            let speaking = (chunk / 12) % 2 == 0;
            let samples: Vec<f32> = (0..512)
                .map(|i| {
                    let t = (chunk * 512 + i) as f32 / 16_000.0;
                    noise = noise.wrapping_mul(1_103_515_245).wrapping_add(12345);
                    // Room tone around -48 dBFS.
                    let n = ((noise >> 16) as f32 / 32_768.0 - 1.0) * 0.007;
                    let chord = [300.0, 800.0, 1500.0, 2500.0]
                        .iter()
                        .map(|f| (2.0 * std::f32::consts::PI * f * t).sin())
                        .sum::<f32>()
                        / 4.0;
                    n + if speaking { chord * amplitude } else { 0.0 }
                })
                .collect();
            let Some(levels) = v.feed(&samples) else {
                continue;
            };
            let top = levels.iter().copied().fold(0.0, f32::max);
            // Skip the first few seconds and the edges of each burst.
            if chunk > 4 * 16_000 / 512 && chunk % 12 > 1 {
                if speaking {
                    loud.push(top)
                } else {
                    quiet.push(top)
                }
            }
        }
        let mean = |v: &[f32]| v.iter().sum::<f32>() / v.len() as f32;
        (mean(&loud), mean(&quiet))
    }

    #[test]
    fn bars_follow_a_quiet_and_a_loud_voice() {
        // A quiet mic (~-40 dBFS speech) to a loud one.
        for amplitude in [0.03, 0.15, 0.8] {
            let (speaking, gaps) = speech_and_gaps(amplitude);
            assert!(
                speaking > 0.6 && speaking < 0.98,
                "amplitude {amplitude}: speaking {speaking}"
            );
            assert!(gaps < 0.2, "amplitude {amplitude}: gaps {gaps}");
        }
    }
}
