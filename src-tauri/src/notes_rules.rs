//! The rules behind a Notes recording, kept free of audio devices and app
//! state so they can be tested on their own: where one spoken line ends,
//! which lines are the microphone hearing the speakers, and how a level is
//! shown.

/// Samples per second of the audio Notes records.
pub const RATE: usize = 16_000;

/// Audio kept before speech starts, so a line's first word isn't clipped.
const PRE_ROLL: usize = RATE * 300 / 1000;
/// Silence that ends a line.
const END_PAUSE: usize = RATE * 700 / 1000;
/// Past this length a shorter pause ends the line. Short enough that a
/// steady talker's words reach the transcript within seconds.
const LONG_LINE: usize = RATE * 8;
const LONG_LINE_PAUSE: usize = RATE * 200 / 1000;
/// A line is cut here even without a pause.
const MAX_LINE: usize = RATE * 15;
/// Less speech than this is a cough or a click, not a line.
const MIN_SPEECH: usize = RATE * 300 / 1000;

/// One spoken line, ready to transcribe.
#[derive(Debug, Clone, PartialEq)]
pub struct Utterance {
    /// Sample offset of the first sample from the start of the track.
    pub start: usize,
    pub samples: Vec<f32>,
}

impl Utterance {
    pub fn start_ms(&self) -> i64 {
        (self.start * 1000 / RATE) as i64
    }

    pub fn end_ms(&self) -> i64 {
        ((self.start + self.samples.len()) * 1000 / RATE) as i64
    }
}

/// Cuts a track into spoken lines from per-frame voice activity, as the
/// audio arrives.
pub struct Segmenter {
    /// Samples pushed so far.
    at: usize,
    /// Recent audio while nobody speaks, at most `PRE_ROLL` long.
    before: Vec<f32>,
    /// The line being spoken, when one is.
    line: Option<Line>,
}

struct Line {
    start: usize,
    samples: Vec<f32>,
    speech: usize,
    /// Silence at the end of `samples`.
    pause: usize,
}

impl Default for Segmenter {
    fn default() -> Self {
        Self::new()
    }
}

impl Segmenter {
    pub fn new() -> Self {
        Self {
            at: 0,
            before: Vec::new(),
            line: None,
        }
    }

    /// Adds one frame and whether it holds speech. Returns a line when this
    /// frame ended one.
    pub fn push(&mut self, frame: &[f32], speech: bool) -> Option<Utterance> {
        let start = self.at;
        self.at += frame.len();
        match self.line.as_mut() {
            None if speech => {
                let mut samples = std::mem::take(&mut self.before);
                let line_start = start - samples.len();
                samples.extend_from_slice(frame);
                self.line = Some(Line {
                    start: line_start,
                    samples,
                    speech: frame.len(),
                    pause: 0,
                });
                None
            }
            None => {
                self.before.extend_from_slice(frame);
                if self.before.len() > PRE_ROLL {
                    let extra = self.before.len() - PRE_ROLL;
                    self.before.drain(..extra);
                }
                None
            }
            Some(line) => {
                line.samples.extend_from_slice(frame);
                if speech {
                    line.speech += frame.len();
                    line.pause = 0;
                } else {
                    line.pause += frame.len();
                }
                let long = line.samples.len() >= LONG_LINE;
                let ended = line.pause >= END_PAUSE
                    || (long && line.pause >= LONG_LINE_PAUSE)
                    || line.samples.len() >= MAX_LINE;
                if ended {
                    self.end()
                } else {
                    None
                }
            }
        }
    }

    /// The line being spoken so far, for a preview of its words: where it
    /// starts and its audio. None between lines.
    pub fn open_line(&self) -> Option<(usize, &[f32])> {
        self.line.as_ref().map(|l| (l.start, l.samples.as_slice()))
    }

    /// Ends the line being spoken, if any; the track has stopped.
    pub fn finish(&mut self) -> Option<Utterance> {
        self.end()
    }

    fn end(&mut self) -> Option<Utterance> {
        let mut line = self.line.take()?;
        // Keep a little of the pause, and start the next pre-roll with the
        // rest of it.
        let keep = line.pause.min(RATE * 200 / 1000);
        let cut = line.samples.len() - (line.pause - keep);
        let tail = line.samples.split_off(cut);
        self.before = tail;
        if self.before.len() > PRE_ROLL {
            let extra = self.before.len() - PRE_ROLL;
            self.before.drain(..extra);
        }
        (line.speech >= MIN_SPEECH).then_some(Utterance {
            start: line.start,
            samples: line.samples,
        })
    }
}

/// How close in time two lines must be for one to be the other's echo.
const ECHO_WINDOW_MS: i64 = 2_000;

/// `text` with each whole-word `from` (one or more words, any case)
/// written as `to`; None when it has none.
pub fn replace_words(text: &str, from: &str, to: &str) -> Option<String> {
    let from = from.trim();
    if from.is_empty() {
        return None;
    }
    let lower = text.to_lowercase();
    let needle = from.to_lowercase();
    // Lowercasing can change byte lengths; then offsets wouldn't line up.
    if lower.len() != text.len() {
        return None;
    }
    let word = |c: Option<char>| c.is_some_and(char::is_alphanumeric);
    let mut out = String::with_capacity(text.len());
    let mut from_byte = 0;
    let mut found = false;
    let mut search = 0;
    while let Some(i) = lower[search..].find(&needle) {
        let at = search + i;
        let end = at + needle.len();
        if !word(text[..at].chars().next_back()) && !word(text[end..].chars().next()) {
            out.push_str(&text[from_byte..at]);
            out.push_str(to);
            from_byte = end;
            found = true;
        }
        search = end;
    }
    found.then(|| {
        out.push_str(&text[from_byte..]);
        out
    })
}

/// One speaker's turn, as the speaker model found it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Turn {
    pub speaker: i32,
    pub t0_ms: i64,
    pub t1_ms: i64,
}

/// A line counts as a speaker's when their turns cover at least this share
/// of it.
const SPEAKER_SHARE: f64 = 0.3;

/// Who said each line, given as (start, end) in ms: the speaker whose turns
/// cover most of it, numbered 1, 2, … in the order they first speak. None
/// when no one covers enough of it, and for every line when only one
/// speaker was found, since then there's no one to tell apart.
pub fn assign_speakers(lines: &[(i64, i64)], turns: &[Turn]) -> Vec<Option<i64>> {
    let raw: Vec<Option<i32>> = lines
        .iter()
        .map(|&(at, end)| {
            let length = (end - at).max(1) as f64;
            let mut cover: Vec<(i32, i64)> = Vec::new();
            for turn in turns {
                let overlap = turn.t1_ms.min(end) - turn.t0_ms.max(at);
                if overlap <= 0 {
                    continue;
                }
                match cover.iter_mut().find(|(s, _)| *s == turn.speaker) {
                    Some((_, ms)) => *ms += overlap,
                    None => cover.push((turn.speaker, overlap)),
                }
            }
            cover
                .into_iter()
                .max_by_key(|&(speaker, ms)| (ms, -speaker))
                .filter(|&(_, ms)| ms as f64 >= length * SPEAKER_SHARE)
                .map(|(speaker, _)| speaker)
        })
        .collect();
    // Renumber by first line, so Speaker 1 is the first to talk.
    let mut order: Vec<i32> = Vec::new();
    for speaker in raw.iter().flatten() {
        if !order.contains(speaker) {
            order.push(*speaker);
        }
    }
    if order.len() < 2 {
        return vec![None; lines.len()];
    }
    raw.into_iter()
        .map(|s| {
            s.and_then(|s| order.iter().position(|&o| o == s))
                .map(|i| i as i64 + 1)
        })
        .collect()
}

/// Keeps the numbers people already see. `new` is a fresh pass over the
/// same lines as `old` (the last pass); each new voice takes the number of
/// the old one it shares the most lines with, and voices the last pass
/// didn't have get the next free numbers, in the order they first speak.
pub fn keep_numbers(old: &[Option<i64>], new: &[Option<i64>]) -> Vec<Option<i64>> {
    let mut shared: Vec<((i64, i64), usize)> = Vec::new();
    for (o, n) in old.iter().zip(new) {
        if let (Some(o), Some(n)) = (o, n) {
            match shared.iter_mut().find(|(pair, _)| *pair == (*n, *o)) {
                Some((_, count)) => *count += 1,
                None => shared.push(((*n, *o), 1)),
            }
        }
    }
    shared.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let mut map: Vec<(i64, i64)> = Vec::new();
    for ((n, o), _) in shared {
        if !map.iter().any(|&(mn, mo)| mn == n || mo == o) {
            map.push((n, o));
        }
    }
    let mut next = old.iter().flatten().copied().max().unwrap_or(0) + 1;
    new.iter()
        .map(|n| {
            let n = (*n)?;
            Some(match map.iter().find(|&&(mn, _)| mn == n) {
                Some(&(_, o)) => o,
                None => {
                    map.push((n, next));
                    next += 1;
                    next - 1
                }
            })
        })
        .collect()
}

