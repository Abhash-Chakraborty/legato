//! Calendars from their secret iCal addresses: reading the ICS text,
//! repeating events by their rules, and finding the meeting link. Pure
//! functions with the local time zone passed in, tested without the app.

use chrono::{
    Datelike, Days, Duration, Months, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Weekday,
};
use serde::{Deserialize, Serialize};
use specta::Type;

/// When an event starts or ends, as the file wrote it.
#[derive(Clone, Debug, PartialEq)]
pub enum Stamp {
    /// Ends in Z.
    Utc(NaiveDateTime),
    /// With a TZID: an IANA name from Google, often a Windows name from
    /// Outlook.
    Zoned(NaiveDateTime, String),
    /// Neither: the reader's own time.
    Floating(NaiveDateTime),
    /// A whole day.
    Day(NaiveDate),
}

impl Stamp {
    fn naive(&self) -> NaiveDateTime {
        match self {
            Stamp::Utc(n) | Stamp::Zoned(n, _) | Stamp::Floating(n) => *n,
            Stamp::Day(d) => d.and_time(NaiveTime::MIN),
        }
    }

    fn with(&self, n: NaiveDateTime) -> Stamp {
        match self {
            Stamp::Utc(_) => Stamp::Utc(n),
            Stamp::Zoned(_, tz) => Stamp::Zoned(n, tz.clone()),
            Stamp::Floating(_) => Stamp::Floating(n),
            Stamp::Day(_) => Stamp::Day(n.date()),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Freq {
    Daily,
    Weekly,
    Monthly,
    Yearly,
}

/// An RRULE, the parts calendars write for meetings.
#[derive(Clone, Debug, PartialEq)]
pub struct Rule {
    pub freq: Freq,
    pub interval: u32,
    pub count: Option<u32>,
    pub until: Option<Stamp>,
    /// Weekdays, with a position for monthly rules ("2TU", "-1FR").
    pub by_day: Vec<(Option<i32>, Weekday)>,
    pub by_month_day: Vec<i32>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Event {
    pub uid: String,
    pub summary: String,
    pub location: String,
    pub description: String,
    pub url: String,
    pub conference: String,
    pub start: Option<Stamp>,
    pub end: Option<Stamp>,
    pub duration: Option<Duration>,
    pub rule: Option<Rule>,
    pub exdates: Vec<NaiveDateTime>,
    /// Set on a changed occurrence of a repeating event: the start it
    /// replaces.
    pub recurrence_id: Option<NaiveDateTime>,
    pub cancelled: bool,
}

/// One meeting on the calendar, as Legato shows it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct Meeting {
    pub uid: String,
    pub title: String,
    /// Unix seconds.
    pub start: i64,
    pub end: i64,
    pub all_day: bool,
    /// A Meet, Zoom, Teams or Webex link, when the event has one.
    pub link: Option<String>,
    pub location: Option<String>,
    /// The calendar's name in Legato.
    pub calendar: String,
}

/// Joins folded lines: a line starting with a space or tab continues the
/// one before.
fn unfold(text: &str) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for raw in text.split('\n') {
        let raw = raw.strip_suffix('\r').unwrap_or(raw);
        if let Some(rest) = raw.strip_prefix([' ', '\t']) {
            if let Some(last) = lines.last_mut() {
                last.push_str(rest);
                continue;
            }
        }
        lines.push(raw.to_string());
    }
    lines
}

/// A property's parameters, names upper-cased.
type Params = Vec<(String, String)>;

/// "DTSTART;TZID=Europe/Berlin:20261020T140000" → ("DTSTART", [("TZID",
/// "Europe/Berlin")], "20261020T140000"). Colons inside quoted parameters
/// don't end the name.
fn split(line: &str) -> Option<(String, Params, &str)> {
    let mut quoted = false;
    let colon = line.char_indices().find_map(|(i, c)| {
        if c == '"' {
            quoted = !quoted;
        }
        (c == ':' && !quoted).then_some(i)
    })?;
    let (head, value) = (&line[..colon], &line[colon + 1..]);
    let mut parts = head.split(';');
    let name = parts.next()?.to_ascii_uppercase();
    let params = parts
        .filter_map(|p| {
            let (k, v) = p.split_once('=')?;
            Some((k.to_ascii_uppercase(), v.trim_matches('"').to_string()))
        })
        .collect();
    Some((name, params, value))
}

fn unescape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') | Some('N') => out.push('\n'),
                Some(other) => out.push(other),
                None => {}
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn date(v: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(v.get(..8)?, "%Y%m%d").ok()
}

fn stamp(value: &str, params: &[(String, String)]) -> Option<Stamp> {
    let value = value.trim();
    let param = |k: &str| params.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str());
    if param("VALUE") == Some("DATE") || value.len() == 8 {
        return date(value).map(Stamp::Day);
    }
    let (body, utc) = match value.strip_suffix(['Z', 'z']) {
        Some(b) => (b, true),
        None => (value, false),
    };
    let n = NaiveDateTime::parse_from_str(body, "%Y%m%dT%H%M%S").ok()?;
    Some(if utc {
        Stamp::Utc(n)
    } else if let Some(tz) = param("TZID") {
        Stamp::Zoned(n, tz.to_string())
    } else {
        Stamp::Floating(n)
    })
}

/// "PT1H30M", "P1D", "-PT15M".
fn duration(v: &str) -> Option<Duration> {
    let (sign, v) = match v.strip_prefix('-') {
        Some(rest) => (-1, rest),
        None => (1, v.strip_prefix('+').unwrap_or(v)),
    };
    let v = v.strip_prefix('P')?;
    let mut total = 0i64;
    let mut number = String::new();
    // Checked throughout: a hostile feed's "PT9223372036854775807S" must
    // not take the calendar down.
    for c in v.chars() {
        match c {
            '0'..='9' => number.push(c),
            'T' => {}
            unit => {
                let n: i64 = number.parse().ok()?;
                number.clear();
                let unit = match unit {
                    'W' => 7 * 86_400,
                    'D' => 86_400,
                    'H' => 3_600,
                    'M' => 60,
                    'S' => 1,
                    _ => return None,
                };
                total = total.checked_add(n.checked_mul(unit)?)?;
            }
        }
    }
    Duration::try_seconds(sign * total)
}

fn weekday(code: &str) -> Option<Weekday> {
    Some(match code {
        "MO" => Weekday::Mon,
        "TU" => Weekday::Tue,
        "WE" => Weekday::Wed,
        "TH" => Weekday::Thu,
        "FR" => Weekday::Fri,
        "SA" => Weekday::Sat,
        "SU" => Weekday::Sun,
        _ => return None,
    })
}

fn rule(value: &str) -> Option<Rule> {
    let mut r = Rule {
        freq: Freq::Daily,
        interval: 1,
        count: None,
        until: None,
        by_day: Vec::new(),
        by_month_day: Vec::new(),
    };
    let mut freq = None;
    for part in value.split(';') {
        let (k, v) = part.split_once('=')?;
        match k.to_ascii_uppercase().as_str() {
            "FREQ" => {
                freq = Some(match v {
                    "DAILY" => Freq::Daily,
                    "WEEKLY" => Freq::Weekly,
                    "MONTHLY" => Freq::Monthly,
                    "YEARLY" => Freq::Yearly,
                    // Hourly and finer don't happen to meetings.
                    _ => return None,
                })
            }
            "INTERVAL" => r.interval = v.parse().ok().filter(|n| *n > 0)?,
            "COUNT" => r.count = v.parse().ok(),
            "UNTIL" => r.until = stamp(v, &[]),
            "BYDAY" => {
                for d in v.split(',') {
                    let d = d.trim();
                    let split = d.len().checked_sub(2)?;
                    let n = d.get(..split)?.parse::<i32>().ok();
                    r.by_day.push((n, weekday(d.get(split..)?)?));
                }
            }
            "BYMONTHDAY" => r.by_month_day = v.split(',').filter_map(|d| d.parse().ok()).collect(),
            _ => {}
        }
    }
    r.freq = freq?;
    Some(r)
}

/// The events in an ICS file. Anything it can't read is left out rather
/// than failing the calendar.
pub fn parse(ics: &str) -> Vec<Event> {
    let mut events = Vec::new();
    let mut current: Option<Event> = None;
    // Alarms inside an event have their own DESCRIPTION; skip them.
    let mut nested = 0usize;
    for line in unfold(ics) {
        let Some((name, params, value)) = split(&line) else {
            continue;
        };
        match (name.as_str(), value.trim().to_ascii_uppercase().as_str()) {
            ("BEGIN", "VEVENT") => {
                current = Some(Event::default());
                nested = 0;
                continue;
            }
            ("END", "VEVENT") => {
                if let Some(e) = current.take().filter(|e| e.start.is_some()) {
                    events.push(e);
                }
                continue;
            }
            ("BEGIN", _) if current.is_some() => {
                nested += 1;
                continue;
            }
            ("END", _) if current.is_some() => {
                nested = nested.saturating_sub(1);
                continue;
            }
            _ => {}
        }
        let Some(e) = current.as_mut() else {
            continue;
        };
        if nested > 0 {
            continue;
        }
        match name.as_str() {
            "UID" => e.uid = value.trim().to_string(),
            "SUMMARY" => e.summary = unescape(value).trim().to_string(),
            "LOCATION" => e.location = unescape(value).trim().to_string(),
            "DESCRIPTION" => e.description = unescape(value),
            "URL" => e.url = value.trim().to_string(),
            "X-GOOGLE-CONFERENCE" => e.conference = value.trim().to_string(),
            "DTSTART" => e.start = stamp(value, &params),
            "DTEND" => e.end = stamp(value, &params),
            "DURATION" => e.duration = duration(value.trim()),
            "RRULE" => e.rule = rule(value.trim()),
            "EXDATE" => e.exdates.extend(
                value
                    .split(',')
                    .filter_map(|v| stamp(v, &params))
                    .map(|s| s.naive()),
            ),
            "RECURRENCE-ID" => e.recurrence_id = stamp(value, &params).map(|s| s.naive()),
            "STATUS" => e.cancelled = value.trim().eq_ignore_ascii_case("CANCELLED"),
            _ => {}
        }
    }
    events
}

/// Windows time zone names Outlook writes, for the places most meetings
/// happen; others fall back to the reader's own zone.
fn iana(tz: &str) -> Option<chrono_tz::Tz> {
    if let Ok(z) = tz.parse::<chrono_tz::Tz>() {
        return Some(z);
    }
    let name = match tz {
        "UTC" | "Coordinated Universal Time" | "GMT" => "UTC",
        "W. Europe Standard Time" => "Europe/Berlin",
        "Central Europe Standard Time" => "Europe/Budapest",
        "Central European Standard Time" => "Europe/Warsaw",
        "Romance Standard Time" => "Europe/Paris",
        "GMT Standard Time" => "Europe/London",
        "E. Europe Standard Time" => "Europe/Chisinau",
        "FLE Standard Time" => "Europe/Kiev",
        "GTB Standard Time" => "Europe/Bucharest",
        "Russian Standard Time" => "Europe/Moscow",
        "India Standard Time" => "Asia/Kolkata",
        "Arabian Standard Time" => "Asia/Dubai",
        "Singapore Standard Time" => "Asia/Singapore",
        "China Standard Time" => "Asia/Shanghai",
        "Tokyo Standard Time" => "Asia/Tokyo",
        "Korea Standard Time" => "Asia/Seoul",
        "AUS Eastern Standard Time" => "Australia/Sydney",
        "Eastern Standard Time" => "America/New_York",
        "Central Standard Time" => "America/Chicago",
        "Mountain Standard Time" => "America/Denver",
        "Pacific Standard Time" => "America/Los_Angeles",
        "SA Pacific Standard Time" => "America/Bogota",
        "E. South America Standard Time" => "America/Sao_Paulo",
        _ => return None,
    };
    name.parse().ok()
}

/// Unix seconds for a stamp; `local` turns the reader's wall time into
/// seconds.
pub fn resolve(s: &Stamp, local: &dyn Fn(NaiveDateTime) -> Option<i64>) -> Option<i64> {
    match s {
        Stamp::Utc(n) => Some(n.and_utc().timestamp()),
        Stamp::Zoned(n, tz) => match iana(tz) {
            Some(z) => z
                .from_local_datetime(n)
                .earliest()
                .map(|t| t.timestamp())
                // Inside a spring-forward gap: an hour later is real.
                .or_else(|| {
                    z.from_local_datetime(&(*n + Duration::hours(1)))
                        .earliest()
                        .map(|t| t.timestamp())
                }),
            None => local(*n),
        },
        Stamp::Floating(n) => local(*n),
        Stamp::Day(d) => local(d.and_time(NaiveTime::MIN)),
    }
}

/// The nth (from 1, or from the end when negative) `day` of a month.
fn nth_weekday(year: i32, month: u32, n: i32, day: Weekday) -> Option<NaiveDate> {
    if n > 0 {
        NaiveDate::from_weekday_of_month_opt(year, month, day, n as u8)
    } else {
        let first = NaiveDate::from_ymd_opt(year, month, 1)?;
        let last = first.checked_add_months(Months::new(1))?.pred_opt()?;
        let back = (last.weekday().num_days_from_monday() + 7 - day.num_days_from_monday()) % 7;
        let d = last.checked_sub_days(Days::new(back as u64))?;
        d.checked_sub_days(Days::new(7 * (n.unsigned_abs() as u64 - 1)))
            .filter(|d| d.month() == month)
    }
}

/// The days of one period (a day, a week, a month or a year, counted from
/// the first) that the rule picks, in order.
fn period(rule: &Rule, first: NaiveDate, index: u32) -> Vec<NaiveDate> {
    let step = index.saturating_mul(rule.interval);
    let plain_days: Vec<Weekday> = rule
        .by_day
        .iter()
        .filter(|(n, _)| n.is_none())
        .map(|(_, d)| *d)
        .collect();
    match rule.freq {
        Freq::Daily => first
            .checked_add_days(Days::new(step as u64))
            .filter(|d| plain_days.is_empty() || plain_days.contains(&d.weekday()))
            .into_iter()
            .collect(),
        Freq::Weekly => {
            let monday = first
                .checked_sub_days(Days::new(first.weekday().num_days_from_monday() as u64))
                .and_then(|m| m.checked_add_days(Days::new(7 * step as u64)));
            let Some(monday) = monday else {
                return Vec::new();
            };
            let mut days: Vec<Weekday> = if rule.by_day.is_empty() {
                vec![first.weekday()]
            } else {
                rule.by_day.iter().map(|(_, d)| *d).collect()
            };
            days.sort_by_key(|d| d.num_days_from_monday());
            days.dedup();
            days.iter()
                .filter_map(|d| monday.checked_add_days(Days::new(d.num_days_from_monday() as u64)))
                .collect()
        }
        Freq::Monthly => {
            let Some(month) = first
                .with_day(1)
                .and_then(|m| m.checked_add_months(Months::new(step)))
            else {
                return Vec::new();
            };
            let (y, m) = (month.year(), month.month());
            let days_in = month
                .checked_add_months(Months::new(1))
                .and_then(|n| n.pred_opt())
                .map_or(28, |d| d.day() as i32);
            let mut out: Vec<NaiveDate> = Vec::new();
            for (n, d) in &rule.by_day {
                match n {
                    Some(n) => out.extend(nth_weekday(y, m, *n, *d)),
                    None => out.extend(
                        (1..=days_in as u32)
                            .filter_map(|day| NaiveDate::from_ymd_opt(y, m, day))
                            .filter(|x| x.weekday() == *d),
                    ),
                }
            }
            for md in &rule.by_month_day {
                let day = if *md < 0 { days_in + md + 1 } else { *md };
                out.extend(NaiveDate::from_ymd_opt(y, m, day.max(0) as u32));
            }
            if rule.by_day.is_empty() && rule.by_month_day.is_empty() {
                out.extend(NaiveDate::from_ymd_opt(y, m, first.day()));
            }
            out.sort();
            out.dedup();
            out
        }
        Freq::Yearly => {
            NaiveDate::from_ymd_opt(first.year() + step as i32, first.month(), first.day())
                .into_iter()
                .collect()
        }
    }
}

/// The starts of a repeating event, in its own wall time, until `to` (Unix
/// seconds) or the rule ends.
fn starts(
    start: &Stamp,
    rule: &Rule,
    to: i64,
    local: &dyn Fn(NaiveDateTime) -> Option<i64>,
) -> Vec<NaiveDateTime> {
    let first = start.naive();
    let until = rule.until.as_ref().and_then(|u| resolve(u, local));
    let mut out = Vec::new();
    let mut generated = 0u32;
    // Enough for a daily meeting over many years; a broken rule stops here.
    for index in 0..20_000u32 {
        let days = period(rule, first.date(), index);
        let mut past_end = false;
        for day in days {
            let at = day.and_time(first.time());
            if at < first {
                continue;
            }
            let secs = resolve(&start.with(at), local).unwrap_or(i64::MAX);
            if secs > to || until.is_some_and(|u| secs > u) {
                past_end = true;
                break;
            }
            if rule.count.is_some_and(|c| generated >= c) {
                return out;
            }
            generated += 1;
            out.push(at);
        }
        if past_end {
            break;
        }
    }
    out
}

/// Meeting services by host (a subdomain counts) and path.
const LINK_HOSTS: &[(&str, &str)] = &[
    ("meet.google.com", "/"),
    ("zoom.us", "/j/"),
    ("zoom.us", "/my/"),
    ("teams.microsoft.com", "/l/meetup-join/"),
    ("teams.live.com", "/meet/"),
    ("webex.com", "/"),
];

/// Whether an address is a meeting service's: its host, not text anywhere
/// in it, so `https://evil.example/?zoom.us/j/1` doesn't pass.
fn is_meeting_link(url: &str) -> bool {
    let Ok(url) = url::Url::parse(url) else {
        return false;
    };
    let Some(host) = url.host_str() else {
        return false;
    };
    url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && LINK_HOSTS.iter().any(|(h, path)| {
            (host == *h || host.ends_with(&format!(".{h}"))) && url.path().starts_with(path)
        })
}

/// The first meeting link in these texts.
pub fn meeting_link(texts: &[&str]) -> Option<String> {
    for text in texts {
        for (i, _) in text.match_indices("https://") {
            let url: String = text[i..]
                .chars()
                .take_while(|c| !c.is_whitespace() && !matches!(c, '<' | '>' | '"' | '\'' | ')'))
                .collect();
            let url = url.trim_end_matches(['.', ',', ';']).to_string();
            if is_meeting_link(&url) {
                return Some(url);
            }
        }
    }
    None
}

/// The meetings between `from` and `to` (Unix seconds), repeats included,
/// ordered by start.
pub fn meetings(
    events: &[Event],
    calendar: &str,
    from: i64,
    to: i64,
    local: &dyn Fn(NaiveDateTime) -> Option<i64>,
) -> Vec<Meeting> {
    let overridden: Vec<(&str, NaiveDateTime)> = events
        .iter()
        .filter_map(|e| Some((e.uid.as_str(), e.recurrence_id?)))
        .collect();
    let mut out = Vec::new();
    for e in events.iter().filter(|e| !e.cancelled) {
        let Some(start) = &e.start else { continue };
        let Some(first) = resolve(start, local) else {
            continue;
        };
        let length = match (&e.end, e.duration) {
            (Some(end), _) => resolve(end, local).map_or(0, |end| (end - first).max(0)),
            (None, Some(d)) => d.num_seconds().max(0),
            (None, None) if matches!(start, Stamp::Day(_)) => 86_400,
            _ => 0,
        };
        let title = if e.summary.is_empty() {
            "Busy".to_string()
        } else {
            e.summary.clone()
        };
        let link = meeting_link(&[&e.conference, &e.url, &e.location, &e.description]);
        let location =
            Some(e.location.clone()).filter(|l| !l.is_empty() && !l.starts_with("https://"));
        let all_day = matches!(start, Stamp::Day(_));
        let mut push = |at: i64| {
            if at < to && at + length.max(1) > from {
                out.push(Meeting {
                    uid: e.uid.clone(),
                    title: title.clone(),
                    start: at,
                    end: at + length,
                    all_day,
                    link: link.clone(),
                    location: location.clone(),
                    calendar: calendar.to_string(),
                });
            }
        };
        match (&e.rule, e.recurrence_id) {
            (Some(rule), None) => {
                for at in starts(start, rule, to, local) {
                    if e.exdates.contains(&at)
                        || overridden.iter().any(|(u, r)| *u == e.uid && *r == at)
                    {
                        continue;
                    }
                    if let Some(secs) = resolve(&start.with(at), local) {
                        push(secs);
                    }
                }
            }
            _ => push(first),
        }
    }
    out.sort_by(|a, b| a.start.cmp(&b.start).then(a.title.cmp(&b.title)));
    out
}

/// The meeting going on at `now`, or the next one starting within `soon`
/// seconds. All-day events don't count: they're not meetings to take notes
/// in.
pub fn current(meetings: &[Meeting], now: i64, soon: i64) -> Option<&Meeting> {
    let timed = || meetings.iter().filter(|m| !m.all_day);
    timed()
        .filter(|m| m.start <= now && now < m.end)
        // The most recent start, when two overlap.
        .max_by_key(|m| m.start)
        .or_else(|| timed().find(|m| m.start > now && m.start - now <= soon))
}

/// How long before a meeting starts the bar offers to join it.
pub const REMIND_BEFORE: i64 = 60;
/// A meeting is still offered this long after it started, for a late join.
pub const REMIND_LATE: i64 = 5 * 60;

/// The meeting to offer to join at `now`: a timed one starting within a
/// minute or started less than five minutes ago, not over, and not offered
/// before (`offered` holds uid and start of those).
pub fn to_remind<'a>(
    meetings: &'a [Meeting],
    now: i64,
    offered: &[(String, i64)],
) -> Option<&'a Meeting> {
    meetings
        .iter()
        .filter(|m| !m.all_day && now < m.end)
        .filter(|m| m.start - now <= REMIND_BEFORE && now - m.start <= REMIND_LATE)
        .filter(|m| {
            !offered
                .iter()
                .any(|(uid, start)| *uid == m.uid && *start == m.start)
        })
        .min_by_key(|m| (m.start - now).abs())
}

