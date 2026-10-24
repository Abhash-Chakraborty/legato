//! Long recordings in pieces for the phone's own model. Small models make
//! things up or give nothing past about half a minute, so a recording longer
//! than that is cut at its quietest moments and written up piece by piece.
//! Energy, not a voice detector: the phone carries no VAD model.

use std::ops::Range;

/// 16 kHz mono, as everything on the phone is.
pub const RATE: usize = 16_000;
/// A recording up to this long goes to the model in one piece.
const MAX_PIECE: usize = 30 * RATE;
/// Each cut falls this far or more after the last one.
const MIN_PIECE: usize = 20 * RATE;
/// The window a cut looks for quiet in.
const WINDOW: usize = RATE / 10;
/// A tail shorter than this joins the piece before it rather than stand alone.
const MIN_TAIL: usize = 3 * RATE;

/// Where to cut `samples`: pieces of at most 30 s (33 s at the very end, to
/// keep a short tail with its piece), each ending in the middle of the
/// quietest 100 ms after the 20 s mark.
pub fn pieces(samples: &[f32]) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let mut from = 0;
    while samples.len() - from > MAX_PIECE {
        if samples.len() - from <= MAX_PIECE + MIN_TAIL {
            break;
        }
        let mut best = from + MAX_PIECE;
        let mut quietest = f64::MAX;
        let mut at = from + MIN_PIECE;
        while at + WINDOW <= from + MAX_PIECE {
            let energy: f64 = samples[at..at + WINDOW]
                .iter()
                .map(|s| f64::from(*s) * f64::from(*s))
                .sum();
            if energy < quietest {
                quietest = energy;
                best = at + WINDOW / 2;
            }
            at += WINDOW;
        }
        out.push(from..best);
        from = best;
    }
    if from < samples.len() {
        out.push(from..samples.len());
    }
    out
}

/// A stretch with no speech in it: models make words up from silence.
pub fn quiet(samples: &[f32]) -> bool {
    samples.iter().all(|s| s.abs() < 0.02)
}

/// The pieces' text as one, a space between each.
pub fn join<S: AsRef<str>>(texts: &[S]) -> String {
    texts
        .iter()
        .map(|t| t.as_ref().trim())
        .filter(|t| !t.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Signed 16-bit little-endian PCM, as Kotlin hands recordings over (a byte
/// array crosses to Rust in one copy, a list of floats one box at a time),
/// as -1..1 floats.
pub fn from_pcm16(bytes: &[u8]) -> Vec<f32> {
    bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| f32::from(i16::from_le_bytes(*b)) / 32768.0)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Speech-loud noise with a quiet gap at each of `gaps` (in seconds).
    fn talk(seconds: usize, gaps: &[f32]) -> Vec<f32> {
        let mut s: Vec<f32> = (0..seconds * RATE)
            .map(|i| ((i as f32) * 0.37).sin() * 0.3)
            .collect();
        for g in gaps {
            let at = (g * RATE as f32) as usize;
            for x in &mut s[at..at + RATE / 5] {
                *x = 0.0;
            }
        }
        s
    }

    #[test]
    fn short_recordings_stay_whole() {
        assert_eq!(pieces(&talk(12, &[])), vec![0..12 * RATE]);
        assert_eq!(pieces(&talk(30, &[])), vec![0..30 * RATE]);
        assert!(pieces(&[]).is_empty());
    }

    #[test]
    fn long_recordings_are_cut_in_their_pauses() {
        let p = pieces(&talk(70, &[24.0, 51.0]));
        assert_eq!(p.len(), 3);
        // Each cut lands inside its pause.
        assert!(
            (24 * RATE..24 * RATE + RATE / 5).contains(&p[0].end),
            "{p:?}"
        );
        assert!(
            (51 * RATE..51 * RATE + RATE / 5).contains(&p[1].end),
            "{p:?}"
        );
        // The pieces cover the recording, end to end.
        assert_eq!(p[0].start, 0);
        assert_eq!(p[2].end, 70 * RATE);
        assert!(p.windows(2).all(|w| w[0].end == w[1].start));
        assert!(p.iter().all(|r| r.len() <= MAX_PIECE));
    }

    #[test]
    fn a_short_tail_stays_with_its_piece() {
        // 32 s: a cut near 30 s would leave 2 s on its own.
        assert_eq!(pieces(&talk(32, &[])).len(), 1);
        assert_eq!(pieces(&talk(35, &[25.0])).len(), 2);
    }

    #[test]
    fn talk_without_pauses_still_goes_in_pieces() {
        let p = pieces(&talk(125, &[]));
        assert!(p.len() >= 4, "{p:?}");
        assert!(p.iter().all(|r| r.len() <= MAX_PIECE + MIN_TAIL));
    }

    #[test]
    fn pieces_join_with_one_space() {
        assert_eq!(
            join(&["Hello there. ", "", " How are you?"]),
            "Hello there. How are you?"
        );
        assert!(quiet(&[0.0, 0.01, -0.015]));
        assert!(!quiet(&[0.0, 0.3]));
        assert_eq!(from_pcm16(&[0x00, 0x40, 0x00, 0xc0]), vec![0.5, -0.5]);
    }
}