/// How sure the speaker model is that two clips are one voice, from 0 to 1,
/// given its turns over the clips played one after the other: the first
/// clip in `[0, a_end_ms)`, the second in `[b_start_ms, b_end_ms)`. The
/// voice that holds most of the first clip must hold most of the second.
pub fn same_voice(turns: &[Turn], a_end_ms: i64, b_start_ms: i64, b_end_ms: i64) -> f64 {
    same_voices(turns, a_end_ms, &[(b_start_ms, b_end_ms)])[0]
}

/// `same_voice` for one clip followed by several: a score for each span.
pub fn same_voices(turns: &[Turn], a_end_ms: i64, spans: &[(i64, i64)]) -> Vec<f64> {
    let held = |from: i64, to: i64| -> Vec<(i32, i64)> {
        let mut cover: Vec<(i32, i64)> = Vec::new();
        for turn in turns {
            let overlap = turn.t1_ms.min(to) - turn.t0_ms.max(from);
            if overlap > 0 {
                match cover.iter_mut().find(|(s, _)| *s == turn.speaker) {
                    Some((_, ms)) => *ms += overlap,
                    None => cover.push((turn.speaker, overlap)),
                }
            }
        }
        cover
    };
    let top = |cover: &[(i32, i64)]| cover.iter().copied().max_by_key(|&(s, ms)| (ms, -s));
    let a = held(0, a_end_ms);
    let Some((voice, a_ms)) = top(&a) else {
        return vec![0.0; spans.len()];
    };
    // Mostly silence in the first clip: nothing to compare.
    if (a_ms as f64) < a_end_ms as f64 * 0.5 {
        return vec![0.0; spans.len()];
    }
    spans
        .iter()
        .map(|&(from, to)| {
            let b = held(from, to);
            if top(&b).map(|(v, _)| v) != Some(voice) {
                return 0.0;
            }
            let b_ms = b.iter().find(|(s, _)| *s == voice).map_or(0, |&(_, ms)| ms);
            (b_ms as f64 / (to - from).max(1) as f64).min(1.0)
        })
        .collect()
}

/// Two clips count as one voice from this score up.
pub const SAME_VOICE: f64 = 0.6;
/// The best saved voice must beat the next by this much to be trusted.
const VOICE_MARGIN: f64 = 0.15;

/// Which saved voice each unnamed voice is, from `scores[voice][saved]`
/// (`same_voice` for every pair). A voice gets a name only when one saved
/// voice clearly fits it best; no saved voice names two voices.
pub fn pick_voices(scores: &[Vec<f64>]) -> Vec<Option<usize>> {
    let mut picked: Vec<Option<usize>> = scores
        .iter()
        .map(|row| {
            let mut order: Vec<usize> = (0..row.len()).collect();
            order.sort_by(|&a, &b| row[b].total_cmp(&row[a]));
            let best = *order.first()?;
            let second = order.get(1).map_or(0.0, |&i| row[i]);
            (row[best] >= SAME_VOICE && row[best] - second >= VOICE_MARGIN).then_some(best)
        })
        .collect();
    // A saved voice claimed twice names neither.
    for i in 0..picked.len() {
        if let Some(saved) = picked[i] {
            if picked.iter().filter(|p| **p == Some(saved)).count() > 1 {
                for p in picked.iter_mut().filter(|p| **p == Some(saved)) {
                    *p = None;
                }
            }
        }
    }
    picked
}

// ---- More voices than the speaker model holds --------------------------

/// The speaker model keeps four voices apart. A long meeting with more is
/// heard in one-minute windows, short enough that rarely more than four talk in one;
/// neighbouring windows share a stretch, so a voice talking there links
/// them, and voices that never share a window are compared by ear.
pub const MODEL_VOICES: usize = 4;
pub const VOICE_WINDOW_MS: i64 = 60_000;
pub const VOICE_OVERLAP_MS: i64 = 20_000;
/// A voice must talk this long in a shared stretch to link two windows.
const LINK_MS: i64 = 1_500;

/// The windows `[start, end)` a recording of `duration_ms` is heard in.
pub fn voice_windows(duration_ms: i64) -> Vec<(i64, i64)> {
    windows_of(duration_ms, VOICE_WINDOW_MS, VOICE_OVERLAP_MS)
}

pub fn windows_of(duration_ms: i64, window_ms: i64, overlap_ms: i64) -> Vec<(i64, i64)> {
    let step = (window_ms - overlap_ms).max(1);
    let mut out = Vec::new();
    let mut start = 0;
    loop {
        let end = (start + window_ms).min(duration_ms);
        out.push((start, end));
        if end >= duration_ms {
            break;
        }
        start += step;
    }
    out
}

/// One window's turns, in the recording's time, with the window's own
/// voice numbers.
#[derive(Debug, Clone)]
pub struct WindowTurns {
    pub start_ms: i64,
    pub end_ms: i64,
    pub turns: Vec<Turn>,
}

impl WindowTurns {
    fn voices(&self) -> Vec<i32> {
        let mut v: Vec<i32> = self.turns.iter().map(|t| t.speaker).collect();
        v.sort_unstable();
        v.dedup();
        v
    }
}

/// How long each pair of voices talks at the same time within `[from, to)`.
fn together(a: &[Turn], b: &[Turn], from: i64, to: i64) -> Vec<((i32, i32), i64)> {
    let mut out: Vec<((i32, i32), i64)> = Vec::new();
    for x in a {
        for y in b {
            let overlap = x.t1_ms.min(y.t1_ms).min(to) - x.t0_ms.max(y.t0_ms).max(from);
            if overlap > 0 {
                match out.iter_mut().find(|(p, _)| *p == (x.speaker, y.speaker)) {
                    Some((_, ms)) => *ms += overlap,
                    None => out.push(((x.speaker, y.speaker), overlap)),
                }
            }
        }
    }
    out
}

/// The meeting's voices before any are compared by ear: each window's
/// voices, linked to the next window's where they talk in the stretch the
/// two share. `groups[w]` gives each voice of window `w` its group.
#[derive(Debug, Clone, PartialEq)]
pub struct VoiceGroups {
    pub groups: Vec<Vec<(i32, usize)>>,
    pub count: usize,
}

impl VoiceGroups {
    pub fn of(&self, window: usize, voice: i32) -> Option<usize> {
        self.groups
            .get(window)?
            .iter()
            .find(|(v, _)| *v == voice)
            .map(|&(_, g)| g)
    }

    /// The windows each group talks in.
    pub fn windows(&self) -> Vec<Vec<usize>> {
        let mut out = vec![Vec::new(); self.count];
        for (w, voices) in self.groups.iter().enumerate() {
            for &(_, g) in voices {
                if !out[g].contains(&w) {
                    out[g].push(w);
                }
            }
        }
        out
    }
}

pub fn link_windows(windows: &[WindowTurns]) -> VoiceGroups {
    let mut groups: Vec<Vec<(i32, usize)>> = Vec::with_capacity(windows.len());
    let mut count = 0;
    for (w, window) in windows.iter().enumerate() {
        let mut mine: Vec<(i32, usize)> = Vec::new();
        if w > 0 {
            let before = &windows[w - 1];
            let mut pairs = together(&before.turns, &window.turns, window.start_ms, before.end_ms);
            pairs.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
            for ((old, new), ms) in pairs {
                if ms < LINK_MS || mine.iter().any(|(v, _)| *v == new) {
                    continue;
                }
                let Some(group) = groups[w - 1].iter().find(|(v, _)| *v == old).map(|p| p.1) else {
                    continue;
                };
                if mine.iter().any(|&(_, g)| g == group) {
                    continue;
                }
                mine.push((new, group));
            }
        }
        for voice in window.voices() {
            if !mine.iter().any(|(v, _)| *v == voice) {
                mine.push((voice, count));
                count += 1;
            }
        }
        groups.push(mine);
    }
    VoiceGroups { groups, count }
}

/// Each voice linked to the window before: (window, voice, earlier voice).
pub fn window_links(groups: &VoiceGroups) -> Vec<(usize, i32, i32)> {
    let mut out = Vec::new();
    for w in 1..groups.groups.len() {
        for &(voice, g) in &groups.groups[w] {
            if let Some(&(earlier, _)) = groups.groups[w - 1].iter().find(|&&(_, h)| h == g) {
                out.push((w, voice, earlier));
            }
        }
    }
    out
}

/// A link is doubted when the model, hearing the two voices one after the
/// other, scores them below this.
pub const LINK_DOUBT: f64 = 0.3;

/// A stretch `(start, end)` of the recording, in ms.
pub type Span = (i64, i64);