/// Accepts a pasted address: webcal:// becomes https://; anything that isn't
/// https is refused.
pub fn feed_url(input: &str) -> Option<String> {
    let s = input.trim();
    let s = if let Some(rest) = s.strip_prefix("webcal://") {
        format!("https://{rest}")
    } else {
        s.to_string()
    };
    // https only: the address is the calendar's password, and over plain
    // http anyone on the network could read it and change the meetings.
    let url = url::Url::parse(&s).ok()?;
    let ok = url.scheme() == "https"
        && url.host_str().is_some_and(|h| h.contains('.'))
        && !s.contains(char::is_whitespace);
    ok.then_some(s)
}

/// Whether text looks like a calendar file.
pub fn is_calendar(text: &str) -> bool {
    text.trim_start_matches('\u{feff}')
        .trim_start()
        .to_ascii_uppercase()
        .starts_with("BEGIN:VCALENDAR")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The reader in Berlin (CEST, UTC+2) for these tests.
    fn berlin(n: NaiveDateTime) -> Option<i64> {
        chrono_tz::Europe::Berlin
            .from_local_datetime(&n)
            .earliest()
            .map(|t| t.timestamp())
    }

    fn ts(s: &str) -> i64 {
        chrono::DateTime::parse_from_rfc3339(s).unwrap().timestamp()
    }

    const GOOGLE: &str = "BEGIN:VCALENDAR\r
PRODID:-//Google Inc//Google Calendar 70.9054//EN\r
VERSION:2.0\r
BEGIN:VEVENT\r
DTSTART;TZID=Europe/Berlin:20261005T100000\r
DTEND;TZID=Europe/Berlin:20261005T101500\r
RRULE:FREQ=WEEKLY;WKST=MO;BYDAY=MO,WE,FR\r
EXDATE;TZID=Europe/Berlin:20261021T100000\r
UID:standup@google.com\r
SUMMARY:Standup\r
DESCRIPTION:Join with Google Meet: https://meet.google.com/abc-defg-hij\\nOr \r
 dial: +49 30 1234\r
BEGIN:VALARM\r
DESCRIPTION:This is an event reminder\r
END:VALARM\r
END:VEVENT\r
BEGIN:VEVENT\r
DTSTART;TZID=Europe/Berlin:20261023T110000\r
DTEND;TZID=Europe/Berlin:20261023T111500\r
RECURRENCE-ID;TZID=Europe/Berlin:20261023T100000\r
UID:standup@google.com\r
SUMMARY:Standup (moved)\r
END:VEVENT\r
BEGIN:VEVENT\r
DTSTART:20261020T130000Z\r
DTEND:20261020T140000Z\r
UID:review@google.com\r
SUMMARY:Design review\\, Legato\r
LOCATION:https://us02web.zoom.us/j/8812345678?pwd=abc\r
END:VEVENT\r
BEGIN:VEVENT\r
DTSTART;VALUE=DATE:20261022\r
DTEND;VALUE=DATE:20261023\r
UID:holiday@google.com\r
SUMMARY:Day off\r
END:VEVENT\r
BEGIN:VEVENT\r
DTSTART:20261021T090000Z\r
DTEND:20261021T093000Z\r
UID:gone@google.com\r
STATUS:CANCELLED\r
SUMMARY:Cancelled sync\r
END:VEVENT\r
END:VCALENDAR\r
";

    #[test]
    fn reads_events_and_their_parts() {
        let events = parse(GOOGLE);
        assert_eq!(events.len(), 5);
        let standup = &events[0];
        assert_eq!(standup.summary, "Standup");
        assert!(standup.description.contains("Or dial"), "folded lines join");
        assert!(!standup.description.contains("reminder"), "alarms skipped");
        assert_eq!(standup.rule.as_ref().unwrap().by_day.len(), 3);
        assert_eq!(events[2].summary, "Design review, Legato");
        assert!(events[4].cancelled);
    }

    #[test]
    fn a_week_of_meetings() {
        let events = parse(GOOGLE);
        let from = ts("2026-10-19T00:00:00+02:00");
        let to = ts("2026-10-26T00:00:00+02:00");
        let m = meetings(&events, "Work", from, to, &berlin);
        let titles: Vec<(&str, i64)> = m.iter().map(|m| (m.title.as_str(), m.start)).collect();
        assert_eq!(
            titles,
            vec![
                ("Standup", ts("2026-10-19T10:00:00+02:00")),
                ("Design review, Legato", ts("2026-10-20T15:00:00+02:00")),
                // Wednesday the 21st is excluded; the cancelled one is gone.
                ("Day off", ts("2026-10-22T00:00:00+02:00")),
                // Friday's moved to 11.
                ("Standup (moved)", ts("2026-10-23T11:00:00+02:00")),
            ]
        );
        assert_eq!(m[0].end - m[0].start, 15 * 60);
        assert_eq!(
            m[0].link.as_deref(),
            Some("https://meet.google.com/abc-defg-hij")
        );
        assert_eq!(
            m[1].link.as_deref(),
            Some("https://us02web.zoom.us/j/8812345678?pwd=abc")
        );
        assert_eq!(m[1].location, None);
        assert!(m[2].all_day);
        assert_eq!(m[2].calendar, "Work");
    }

    #[test]
    fn repeats_keep_their_wall_time_across_daylight_saving() {
        // Berlin leaves summer time on 25 October 2026.
        let events = parse(GOOGLE);
        let from = ts("2026-10-26T00:00:00+01:00");
        let to = ts("2026-10-27T00:00:00+01:00");
        let m = meetings(&events, "Work", from, to, &berlin);
        assert_eq!(m[0].start, ts("2026-10-26T10:00:00+01:00"));
    }

    fn one(rule_text: &str, start: &str, from: &str, to: &str) -> Vec<String> {
        let ics = format!(
            "BEGIN:VCALENDAR\nBEGIN:VEVENT\nUID:x\nSUMMARY:x\nDTSTART:{start}\nDURATION:PT30M\nRRULE:{rule_text}\nEND:VEVENT\nEND:VCALENDAR\n"
        );
        meetings(&parse(&ics), "c", ts(from), ts(to), &berlin)
            .iter()
            .map(|m| {
                chrono::DateTime::from_timestamp(m.start, 0)
                    .unwrap()
                    .format("%Y-%m-%d")
                    .to_string()
            })
            .collect()
    }

    #[test]
    fn rules_count_until_interval_and_positions() {
        assert_eq!(
            one(
                "FREQ=DAILY;COUNT=3",
                "20261020T080000Z",
                "2026-10-01T00:00:00Z",
                "2026-12-01T00:00:00Z"
            ),
            ["2026-10-20", "2026-10-21", "2026-10-22"]
        );
        assert_eq!(
            one(
                "FREQ=WEEKLY;INTERVAL=2;UNTIL=20261118T235959Z",
                "20261021T080000Z",
                "2026-10-01T00:00:00Z",
                "2026-12-31T00:00:00Z"
            ),
            ["2026-10-21", "2026-11-04", "2026-11-18"]
        );
        // The last Friday of each month.
        assert_eq!(
            one(
                "FREQ=MONTHLY;BYDAY=-1FR",
                "20261030T080000Z",
                "2026-10-01T00:00:00Z",
                "2027-01-01T00:00:00Z"
            ),
            ["2026-10-30", "2026-11-27", "2026-12-25"]
        );
        // The second Tuesday.
        assert_eq!(
            one(
                "FREQ=MONTHLY;BYDAY=2TU;COUNT=2",
                "20261013T080000Z",
                "2026-10-01T00:00:00Z",
                "2027-06-01T00:00:00Z"
            ),
            ["2026-10-13", "2026-11-10"]
        );
        // The 31st skips short months.
        assert_eq!(
            one(
                "FREQ=MONTHLY",
                "20261031T080000Z",
                "2026-10-01T00:00:00Z",
                "2027-02-01T00:00:00Z"
            ),
            ["2026-10-31", "2026-12-31", "2027-01-31"]
        );
        // Weekdays only, starting on a Saturday.
        assert_eq!(
            one(
                "FREQ=DAILY;BYDAY=MO,TU,WE,TH,FR",
                "20261024T080000Z",
                "2026-10-01T00:00:00Z",
                "2026-10-28T00:00:00Z"
            ),
            ["2026-10-26", "2026-10-27"]
        );
        // A long-running daily meeting still reaches today.
        assert_eq!(
            one(
                "FREQ=DAILY",
                "20200101T080000Z",
                "2026-10-20T00:00:00Z",
                "2026-10-21T00:00:00Z"
            ),
            ["2026-10-20"]
        );
    }

    #[test]
    fn outlook_zone_names_and_floating_times() {
        let ics = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nUID:o\nSUMMARY:1:1\nDTSTART;TZID=\"W. Europe Standard Time\":20261020T090000\nDTEND;TZID=\"W. Europe Standard Time\":20261020T093000\nLOCATION:Microsoft Teams Meeting\nDESCRIPTION:Join: <https://teams.microsoft.com/l/meetup-join/19%3ameeting_x/0?context=y>\nEND:VEVENT\nBEGIN:VEVENT\nUID:f\nSUMMARY:Lunch\nDTSTART:20261020T120000\nEND:VEVENT\nEND:VCALENDAR\n";
        let m = meetings(
            &parse(ics),
            "Outlook",
            ts("2026-10-20T00:00:00+02:00"),
            ts("2026-10-21T00:00:00+02:00"),
            &berlin,
        );
        assert_eq!(m[0].start, ts("2026-10-20T09:00:00+02:00"));
        assert!(m[0]
            .link
            .as_deref()
            .unwrap()
            .starts_with("https://teams.microsoft.com/l/meetup-join/"));
        assert_eq!(m[0].location.as_deref(), Some("Microsoft Teams Meeting"));
        assert_eq!(m[1].start, ts("2026-10-20T12:00:00+02:00"));
    }

    #[test]
    fn the_meeting_now_or_about_to_start() {
        let m = |title: &str, start: i64, end: i64, all_day: bool| Meeting {
            uid: title.into(),
            title: title.into(),
            start,
            end,
            all_day,
            link: None,
            location: None,
            calendar: "c".into(),
        };
        let list = vec![
            m("Day off", 0, 86_400, true),
            m("Standup", 1000, 1900, false),
            m("Review", 2000, 5600, false),
        ];
        assert_eq!(current(&list, 1500, 300).unwrap().title, "Standup");
        assert_eq!(current(&list, 1900, 300).unwrap().title, "Review");
        assert!(current(&list, 1500 - 900, 300).is_none());
        assert!(current(&list, 6000, 300).is_none());
    }

    #[test]
    fn a_hostile_feed_cant_crash_or_plant_a_link() {
        assert_eq!(duration("PT9223372036854775807S"), None);
        assert_eq!(duration("P99999999999999W"), None);
        assert_eq!(duration("PT1H30M"), Some(Duration::minutes(90)));
        assert!(rule("FREQ=WEEKLY;BYDAY=€").is_none());
        assert_eq!(meeting_link(&["https://evil.example/?x=zoom.us/j/1"]), None);
        assert_eq!(meeting_link(&["https://zoom.us.evil.example/j/1"]), None);
        assert_eq!(
            meeting_link(&["https://meet.google.com@evil.example/"]),
            None
        );
        assert_eq!(
            meeting_link(&["Join: https://us02web.zoom.us/j/88?pwd=x."]).as_deref(),
            Some("https://us02web.zoom.us/j/88?pwd=x")
        );
    }

    #[test]
    fn pasted_addresses() {
        assert_eq!(
            feed_url(" webcal://calendar.google.com/calendar/ical/x/private-y/basic.ics ")
                .as_deref(),
            Some("https://calendar.google.com/calendar/ical/x/private-y/basic.ics")
        );
        assert_eq!(feed_url("file:///etc/passwd"), None);
        assert_eq!(feed_url("https://a b"), None);
        assert_eq!(feed_url("http://example.com/basic.ics"), None);
        assert_eq!(feed_url("https://localhost/basic.ics"), None);
        assert!(is_calendar("\u{feff}BEGIN:VCALENDAR\r\n"));
        assert!(!is_calendar("<html>"));
    }

    fn meeting(uid: &str, start: i64, minutes: i64) -> Meeting {
        Meeting {
            uid: uid.into(),
            title: uid.into(),
            start,
            end: start + minutes * 60,
            all_day: false,
            link: Some("https://meet.google.com/abc-defg-hij".into()),
            location: None,
            calendar: "Work".into(),
        }
    }

    #[test]
    fn a_meeting_is_offered_a_minute_ahead_and_once() {
        let now = 1_000_000;
        let standup = meeting("standup", now + 50, 15);
        let later = meeting("review", now + 30 * 60, 30);
        let all = vec![standup.clone(), later];
        assert_eq!(to_remind(&all, now - 120, &[]), None, "too early");
        assert_eq!(to_remind(&all, now, &[]), Some(&standup));
        let offered = vec![("standup".to_string(), now + 50)];
        assert_eq!(to_remind(&all, now, &offered), None, "offered already");
        assert_eq!(
            to_remind(&all, now + 50 + 4 * 60, &[]),
            Some(&standup),
            "a late join"
        );
        assert_eq!(to_remind(&all, now + 50 + 6 * 60, &[]), None, "too late");
        let mut day = meeting("holiday", now + 30, 24 * 60);
        day.all_day = true;
        assert_eq!(to_remind(&[day], now, &[]), None, "all day");
    }
}