/// What to play to check each link by ear: the later voice's clearest
/// stretch after the shared part, and the earlier voice's before it. None
/// when either has no stretch long enough.
pub fn link_clips(
    windows: &[WindowTurns],
    links: &[(usize, i32, i32)],
    min_ms: i64,
    max_ms: i64,
) -> Vec<Option<(Span, Span)>> {
    links
        .iter()
        .map(|&(w, voice, earlier)| {
            let (now, before) = (windows.get(w)?, windows.get(w.checked_sub(1)?)?);
            let after: Vec<Turn> = now
                .turns
                .iter()
                .filter(|t| t.t0_ms >= before.end_ms)
                .copied()
                .collect();
            let ahead: Vec<Turn> = before
                .turns
                .iter()
                .filter(|t| t.t1_ms <= now.start_ms)
                .copied()
                .collect();
            let slot = |v: i32| usize::try_from(v - 1).ok();
            let a = *clearest_turns(&after, MODEL_VOICES, min_ms, max_ms).get(slot(voice)?)?;
            let b = *clearest_turns(&ahead, MODEL_VOICES, min_ms, max_ms).get(slot(earlier)?)?;
            Some((a?, b?))
        })
        .collect()
}

/// The groups again without the links in `broken` (window, voice): that
/// voice and the windows after it that were linked through it start a new
/// group. Groups are numbered in the order they first talk.
pub fn unlink(groups: &VoiceGroups, broken: &[(usize, i32)]) -> VoiceGroups {
    let mut out: Vec<Vec<(i32, usize)>> = Vec::with_capacity(groups.groups.len());
    let mut count = 0;
    for (w, voices) in groups.groups.iter().enumerate() {
        let mut mine = Vec::new();
        for &(voice, g) in voices {
            let earlier = (w > 0 && !broken.contains(&(w, voice)))
                .then(|| groups.groups[w - 1].iter().position(|&(_, h)| h == g))
                .flatten();
            let group = match earlier {
                Some(i) => out[w - 1][i].1,
                None => {
                    count += 1;
                    count - 1
                }
            };
            mine.push((voice, group));
        }
        out.push(mine);
    }
    VoiceGroups { groups: out, count }
}

/// Joins groups the comparisons say are one voice, the surest first. Two
/// groups that talk in the same window stay apart: the model told them
/// apart there. `pairs` are (group, group, score); returns each group's
/// voice, numbered from 0 in group order.
pub fn join_voices(groups: &VoiceGroups, pairs: &[(usize, usize, f64)]) -> Vec<usize> {
    let mut root: Vec<usize> = (0..groups.count).collect();
    let mut windows = groups.windows();
    fn find(root: &mut [usize], g: usize) -> usize {
        let mut g = g;
        while root[g] != g {
            root[g] = root[root[g]];
            g = root[g];
        }
        g
    }
    let mut sure: Vec<&(usize, usize, f64)> = pairs.iter().filter(|p| p.2 >= SAME_VOICE).collect();
    sure.sort_by(|a, b| b.2.total_cmp(&a.2).then((a.0, a.1).cmp(&(b.0, b.1))));
    for &&(a, b, _) in &sure {
        if a >= groups.count || b >= groups.count {
            continue;
        }
        let (ra, rb) = (find(&mut root, a), find(&mut root, b));
        if ra == rb || windows[ra].iter().any(|w| windows[rb].contains(w)) {
            continue;
        }
        let (keep, gone) = (ra.min(rb), ra.max(rb));
        root[gone] = keep;
        let moved = std::mem::take(&mut windows[gone]);
        windows[keep].extend(moved);
    }
    let mut number: Vec<usize> = Vec::new();
    (0..groups.count)
        .map(|g| {
            let r = find(&mut root, g);
            match number.iter().position(|&n| n == r) {
                Some(i) => i,
                None => {
                    number.push(r);
                    number.len() - 1
                }
            }
        })
        .collect()
}

/// The whole recording's turns from its windows: each window keeps the
/// part up to the middle of the stretch it shares with the next, and each
/// turn takes its voice's number (from 1).
pub fn stitch_turns(windows: &[WindowTurns], groups: &VoiceGroups, voice: &[usize]) -> Vec<Turn> {
    let mut out = Vec::new();
    for (w, window) in windows.iter().enumerate() {
        let from = match w {
            0 => i64::MIN,
            _ => (window.start_ms + windows[w - 1].end_ms) / 2,
        };
        let to = match windows.get(w + 1) {
            Some(next) => (next.start_ms + window.end_ms) / 2,
            None => i64::MAX,
        };
        for turn in &window.turns {
            let (t0, t1) = (turn.t0_ms.max(from), turn.t1_ms.min(to));
            let Some(g) = groups.of(w, turn.speaker) else {
                continue;
            };
            if t1 > t0 {
                out.push(Turn {
                    speaker: voice.get(g).map_or(g, |&v| v) as i32 + 1,
                    t0_ms: t0,
                    t1_ms: t1,
                });
            }
        }
    }
    out.sort_by_key(|t| (t.t0_ms, t.speaker));
    out
}

/// The clearest stretch of each voice in `turns`, at least `min_ms` long:
/// its longest turn that no one talks over, cut to `max_ms`. Indexed by
/// voice number - 1.
pub fn clearest_turns(
    turns: &[Turn],
    voices: usize,
    min_ms: i64,
    max_ms: i64,
) -> Vec<Option<(i64, i64)>> {
    let mut best: Vec<Option<(i64, i64)>> = vec![None; voices];
    for (i, turn) in turns.iter().enumerate() {
        let Some(slot) = usize::try_from(turn.speaker - 1)
            .ok()
            .filter(|&s| s < voices)
        else {
            continue;
        };
        let talked_over = turns.iter().enumerate().any(|(j, o)| {
            j != i && o.speaker != turn.speaker && o.t0_ms < turn.t1_ms && o.t1_ms > turn.t0_ms
        });
        let length = turn.t1_ms - turn.t0_ms;
        if talked_over || length < min_ms {
            continue;
        }
        if best[slot].is_none_or(|(a, b)| b - a < length.min(max_ms)) {
            best[slot] = Some((turn.t0_ms, turn.t0_ms + length.min(max_ms)));
        }
    }
    best
}

/// Pairs of groups worth comparing: they never talk in the same window,
/// and both have a clip.
pub fn voice_pairs(groups: &VoiceGroups, has_clip: &[bool]) -> Vec<(usize, usize)> {
    let windows = groups.windows();
    let mut out = Vec::new();
    for a in 0..groups.count {
        for b in a + 1..groups.count {
            if has_clip.get(a) == Some(&true)
                && has_clip.get(b) == Some(&true)
                && !windows[a].iter().any(|w| windows[b].contains(w))
            {
                out.push((a, b));
            }
        }
    }
    out
}

/// Whether a microphone line is the speakers playing a line of the call back
/// into the microphone: said within two seconds, mostly the same words.
pub fn is_echo(you: (&str, i64, i64), them: (&str, i64, i64)) -> bool {
    let (you_text, you_at, you_end) = you;
    let (them_text, them_at, them_end) = them;
    if you_at > them_end + ECHO_WINDOW_MS || them_at > you_end + ECHO_WINDOW_MS {
        return false;
    }
    let words = |s: &str| -> Vec<String> {
        s.split(|c: char| !c.is_alphanumeric() && c != '\'')
            .filter(|w| !w.is_empty())
            .map(|w| w.to_lowercase())
            .collect()
    };
    let you_words = words(you_text);
    let them_words = words(them_text);
    if you_words.len() < 2 || them_words.is_empty() {
        return false;
    }
    let shared = you_words.iter().filter(|w| them_words.contains(w)).count();
    shared as f64 / you_words.len() as f64 >= 0.6
}

/// A level from 0 to 1 for a meter, from a frame's RMS on a decibel scale
/// (-60 dB to 0 dB).
pub fn level(frame: &[f32]) -> f32 {
    if frame.is_empty() {
        return 0.0;
    }
    let rms = (frame.iter().map(|s| s * s).sum::<f32>() / frame.len() as f32).sqrt();
    let db = 20.0 * rms.max(1e-6).log10();
    ((db + 60.0) / 60.0).clamp(0.0, 1.0)
}

/// Mixes two tracks into one, sample by sample, clipped.
pub fn mix(a: &[i16], b: &[i16]) -> Vec<i16> {
    let len = a.len().max(b.len());
    (0..len)
        .map(|i| {
            let x = *a.get(i).unwrap_or(&0) as i32 + *b.get(i).unwrap_or(&0) as i32;
            x.clamp(i16::MIN as i32, i16::MAX as i32) as i16
        })
        .collect()
}

/// Transcripts longer than this are summarised in parts, then the parts'
/// summaries are combined: local models often read only a few thousand
/// words.
pub const PART_WORDS: usize = 2_500;
/// Ask reads at most this much of a transcript, the lines that match the
/// question first.
pub const ASK_WORDS: usize = 3_000;
/// Shorter transcripts get no summary.
pub const MIN_SUMMARY_WORDS: usize = 40;

/// One transcript line as the prompts see it.
pub struct PromptLine<'a> {
    pub at_ms: i64,
    /// "You", "Them", or a voice's name or number.
    pub who: &'a str,
    pub text: &'a str,
}

/// "12:04" or "1:02:05".
pub fn clock(ms: i64) -> String {
    let secs = ms.max(0) / 1000;
    let (h, m, s) = (secs / 3600, (secs / 60) % 60, secs % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

fn format_line(line: &PromptLine<'_>) -> String {
    format!("[{}] {}: {}", clock(line.at_ms), line.who, line.text)
}

fn word_count(text: &str) -> usize {
    text.split_whitespace().count()
}

pub fn transcript_words(lines: &[PromptLine<'_>]) -> usize {
    lines.iter().map(|l| word_count(l.text)).sum()
}

/// The transcript in parts of about `PART_WORDS` words, cut between lines.
pub fn transcript_parts(lines: &[PromptLine<'_>]) -> Vec<String> {
    let mut parts = Vec::new();
    let mut part = String::new();
    let mut words = 0;
    for line in lines {
        let n = word_count(line.text);
        if words > 0 && words + n > PART_WORDS {
            parts.push(std::mem::take(&mut part));
            words = 0;
        }
        part.push_str(&format_line(line));
        part.push('\n');
        words += n;
    }
    if !part.is_empty() {
        parts.push(part);
    }
    parts
}

pub const SUMMARY_SYSTEM: &str = "You summarise one recorded conversation for the person who recorded it. Lines marked You are that person; every other line is someone else, from the call or the room, marked Them or with their name or a speaker number. Use only what the transcript says. Write in the transcript's language. Answer with JSON only.";

/// At most this many tags are suggested for a note.
pub const TAG_MAX: usize = 3;

/// The request for one transcript, or one part of a long one. `tags_in_use`
/// are the tags on other notes, offered first so the same topic keeps one
/// name.
pub fn summary_request(
    transcript: &str,
    json_hint: bool,
    tags_in_use: &[String],
    people: &[String],
    words: &[String],
    you: Option<&str>,
) -> String {
    let mut request = String::from(
        "Summarise this transcript.\n\n\
         title: a short name for the conversation, three to six words, in sentence case, naming its main topic; no date, no quotes.\n\
         tags: one to three short topic tags, lowercase, one or two words each.\n\
         summary: one sentence of at most 25 words on what the conversation was about and where it landed.\n\
         people: the names of everyone who spoke or was addressed by name; leave out anyone the transcript doesn't name.\n\
         topics: two to five topics in the order they came up, each with a heading of two to six words and one to three sentences on what was said and concluded.\n\
         decisions: what was agreed, each as a short title, a colon, then one sentence on what it means; empty when nothing was.\n\
         actions: tasks someone took on, each with its owner (\"You\" for the person who recorded, a name when the transcript gives one, else \"Them\") and text as a short title, a colon, then exactly what to do.\n\
         details: one entry per topic or notable exchange, in order, each with a heading, three to six sentences on who said what (by name when known, with the numbers, names and reasons given), and at: the time in square brackets of the first line it draws on, copied from the transcript, such as 5:48.\n\
         Spell names, products and organisations the way the people list and the transcript's corrected lines do.\n",
    );
    if let Some(name) = you.map(str::trim).filter(|n| !n.is_empty()) {
        request.push_str(&format!(
            "The person who recorded is {name}; their lines say You. When someone speaks to or about {name}, they mean the person who recorded: tasks for {name} have the owner \"You\", and {name} isn't listed in people.\n"
        ));
    }
    if !people.is_empty() {
        request.push_str("People invited to the meeting: ");
        request.push_str(&people.join(", "));
        request.push('\n');
    }
    if !words.is_empty() {
        request.push_str(
            "Names and terms this person uses, spelled right (the transcript may have them misheard; write them this way): ",
        );
        request.push_str(&words.join("; "));
        request.push('\n');
    }
    if !tags_in_use.is_empty() {
        request.push_str("Tags already in use, to pick from when one fits: ");
        request.push_str(&tags_in_use.join(", "));
        request.push('\n');
    }
    if json_hint {
        request.push_str(
            "\nAnswer exactly like this, with no other text:\n\
             {\"title\": \"…\", \"tags\": [\"…\"], \"summary\": \"…\", \"people\": [\"…\"], \"topics\": [{\"heading\": \"…\", \"text\": \"…\"}], \"decisions\": [\"Title: …\"], \"actions\": [{\"owner\": \"You\", \"text\": \"Title: …\"}], \"details\": [{\"heading\": \"…\", \"text\": \"…\", \"at\": \"5:48\"}]}\n",
        );
    }
    request.push_str("\nTranscript:\n");
    request.push_str(transcript);
    request
}

/// The request that combines the summaries of a long transcript's parts.
pub fn combine_request(
    parts: &[NotesSummary],
    json_hint: bool,
    tags_in_use: &[String],
    words: &[String],
    you: Option<&str>,
) -> String {
    let mut text = String::new();
    for (i, part) in parts.iter().enumerate() {
        text.push_str(&format!("Part {}: {}\n", i + 1, part.summary));
        for t in &part.outline.topics {
            text.push_str(&format!("Topic: {}: {}\n", t.heading, t.text));
        }
        for d in &part.decisions {
            text.push_str(&format!("Decision: {d}\n"));
        }
        for (owner, task) in &part.actions {
            text.push_str(&format!("Action ({owner}): {task}\n"));
        }
    }
    let people: Vec<String> = merge_people(parts.iter().map(|p| p.outline.people.as_slice()));
    summary_request(&text, json_hint, tags_in_use, &people, words, you).replace(
        "Summarise this transcript.",
        "These are summaries of consecutive parts of one long conversation. Combine them into one summary, removing repeats; details may stay empty, since each part keeps its own.",
    )
}

/// Each name once, in the order first seen.
pub fn merge_people<'a>(lists: impl Iterator<Item = &'a [String]>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for name in lists.flatten() {
        let name = name.trim();
        if !name.is_empty() && !out.iter().any(|n| n.eq_ignore_ascii_case(name)) {
            out.push(name.to_string());
        }
    }
    out
}

/// A time written like the transcript's ("5:48", "[1:02:03]") in ms.
pub fn parse_clock(text: &str) -> Option<i64> {
    let text = text
        .trim()
        .trim_matches(|c| c == '[' || c == ']' || c == '(' || c == ')');
    let parts: Vec<i64> = text
        .split(':')
        .map(|p| p.trim().parse::<i64>().ok().filter(|n| *n >= 0))
        .collect::<Option<_>>()?;
    let secs = match parts.as_slice() {
        [m, s] if *s < 60 => m * 60 + s,
        [h, m, s] if *m < 60 && *s < 60 => h * 3600 + m * 60 + s,
        _ => return None,
    };
    Some(secs * 1000)
}

pub fn summary_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "title": { "type": "string" },
            "tags": { "type": "array", "items": { "type": "string" } },
            "summary": { "type": "string" },
            "people": { "type": "array", "items": { "type": "string" } },
            "topics": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "heading": { "type": "string" },
                        "text": { "type": "string" }
                    },
                    "required": ["heading", "text"],
                    "additionalProperties": false
                }
            },
            "decisions": { "type": "array", "items": { "type": "string" } },
            "actions": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "owner": { "type": "string" },
                        "text": { "type": "string" }
                    },
                    "required": ["owner", "text"],
                    "additionalProperties": false
                }
            },
            "details": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "heading": { "type": "string" },
                        "text": { "type": "string" },
                        "at": { "type": "string" }
                    },
                    "required": ["heading", "text", "at"],
                    "additionalProperties": false
                }
            }
        },
        "required": ["title", "tags", "summary", "people", "topics", "decisions", "actions", "details"],
        "additionalProperties": false
    })
}

/// What a summary found: a suggested title and tags, the overview,
/// decisions, (owner, task) pairs, and the longer parts in `outline`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NotesSummary {
    pub title: String,
    pub tags: Vec<String>,
    pub summary: String,
    pub decisions: Vec<String>,
    pub actions: Vec<(String, String)>,
    pub outline: NoteOutline,
}

/// A summary's longer parts: who took part, a short paragraph per topic,
/// and details tied to the moment in the recording they come from.
#[derive(
    Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type,
)]
pub struct NoteOutline {
    #[serde(default)]
    pub people: Vec<String>,
    #[serde(default)]
    pub topics: Vec<OutlineTopic>,
    #[serde(default)]
    pub details: Vec<OutlineDetail>,
}

#[derive(
    Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type,
)]
pub struct OutlineTopic {
    pub heading: String,
    pub text: String,
}

#[derive(
    Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type,
)]
pub struct OutlineDetail {
    pub heading: String,
    pub text: String,
    /// Where in the recording it starts, when the answer gave a time that
    /// fits.
    pub at_ms: Option<i64>,
}

impl NoteOutline {
    /// Drops times past the end of the recording: a detail never points at
    /// audio that isn't there.
    pub fn fit_to(&mut self, last_ms: i64) {
        for d in &mut self.details {
            if d.at_ms.is_some_and(|at| at > last_ms) {
                d.at_ms = None;
            }
        }
    }
}

/// Reads the model's JSON, also when it is wrapped in a code fence or text.
pub fn parse_summary(answer: &str) -> Option<NotesSummary> {
    let start = answer.find('{')?;
    let end = answer.rfind('}')?;
    let value: serde_json::Value = serde_json::from_str(answer.get(start..=end)?).ok()?;
    let summary = value.get("summary")?.as_str()?.trim().to_string();
    let strings = |key: &str| -> Vec<String> {
        value
            .get(key)
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|d| d.as_str())
                    .map(|d| d.trim().to_string())
                    .filter(|d| !d.is_empty())
                    .collect()
            })
            .unwrap_or_default()
    };
    let decisions = strings("decisions");
    let actions = value
        .get("actions")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|item| {
                    let text = item.get("text")?.as_str()?.trim();
                    let owner = item
                        .get("owner")
                        .and_then(|o| o.as_str())
                        .map(str::trim)
                        .filter(|o| !o.is_empty())
                        .unwrap_or("You");
                    (!text.is_empty()).then(|| (owner.to_string(), text.to_string()))
                })
                .collect()
        })
        .unwrap_or_default();
    let title = value
        .get("title")
        .and_then(|t| t.as_str())
        .map(clean_title)
        .unwrap_or_default();
    let tags = clean_tags(&strings("tags"));
    let pairs = |key: &str| -> Vec<(String, String, Option<String>)> {
        value
            .get(key)
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|item| {
                        let heading = item.get("heading")?.as_str()?.trim().to_string();
                        let text = item.get("text")?.as_str()?.trim().to_string();
                        let at = item.get("at").and_then(|v| v.as_str()).map(str::to_string);
                        (!text.is_empty()).then_some((heading, text, at))
                    })
                    .collect()
            })
            .unwrap_or_default()
    };
    let outline = NoteOutline {
        people: merge_people(std::iter::once(strings("people").as_slice())),
        topics: pairs("topics")
            .into_iter()
            .map(|(heading, text, _)| OutlineTopic { heading, text })
            .collect(),
        details: pairs("details")
            .into_iter()
            .map(|(heading, text, at)| OutlineDetail {
                heading,
                text,
                at_ms: at.as_deref().and_then(parse_clock),
            })
            .collect(),
    };
    (!summary.is_empty()).then_some(NotesSummary {
        title,
        tags,
        summary,
        decisions,
        actions,
        outline,
    })
}

/// The longest suggested title kept; longer ones are dropped, not cut.
const TITLE_MAX_CHARS: usize = 70;

/// A suggested title without quotes or a closing period, or empty when it
/// is too long to be a title.
fn clean_title(title: &str) -> String {
    let t = title
        .trim()
        .trim_matches(|c: char| matches!(c, '"' | '\'' | '“' | '”' | '‘' | '’'))
        .trim_end_matches('.')
        .trim();
    if t.chars().count() > TITLE_MAX_CHARS || t.contains('\n') {
        return String::new();
    }
    t.to_string()
}

/// The longest tag kept.
const TAG_MAX_CHARS: usize = 24;

/// Suggested tags without a leading "#", repeats or ones too long, at most
/// [`TAG_MAX`].
fn clean_tags(tags: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for tag in tags {
        let tag = tag.trim().trim_start_matches('#').trim();
        if tag.is_empty() || tag.chars().count() > TAG_MAX_CHARS {
            continue;
        }
        if out.iter().any(|t| t.eq_ignore_ascii_case(tag)) {
            continue;
        }
        out.push(tag.to_string());
        if out.len() == TAG_MAX {
            break;
        }
    }
    out
}

/// Whether a note still has the name Legato gave it when it began, such as
/// "Meeting · Oct 22, 2:29 PM" or "Zoom call · Oct 22, 2:29 PM" from a
/// call's card, so a suggested title may replace it. A title you typed, or
/// a meeting's name from your calendar, stays.
pub fn is_default_title(title: &str) -> bool {
    let Some((kind, when)) = title.split_once(" · ") else {
        return false;
    };
    (matches!(kind, "Meeting" | "Voice memo") || kind.ends_with(" call"))
        && when.chars().any(|c| c.is_ascii_digit())
        && (when.ends_with(" AM") || when.ends_with(" PM"))
}

/// The suggested tags written the way an existing tag is, so "Backend"
/// joins "backend" rather than starting a second one.
pub fn match_existing_tags(suggested: &[String], in_use: &[String]) -> Vec<String> {
    suggested
        .iter()
        .map(|tag| {
            in_use
                .iter()
                .find(|t| t.eq_ignore_ascii_case(tag))
                .cloned()
                .unwrap_or_else(|| tag.clone())
        })
        .collect()
}

pub const FIX_SYSTEM: &str = "You proofread a speech-to-text transcript of one conversation. Speech models mishear names, product names, jargon and acronyms, writing words that sound alike, such as \"cube cuddle\" for \"kubectl\". Find the words that are clearly misheard, judging from the rest of the conversation, its title and the person's own word list, and give the word that was meant. Words in that list are spelled exactly as listed: a person, project, organisation or term from it is never turned into a similar ordinary word, and a misheard form listed for a word is that word wherever the sentence is about it. Fix only misheard words: never rephrase, fix grammar or style, change punctuation, or translate. When unsure, leave the word alone. Answer with JSON only.";

/// At most this many fixes are suggested for one note.
pub const FIX_MAX: usize = 40;

/// The transcript numbered line by line, in parts of about `PART_WORDS`
/// words; the numbers run on across parts.
pub fn numbered_parts(lines: &[PromptLine<'_>]) -> Vec<String> {
    let mut parts = Vec::new();
    let mut part = String::new();
    let mut words = 0;
    for (i, line) in lines.iter().enumerate() {
        let n = word_count(line.text);
        if words > 0 && words + n > PART_WORDS {
            parts.push(std::mem::take(&mut part));
            words = 0;
        }
        part.push_str(&format!("{} {}: {}\n", i + 1, line.who, line.text));
        words += n;
    }
    if !part.is_empty() {
        parts.push(part);
    }
    parts
}

/// A Dictionary word for a note's AI requests: what it is and how it gets
/// misheard, as in "AIPS (a project name; misheard as IPS, IPES)".
pub fn describe_word(text: &str, kind: Option<&str>, heard: &[String]) -> String {
    let mut about: Vec<String> = Vec::new();
    if let Some(kind) = kind {
        about.push(kind.to_string());
    }
    if !heard.is_empty() {
        about.push(format!("misheard as {}", heard.join(", ")));
    }
    if about.is_empty() {
        text.to_string()
    } else {
        format!("{text} ({})", about.join("; "))
    }
}

/// The request for misheard words in one numbered part.
pub fn fix_request(part: &str, title: &str, words: &[String], json_hint: bool) -> String {
    let mut request = format!("Title: {title}\n");
    if !words.is_empty() {
        request.push_str(&format!(
            "Words this person uses, spelled right: {}\n",
            words.join("; ")
        ));
    }
    request.push_str(
        "\nList the misheard words. line: the line's number. heard: the words exactly as that line has them. word: what was meant. An empty list when nothing is misheard.\n",
    );
    if json_hint {
        request.push_str(
            "\nAnswer exactly like this, with no other text:\n\
             {\"fixes\": [{\"line\": 12, \"heard\": \"…\", \"word\": \"…\"}]}\n",
        );
    }
    request.push_str("\nTranscript, one numbered line per row:\n");
    request.push_str(part);
    request
}

pub fn fix_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "fixes": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "line": { "type": "integer" },
                        "heard": { "type": "string" },
                        "word": { "type": "string" }
                    },
                    "required": ["line", "heard", "word"],
                    "additionalProperties": false
                }
            }
        },
        "required": ["fixes"],
        "additionalProperties": false
    })
}

/// A misheard word the model found: the line's index, what it says, and
/// what was meant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineFix {
    pub index: usize,
    pub heard: String,
    pub word: String,
}

/// The fixes in the model's answer that hold up: the line exists and has
/// the heard words, whole, and the word differs. `texts` are the lines in
/// the order they were numbered.
pub fn parse_fixes(answer: &str, texts: &[&str]) -> Vec<LineFix> {
    let (Some(start), Some(end)) = (answer.find('{'), answer.rfind('}')) else {
        return Vec::new();
    };
    let Some(value) = answer
        .get(start..=end)
        .and_then(|json| serde_json::from_str::<serde_json::Value>(json).ok())
    else {
        return Vec::new();
    };
    let Some(items) = value.get("fixes").and_then(|f| f.as_array()) else {
        return Vec::new();
    };
    let mut fixes: Vec<LineFix> = Vec::new();
    for item in items {
        let Some(line) = item.get("line").and_then(|l| l.as_u64()) else {
            continue;
        };
        let text = |key: &str| {
            item.get(key)
                .and_then(|v| v.as_str())
                .map(|v| v.trim().to_string())
                .unwrap_or_default()
        };
        let (heard, word) = (text("heard"), text("word"));
        let Some(index) = (line as usize).checked_sub(1).filter(|&i| i < texts.len()) else {
            continue;
        };
        if heard.is_empty()
            || word.is_empty()
            || heard == word
            || word_count(&heard) > 4
            || word_count(&word) > 6
            || replace_words(texts[index], &heard, &word).is_none()
            || fixes
                .iter()
                .any(|f| f.index == index && f.heard.eq_ignore_ascii_case(&heard))
        {
            continue;
        }
        fixes.push(LineFix { index, heard, word });
        if fixes.len() == FIX_MAX {
            break;
        }
    }
    fixes
}

pub const FOLLOW_UP_SYSTEM: &str = "You write a short follow-up email after a meeting, for the person who recorded it to send to the others. Use only the notes given. Plain text, no subject line, no markdown: a one-line greeting, two or three sentences on what was discussed, then what was agreed and who does what, one per line starting with a dash, and a one-line sign-off without a name. Write in the notes' language.";

/// The request for a follow-up email from a note's summary.
pub fn follow_up_request(title: &str, summary: &NotesSummary, my_notes: &str) -> String {
    let mut request = format!("Meeting: {title}\n\nSummary: {}\n", summary.summary);
    for d in &summary.decisions {
        request.push_str(&format!("Decision: {d}\n"));
    }
    for (owner, task) in &summary.actions {
        request.push_str(&format!("Action ({owner}): {task}\n"));
    }
    let mine = my_notes.trim();
    if !mine.is_empty() {
        request.push_str(&format!("\nTheir own notes:\n{mine}\n"));
    }
    request
}

pub const ASK_SYSTEM: &str = "You answer questions about one recorded conversation, using only its transcript. Lines marked You are the person asking; every other line is someone else, marked Them or with their name or a speaker number. After each statement, put the time of the line it comes from in square brackets, like [12:04]. If the transcript doesn't answer the question, say so in one sentence. Answer in two to four sentences, in the question's language, as plain text.";

const STOPWORDS: &[&str] = &[
    "a", "an", "and", "are", "about", "did", "do", "does", "for", "from", "has", "have", "how",
    "i", "in", "is", "it", "me", "my", "of", "on", "or", "so", "that", "the", "their", "them",
    "they", "this", "to", "was", "we", "were", "what", "when", "where", "which", "who", "why",
    "will", "with", "you",
];

fn keywords(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .map(|w| w.to_lowercase())
        .filter(|w| w.chars().count() > 2 && !STOPWORDS.contains(&w.as_str()))
        .collect()
}

/// The transcript Ask reads: all of it when it's short, else the lines
/// that share words with the question and the lines around them, in order.
pub fn ask_transcript(lines: &[PromptLine<'_>], question: &str) -> String {
    let render = |keep: &[bool]| -> String {
        let mut out = String::new();
        let mut gap = false;
        for (line, &k) in lines.iter().zip(keep) {
            if k {
                if gap && !out.is_empty() {
                    out.push_str("…\n");
                }
                out.push_str(&format_line(line));
                out.push('\n');
                gap = false;
            } else {
                gap = true;
            }
        }
        out
    };
    if transcript_words(lines) <= ASK_WORDS {
        return render(&vec![true; lines.len()]);
    }
    let wanted = keywords(question);
    let mut scored: Vec<(usize, usize)> = lines
        .iter()
        .enumerate()
        .map(|(i, l)| {
            let words = keywords(l.text);
            (i, wanted.iter().filter(|w| words.contains(w)).count())
        })
        .filter(|&(_, score)| score > 0)
        .collect();
    scored.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let mut keep = vec![false; lines.len()];
    let mut words = 0;
    for (i, _) in scored {
        let from = i.saturating_sub(2);
        let to = (i + 2).min(lines.len() - 1);
        for (j, k) in keep.iter_mut().enumerate().take(to + 1).skip(from) {
            if !*k {
                *k = true;
                words += word_count(lines[j].text);
            }
        }
        if words >= ASK_WORDS {
            break;
        }
    }
    // No line matched: the start of the conversation, where it's set up.
    if words == 0 {
        for (j, k) in keep.iter_mut().enumerate() {
            if words >= ASK_WORDS {
                break;
            }
            *k = true;
            words += word_count(lines[j].text);
        }
    }
    render(&keep)
}

pub fn ask_request(transcript: &str, question: &str) -> String {
    format!("Transcript:\n{transcript}\nQuestion: {}", question.trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRAME: usize = 512;

    fn run(pattern: &[(bool, usize)]) -> Vec<Utterance> {
        let mut seg = Segmenter::new();
        let mut out = Vec::new();
        for &(speech, frames) in pattern {
            for _ in 0..frames {
                if let Some(u) = seg.push(&[0.1; FRAME], speech) {
                    out.push(u);
                }
            }
        }
        out.extend(seg.finish());
        out
    }

    fn frames(ms: usize) -> usize {
        ms * RATE / 1000 / FRAME
    }

    #[test]
    fn a_pause_ends_a_line() {
        let lines = run(&[
            (false, frames(1000)),
            (true, frames(2000)),
            (false, frames(1000)),
            (true, frames(1500)),
        ]);
        assert_eq!(lines.len(), 2);
        // The pre-roll reaches back before the first speech.
        assert!(lines[0].start_ms() < 1000 && lines[0].start_ms() >= 650);
        assert!(lines[1].start_ms() > lines[0].end_ms() - 300);
    }

    #[test]
    fn short_pauses_stay_inside_a_line() {
        let lines = run(&[
            (true, frames(1500)),
            (false, frames(300)),
            (true, frames(1500)),
        ]);
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn long_lines_end_at_the_next_short_pause_or_are_cut() {
        let lines = run(&[
            (true, frames(9_000)),
            (false, frames(250)),
            (true, frames(500)),
        ]);
        assert_eq!(lines.len(), 2);
        assert!(lines[0].end_ms() >= 9_000 && lines[0].end_ms() < 9_500);
        let lines = run(&[(true, frames(65_000))]);
        assert_eq!(lines.len(), 5);
        assert!(lines.iter().all(|l| l.samples.len() <= MAX_LINE + FRAME));
    }

    #[test]
    fn the_open_line_grows_until_it_ends() {
        let mut seg = Segmenter::new();
        assert!(seg.open_line().is_none());
        for _ in 0..frames(1000) {
            seg.push(&[0.1; FRAME], true);
        }
        let (start, audio) = seg.open_line().unwrap();
        assert_eq!(start, 0);
        assert_eq!(audio.len(), frames(1000) * FRAME);
        for _ in 0..frames(800) {
            seg.push(&[0.0; FRAME], false);
        }
        assert!(seg.open_line().is_none());
    }

    #[test]
    fn clicks_are_dropped() {
        assert!(run(&[(false, 20), (true, 3), (false, 40)]).is_empty());
    }

    #[test]
    fn echoes_are_close_in_time_and_mostly_the_same_words() {
        let them = (
            "Netbird routes are up. Only the second instance is left.",
            11_000,
            15_000,
        );
        assert!(is_echo(
            (
                "Netbird routes are up, only the second instance left",
                11_400,
                15_200
            ),
            them
        ));
        assert!(!is_echo(
            (
                "Netbird routes are up, only the second instance left",
                19_000,
                22_000
            ),
            them
        ));
        assert!(!is_echo(
            ("Then let's consolidate both on the ARM box", 15_500, 18_000),
            them
        ));
        assert!(!is_echo(("Up.", 11_000, 11_500), them));
    }

    #[test]
    fn levels_and_mixing() {
        assert_eq!(level(&[]), 0.0);
        assert!(level(&[0.5; 100]) > 0.8);
        assert!(level(&[0.0005; 100]) < 0.1);
        assert_eq!(mix(&[30_000, 1], &[10_000]), vec![i16::MAX, 1]);
    }

    fn lines(texts: &[(i64, bool, &'static str)]) -> Vec<PromptLine<'static>> {
        texts
            .iter()
            .map(|&(at_ms, you, text)| PromptLine {
                at_ms,
                who: if you { "You" } else { "Them" },
                text,
            })
            .collect()
    }

    #[test]
    fn transcripts_split_between_lines() {
        let long = "word ".repeat(1_500);
        let long: &'static str = Box::leak(long.into_boxed_str());
        let parts = transcript_parts(&lines(&[
            (0, true, long),
            (1_000, false, long),
            (2_000, true, "Thanks."),
        ]));
        assert_eq!(parts.len(), 2);
        assert!(parts[0].starts_with("[0:00] You: "));
        assert!(parts[1].ends_with("[0:02] You: Thanks.\n"));
    }

    #[test]
    fn summaries_parse_from_fenced_or_plain_json() {
        let fenced = "```json\n{\"summary\": \"Dokploy moves.\", \"decisions\": [\"ARM box\", \"\"], \"actions\": [{\"owner\": \"Samir\", \"text\": \"Move it\"}, {\"text\": \"Send the doc\"}]}\n```";
        let s = parse_summary(fenced).unwrap();
        assert_eq!(s.decisions, vec!["ARM box"]);
        assert_eq!(
            s.actions[1],
            ("You".to_string(), "Send the doc".to_string())
        );
        assert!(parse_summary("I can't help with that.").is_none());
        assert!(parse_summary("{\"summary\": \"\"}").is_none());
    }

    #[test]
    fn summaries_bring_topics_people_and_timed_details() {
        let answer = r#"{"summary": "Repos and a test plan.", "people": ["Sahil", "sahil", "Abhash"],
            "topics": [{"heading": "GIS app", "text": "A map of 3.3 million buildings."}, {"heading": "", "text": ""}],
            "details": [{"heading": "Cloudflare workers", "text": "They switch them.", "at": "[5:48]"},
                        {"heading": "Late", "text": "Past the end.", "at": "1:02:03"},
                        {"heading": "No time", "text": "Fine.", "at": "soon"}]}"#;
        let mut s = parse_summary(answer).unwrap();
        assert_eq!(s.outline.people, vec!["Sahil", "Abhash"]);
        assert_eq!(s.outline.topics.len(), 1);
        assert_eq!(s.outline.details[0].at_ms, Some(348_000));
        assert_eq!(s.outline.details[1].at_ms, Some(3_723_000));
        assert_eq!(s.outline.details[2].at_ms, None);
        s.outline.fit_to(1_200_000);
        assert_eq!(s.outline.details[1].at_ms, None);
        assert_eq!(s.outline.details[0].at_ms, Some(348_000));
    }

    #[test]
    fn clocks_read_like_the_transcript_writes_them() {
        assert_eq!(parse_clock("0:54"), Some(54_000));
        assert_eq!(parse_clock("00:00:54"), Some(54_000));
        assert_eq!(parse_clock("(12:07)"), Some(727_000));
        assert_eq!(parse_clock("5:75"), None);
        assert_eq!(parse_clock("five"), None);
    }

    #[test]
    fn summaries_bring_a_title_and_tags() {
        let answer = r##"{"title": "\"AIPS backend not working.\"", "tags": ["#backend", "Backend", "deployment", "a tag far too long to be a tag at all", "api", "extra"], "summary": "The server is slow.", "decisions": [], "actions": []}"##;
        let s = parse_summary(answer).unwrap();
        assert_eq!(s.title, "AIPS backend not working");
        assert_eq!(s.tags, vec!["backend", "deployment", "api"]);
        // An answer without them still parses.
        let s = parse_summary("{\"summary\": \"Short.\"}").unwrap();
        assert!(s.title.is_empty() && s.tags.is_empty());
        // A paragraph is not a title.
        let long = format!(
            "{{\"title\": \"{}\", \"summary\": \"x\"}}",
            "word ".repeat(20)
        );
        assert!(parse_summary(&long).unwrap().title.is_empty());
    }

    #[test]
    fn only_default_titles_are_replaced() {
        assert!(is_default_title("Meeting · Oct 22, 2:29 PM"));
        assert!(is_default_title("Voice memo · Jan 3, 11:05 AM"));
        assert!(!is_default_title("AIPS backend not working"));
        assert!(!is_default_title("Meeting with Sahil"));
        assert!(!is_default_title("Meeting · design review"));
        assert!(is_default_title("Google Meet call · Oct 23, 3:04 PM"));
        assert!(!is_default_title("Sales call · pricing"));
    }

    #[test]
    fn suggested_tags_take_the_existing_spelling() {
        let in_use = vec!["Backend".to_string(), "clients".to_string()];
        assert_eq!(
            match_existing_tags(&["backend".into(), "api".into()], &in_use),
            vec!["Backend", "api"]
        );
    }

    #[test]
    fn the_summary_request_offers_tags_in_use() {
        assert_eq!(
            describe_word(
                "AIPS",
                Some("a project name"),
                &["IPS".into(), "IPES".into()]
            ),
            "AIPS (a project name; misheard as IPS, IPES)"
        );
        assert_eq!(describe_word("Datev", None, &[]), "Datev");
        assert!(summary_request("You: hi", false, &[], &[], &["AIPS (a project name)".into()], None)
            .contains("spelled right (the transcript may have them misheard; write them this way): AIPS (a project name)"));
        let r = summary_request(
            "You: hi",
            false,
            &["backend".into(), "clients".into()],
            &[],
            &[],
            None,
        );
        assert!(r.contains("Tags already in use, to pick from when one fits: backend, clients"));
        assert!(!summary_request("You: hi", false, &[], &[], &[], None).contains("already in use"));
        assert!(summary_request("You: hi", true, &[], &[], &[], None).contains("\"title\""));
        assert!(
            summary_request("You: hi", false, &[], &["Sahil Khan".into()], &[], None)
                .contains("People invited to the meeting: Sahil Khan\n")
        );
        let mine = summary_request("You: hi", false, &[], &[], &[], Some(" Abhash "));
        assert!(mine.contains("The person who recorded is Abhash; their lines say You."));
        assert!(mine.contains("tasks for Abhash have the owner \"You\""));
        assert!(
            !summary_request("You: hi", false, &[], &[], &[], Some("  "))
                .contains("person who recorded is")
        );
    }

    #[test]
    fn ask_reads_matching_lines_of_long_transcripts() {
        let filler = "nothing much here really ".repeat(60);
        let filler: &'static str = Box::leak(filler.into_boxed_str());
        let mut t: Vec<(i64, bool, &'static str)> =
            (0..60).map(|i| (i * 10_000, i % 2 == 0, filler)).collect();
        t[40] = (400_000, false, "Blocked on the ADB API keys.");
        let all = lines(&t);
        let text = ask_transcript(&all, "What's blocked on the keys?");
        assert!(text.contains("[6:40] Them: Blocked on the ADB API keys."));
        assert!(text.starts_with("[6:20]"));
        let short = lines(&[(4_000, true, "Okay, let's start.")]);
        assert_eq!(
            ask_transcript(&short, "anything"),
            "[0:04] You: Okay, let's start.\n"
        );
    }

    #[test]
    fn words_are_replaced_whole_and_in_any_case() {
        assert_eq!(
            replace_words(
                "Ship cubernetes. Cubernetes again",
                "cubernetes",
                "Kubernetes"
            )
            .as_deref(),
            Some("Ship Kubernetes. Kubernetes again")
        );
        assert_eq!(
            replace_words("ask cool if I today", "cool if I", "Coolify").as_deref(),
            Some("ask Coolify today")
        );
        // Not inside another word.
        assert_eq!(replace_words("scrumptious", "scrum", "Scrum"), None);
        assert_eq!(
            replace_words("nothing here", "cubernetes", "Kubernetes"),
            None
        );
        assert_eq!(replace_words("text", " ", "x"), None);
    }

    #[test]
    fn fixes_that_dont_hold_up_are_dropped() {
        let texts = [
            "we deploy with cube cuddle today",
            "ask Ronja about it",
            "fine",
        ];
        let answer = r#"Sure: {"fixes": [
            {"line": 1, "heard": "cube cuddle", "word": "kubectl"},
            {"line": 1, "heard": "Cube Cuddle", "word": "kubectl"},
            {"line": 2, "heard": "Ronja", "word": "Ronja"},
            {"line": 2, "heard": "Ron", "word": "Rohan"},
            {"line": 3, "heard": "nothing", "word": "else"},
            {"line": 9, "heard": "fine", "word": "find"},
            {"line": 0, "heard": "we", "word": "We"}
        ]}"#;
        assert_eq!(
            parse_fixes(answer, &texts),
            vec![LineFix {
                index: 0,
                heard: "cube cuddle".into(),
                word: "kubectl".into()
            }]
        );
        assert!(parse_fixes("no json here", &texts).is_empty());
    }

    #[test]
    fn numbered_parts_count_on_across_parts() {
        let long = "word ".repeat(PART_WORDS);
        let lines = [
            PromptLine {
                at_ms: 0,
                who: "You",
                text: &long,
            },
            PromptLine {
                at_ms: 1,
                who: "Priya",
                text: "short",
            },
        ];
        let parts = numbered_parts(&lines);
        assert_eq!(parts.len(), 2);
        assert!(parts[0].starts_with("1 You: word"));
        assert_eq!(parts[1], "2 Priya: short\n");
    }

    #[test]
    fn lines_go_to_the_speaker_who_says_most_of_them() {
        let turn = |speaker, t0_ms, t1_ms| Turn {
            speaker,
            t0_ms,
            t1_ms,
        };
        let turns = [
            turn(3, 0, 4_000),
            turn(1, 4_000, 9_000),
            turn(3, 9_500, 12_000),
            turn(1, 11_000, 11_400),
        ];
        let lines = [
            (0, 3_800),
            (4_200, 8_000),
            (3_000, 6_000),
            (9_600, 12_000),
            (20_000, 22_000),
        ];
        // Speaker 3 talks first, so becomes 1.
        assert_eq!(
            assign_speakers(&lines, &turns),
            vec![Some(1), Some(2), Some(2), Some(1), None]
        );
        // One voice: nothing to tell apart.
        assert_eq!(
            assign_speakers(&lines[..1], &[turn(2, 0, 4_000)]),
            vec![None]
        );
        // Too little of the first line covered.
        assert_eq!(
            assign_speakers(
                &[(0, 10_000), (10_000, 12_000), (12_000, 14_000)],
                &[
                    turn(1, 0, 2_000),
                    turn(2, 10_000, 12_000),
                    turn(1, 12_000, 14_000)
                ]
            ),
            vec![None, Some(1), Some(2)]
        );
    }

    #[test]
    fn a_fresh_pass_keeps_the_numbers_people_see() {
        // The new pass calls the voices 2 and 1, and finds a third.
        let old = [Some(1), Some(2), Some(1), None, Some(2)];
        let new = [Some(2), Some(1), Some(2), Some(3), Some(1)];
        assert_eq!(
            keep_numbers(&old, &new),
            vec![Some(1), Some(2), Some(1), Some(3), Some(2)]
        );
        // The first pass keeps its own numbers.
        assert_eq!(
            keep_numbers(&[None, None], &[Some(1), Some(2)]),
            vec![Some(1), Some(2)]
        );
        // A voice the new pass doesn't hear any more leaves its number free.
        assert_eq!(
            keep_numbers(
                &[Some(1), Some(3), Some(3), Some(2)],
                &[Some(1), Some(2), Some(2), Some(2)]
            ),
            vec![Some(1), Some(3), Some(3), Some(3)]
        );
    }

    #[test]
    fn two_clips_of_one_voice_score_high() {
        let turn = |speaker, t0_ms, t1_ms| Turn {
            speaker,
            t0_ms,
            t1_ms,
        };
        // 0–5 s, a gap, then 5.6–10.6 s.
        let same = [turn(1, 100, 4_900), turn(1, 5_700, 10_400)];
        assert!(same_voice(&same, 5_000, 5_600, 10_600) > 0.9);
        let different = [turn(1, 100, 4_900), turn(2, 5_700, 10_400)];
        assert_eq!(same_voice(&different, 5_000, 5_600, 10_600), 0.0);
        // The first clip barely heard: no answer.
        assert_eq!(
            same_voice(
                &[turn(1, 0, 1_000), turn(1, 5_700, 10_400)],
                5_000,
                5_600,
                10_600
            ),
            0.0
        );
    }

    #[test]
    fn a_name_needs_a_clear_best_match() {
        assert_eq!(
            pick_voices(&[
                vec![0.9, 0.1],
                vec![0.2, 0.8],
                vec![0.65, 0.6],
                vec![0.3, 0.2]
            ]),
            vec![Some(0), Some(1), None, None]
        );
        // Both voices sound like the same saved one: neither is named.
        assert_eq!(pick_voices(&[vec![0.9], vec![0.8]]), vec![None, None]);
        assert_eq!(pick_voices(&[vec![]]), vec![None]);
    }

    fn turn(speaker: i32, t0_s: i64, t1_s: i64) -> Turn {
        Turn {
            speaker,
            t0_ms: t0_s * 1000,
            t1_ms: t1_s * 1000,
        }
    }

    #[test]
    fn long_meetings_are_heard_in_overlapping_windows() {
        assert_eq!(voice_windows(50_000), vec![(0, 50_000)]);
        assert_eq!(voice_windows(90_000), vec![(0, 60_000), (40_000, 90_000)]);
        assert_eq!(
            voice_windows(150_000),
            vec![
                (0, 60_000),
                (40_000, 100_000),
                (80_000, 140_000),
                (120_000, 150_000)
            ]
        );
        assert_eq!(windows_of(300_000, 120_000, 30_000).len(), 3);
    }

    #[test]
    fn one_clip_is_compared_with_several() {
        // A in 0-5 s, then three clips; the second is A's voice again.
        let t = [
            turn(1, 0, 5),
            turn(2, 6, 10),
            turn(1, 11, 15),
            turn(3, 16, 20),
        ];
        let scores = same_voices(
            &t,
            5_000,
            &[(5_600, 10_000), (10_600, 15_000), (15_600, 20_000)],
        );
        assert_eq!(scores[0], 0.0);
        assert!(scores[1] > 0.9);
        assert_eq!(scores[2], 0.0);
    }

    fn three_windows() -> Vec<WindowTurns> {
        vec![
            // Ana, then Ben, then Cy, who's still talking into window 2.
            WindowTurns {
                start_ms: 0,
                end_ms: 120_000,
                turns: vec![turn(1, 0, 50), turn(2, 50, 95), turn(3, 95, 120)],
            },
            // Cy carries on, then Dee, a new voice.
            WindowTurns {
                start_ms: 90_000,
                end_ms: 210_000,
                turns: vec![turn(1, 92, 95), turn(2, 95, 150), turn(3, 150, 210)],
            },
            // Dee isn't heard in the shared stretch; Ana is back.
            WindowTurns {
                start_ms: 180_000,
                end_ms: 300_000,
                turns: vec![turn(1, 212, 300)],
            },
        ]
    }

    #[test]
    fn voices_talking_in_a_shared_stretch_link_two_windows() {
        let groups = link_windows(&three_windows());
        // Window 1's voice 1 is Ben (talking 92-95 s), voice 2 is Cy.
        assert_eq!(groups.of(1, 1), groups.of(0, 2));
        assert_eq!(groups.of(1, 2), groups.of(0, 3));
        assert_eq!(groups.of(1, 3), Some(3));
        // Nothing links window 3's voice: it starts a group of its own.
        assert_eq!(groups.of(2, 1), Some(4));
        assert_eq!(groups.count, 5);
        assert_eq!(groups.windows()[2], vec![0, 1]);
    }

    #[test]
    fn voices_join_by_ear_only_if_they_never_share_a_window() {
        let groups = link_windows(&three_windows());
        // Window 3's voice sounds like Ana (group 0), and also like Cy
        // (group 2), who shares no window with it either; Ana is surer.
        // Ben and Dee share window 2, so a high score can't join them.
        let voice = join_voices(
            &groups,
            &[(2, 4, 0.7), (0, 4, 0.9), (1, 3, 0.95), (0, 3, 0.4)],
        );
        assert_eq!(voice[4], voice[0]);
        assert_ne!(voice[4], voice[2]);
        assert_ne!(voice[1], voice[3]);
        assert_eq!(voice, vec![0, 1, 2, 3, 0]);
        let turns = stitch_turns(&three_windows(), &groups, &voice);
        // Cy's turn is cut where the windows meet (105 s) and goes on in
        // window 2 under the same number.
        assert!(turns.contains(&turn(3, 95, 105)));
        assert!(turns.contains(&turn(3, 105, 150)));
        // Dee's turn ends where windows 2 and 3 meet (195 s); Ana's comes
        // back as voice 1.
        assert!(turns.contains(&turn(4, 150, 195)));
        assert_eq!(turns.last(), Some(&turn(1, 212, 300)));
    }

    #[test]
    fn voices_to_compare_have_a_clip_and_no_shared_window() {
        let groups = link_windows(&three_windows());
        let pairs = voice_pairs(&groups, &[true, true, true, true, true]);
        assert!(pairs.contains(&(0, 4)));
        assert!(pairs.contains(&(2, 4)));
        assert!(!pairs.contains(&(1, 3)));
        assert!(!pairs.contains(&(0, 1)));
        assert!(!voice_pairs(&groups, &[false, true, true, true, true]).contains(&(0, 4)));
    }

    #[test]
    fn a_voice_is_clearest_where_no_one_talks_over_it() {
        let t = [
            turn(1, 0, 3),
            turn(2, 2, 12),
            turn(1, 12, 16),
            turn(2, 20, 21),
        ];
        let clips = clearest_turns(&t, 2, 2_500, 6_000);
        assert_eq!(clips[0], Some((12_000, 16_000)));
        // Voice 2's long turn is talked over at the start; its clean one is
        // too short.
        assert_eq!(clips[1], None);
    }

    #[test]
    fn a_link_that_fails_by_ear_starts_a_new_group() {
        let groups = link_windows(&three_windows());
        let links = window_links(&groups);
        assert_eq!(links.len(), 2);
        assert!(links.contains(&(1, 1, 2)) && links.contains(&(1, 2, 3)));
        // Window 2's voice 2 doesn't sound like window 1's voice 3.
        let apart = unlink(&groups, &[(1, 2)]);
        assert_eq!(apart.count, 6);
        assert_eq!(apart.of(1, 1), groups.of(0, 2));
        assert_ne!(apart.of(1, 2), apart.of(0, 3));
        assert_eq!(unlink(&groups, &[]), groups);
    }
}
