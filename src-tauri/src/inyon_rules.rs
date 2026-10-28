//! Rules for Inyon, the agent hub Legato pairs with as a companion app:
//! the hub's address, the frames its WebSocket sends, which agents are
//! waiting for an answer and which one the island shows. Free of Tauri and
//! the network, so it can be tested anywhere. The contract is version 1
//! (docs/companion-contract.md).

use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::BTreeMap;
use std::net::IpAddr;

/// The contract version Legato speaks, sent as `X-Inyon-Contract`.
pub const CONTRACT: u32 = 1;

/// The agent catalog Legato understands. 2 brings Inyon 0.23's terminal-only
/// kinds (cursor, goose, kiro…); the kind is kept as text, so any name works.
pub const CATALOG: u32 = 2;

/// The hub's address as typed, tidied to its origin. A bare address gets
/// `http://` when the token may go in the clear there (this PC, or a
/// NetBird or Tailscale address), else `https://`. A user name, a path, a
/// query or a fragment is refused: a hub lives at the root of its address,
/// and `https://hub.example.com@evil.example` must not slip past.
pub fn hub_url(input: &str) -> Option<String> {
    let input = input.trim();
    let url = if input.contains("://") {
        parse_hub(input)?
    } else {
        let url = parse_hub(&format!("http://{input}"))?;
        let local = match url.host()? {
            url::Host::Domain(d) => d.eq_ignore_ascii_case("localhost"),
            url::Host::Ipv4(v4) => plain_http_ok(IpAddr::V4(v4)),
            url::Host::Ipv6(v6) => plain_http_ok(IpAddr::V6(v6)),
        };
        if local {
            url
        } else {
            parse_hub(&format!("https://{input}"))?
        }
    };
    Some(url.origin().ascii_serialization())
}

/// A hub address the client will talk to: http or https, a host, and
/// nothing after it but a slash.
fn parse_hub(hub: &str) -> Option<url::Url> {
    let url = url::Url::parse(hub).ok()?;
    let ok = matches!(url.scheme(), "http" | "https")
        && url.username().is_empty()
        && url.password().is_none()
        && url.path() == "/"
        && url.query().is_none()
        && url.fragment().is_none()
        && url.host_str().is_some_and(|h| !h.is_empty());
    ok.then_some(url)
}

/// Whether the bearer token may travel over plain http to this address:
/// this PC, or a WireGuard VPN, where the tunnel encrypts it (NetBird and
/// Tailscale's 100.64.0.0/10, Tailscale's fd7a:115c:a1e0::/48). Anywhere
/// else, a home Wi-Fi included, anyone on the network could read it.
pub fn plain_http_ok(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            v4.is_loopback() || (o[0] == 100 && (o[1] & 0xc0) == 64)
        }
        IpAddr::V6(v6) => {
            let s = v6.segments();
            v6.is_loopback()
                || (s[0] == 0xfd7a && s[1] == 0x115c && s[2] == 0xa1e0)
                || v6
                    .to_ipv4_mapped()
                    .is_some_and(|v4| plain_http_ok(IpAddr::V4(v4)))
        }
    }
}

/// The host (without brackets) and port of an `http(s)://` hub address.
pub fn host_port(hub: &str) -> Option<(String, u16)> {
    let url = parse_hub(hub)?;
    let host = match url.host()? {
        url::Host::Domain(d) => d.to_string(),
        url::Host::Ipv4(v4) => v4.to_string(),
        url::Host::Ipv6(v6) => v6.to_string(),
    };
    Some((host, url.port_or_known_default()?))
}

/// The hub's WebSocket address for an `http(s)://` hub address.
pub fn ws_url(hub: &str) -> String {
    let ws = if let Some(rest) = hub.strip_prefix("https://") {
        format!("wss://{rest}")
    } else if let Some(rest) = hub.strip_prefix("http://") {
        format!("ws://{rest}")
    } else {
        hub.to_string()
    };
    format!("{ws}/ws?contract={CONTRACT}&catalog={CATALOG}")
}

/// A pairing code as typed: any case, spaces and dashes dropped.
pub fn pairing_code(input: &str) -> Option<String> {
    let code: String = input
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .collect::<String>()
        .to_uppercase();
    (code.len() == 6 && code.chars().all(|c| c.is_ascii_alphanumeric())).then_some(code)
}

/// A hub certificate's SHA-256 as Inyon gives it: 64 hex digits, kept
/// lowercase. Anything else is refused.
pub fn tls_fingerprint(input: &str) -> Option<String> {
    let fp = input.trim().to_ascii_lowercase();
    (fp.len() == 64 && fp.chars().all(|c| c.is_ascii_hexdigit())).then_some(fp)
}

/// What Inyon's Copy pairing link holds:
/// `inyon://companion?u=<hub origin>&c=<code>[&f=<sha256 hex>]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairingLink {
    pub hub: String,
    pub code: String,
    /// The hub's certificate, when it serves TLS itself.
    pub fingerprint: Option<String>,
}

/// Reads a pairing link; None for anything that isn't a whole, valid one.
/// A fingerprint goes only with an https hub.
pub fn pairing_link(input: &str) -> Option<PairingLink> {
    let url = url::Url::parse(input.trim()).ok()?;
    if url.scheme() != "inyon" || url.host_str() != Some("companion") {
        return None;
    }
    let param = |name: &str| {
        url.query_pairs()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.into_owned())
    };
    // The hub's origin as the link gives it, scheme and all.
    let hub = param("u").filter(|u| u.contains("://"))?;
    let hub = hub_url(&hub)?;
    let code = pairing_code(&param("c")?)?;
    let fingerprint = match param("f") {
        Some(f) => Some(tls_fingerprint(&f)?),
        None => None,
    }
    .filter(|_| hub.starts_with("https://"));
    Some(PairingLink {
        hub,
        code,
        fingerprint,
    })
}

/// A pane id in a route travels without its `%`.
pub fn pane_path(pane: &str) -> &str {
    pane.strip_prefix('%').unwrap_or(pane)
}

/// The route for answering or sending to a pane.
pub fn pane_route(hub: &str, host: &str, pane: &str, action: &str) -> String {
    format!(
        "{hub}/api/hosts/{}/panes/{}/{action}",
        encode(host),
        encode(pane_path(pane))
    )
}

/// Percent-encodes a path segment.
fn encode(segment: &str) -> String {
    let mut out = String::new();
    for b in segment.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// What the claim answers.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Claimed {
    pub token: String,
    pub hub_id: String,
    #[serde(default)]
    pub scopes: Vec<String>,
    pub contract_version: u32,
    /// The hub's certificate, when it serves TLS itself (contract 1.1).
    #[serde(default)]
    pub tls_fingerprint: Option<String>,
}

/// One of an agent's options.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Type)]
pub struct PromptOption {
    pub n: u32,
    pub label: String,
}

/// An agent's question as `chat.screen` carries it.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Prompt {
    #[serde(default)]
    pub body: Vec<String>,
    #[serde(default)]
    pub options: Vec<PromptOption>,
    #[serde(default)]
    pub multi: bool,
    #[serde(default)]
    pub key: Option<String>,
    /// The option an answer may carry text with ("No, and tell Claude
    /// what to do differently"); none when the question takes no text.
    #[serde(default, rename = "freeTextOption")]
    pub free_text: Option<u32>,
}

/// The most questions the queue holds, and how much of each it keeps.
const MAX_QUESTIONS: usize = 50;
const MAX_LINES: usize = 40;
const MAX_OPTIONS: usize = 20;
const MAX_LINE_CHARS: usize = 500;

fn clip(text: &mut String, max: usize) {
    if let Some((at, _)) = text.char_indices().nth(max) {
        text.truncate(at);
    }
}

impl Prompt {
    fn trimmed(mut self) -> Self {
        self.body.truncate(MAX_LINES);
        self.options.truncate(MAX_OPTIONS);
        for line in &mut self.body {
            clip(line, MAX_LINE_CHARS);
        }
        for option in &mut self.options {
            clip(&mut option.label, MAX_LINE_CHARS);
        }
        if let Some(key) = &mut self.key {
            clip(key, MAX_LINE_CHARS);
        }
        self
    }
}

/// Text for an agent's terminal with control characters taken out (an
/// escape could reach the terminal itself), keeping new lines and tabs;
/// Windows line ends become plain new lines.
pub fn terminal_text(text: &str) -> String {
    text.replace("\r\n", "\n")
        .replace('\r', "\n")
        .chars()
        .filter(|c| !c.is_control() || matches!(c, '\n' | '\t'))
        .collect()
}

/// A pane on the hub: its machine and tmux pane id.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Type)]
pub struct PaneRef {
    pub host: String,
    pub pane: String,
}

/// The frames Legato reads from the hub; others are ignored.
#[derive(Debug, Clone, PartialEq)]
pub enum Frame {
    /// The panes running an agent, and whether each waits for you.
    Overview(Vec<AgentPane>),
    /// A watched pane's screen: its question, or none.
    Screen {
        pane: PaneRef,
        prompt: Option<Prompt>,
    },
    /// A pane's details, for its agent's name and title.
    Info {
        pane: PaneRef,
        title: Option<String>,
    },
    Notify(Notify),
    /// Inyon's companion agent finished a turn.
    CompanionReply(CompanionReply),
    Pong,
}

/// What Inyon's companion agent said at the end of a turn.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct CompanionReply {
    /// Unique per reply, so one seen twice (after a reconnect) shows once.
    pub id: String,
    pub host: String,
    pub pane: String,
    /// Plain text, up to about 2000 characters.
    pub text: String,
    /// The turn stopped on an error; `text` says why.
    pub error: bool,
}

/// The longest companion reply kept, past the hub's own limit.
const MAX_REPLY_CHARS: usize = 4000;

/// A pane with an agent, from the overview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentPane {
    pub pane: PaneRef,
    pub agent: String,
    pub waiting: bool,
    /// The session or window name, to tell agents apart.
    pub label: String,
    /// Inyon's companion agent, the one you talk to about the others.
    pub companion: bool,
    /// Busy with a turn.
    pub working: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notify {
    pub pane: PaneRef,
    pub kind: String,
    pub title: String,
    pub body: String,
    pub agent: String,
}

/// Reads one text frame. None for frames Legato doesn't use or can't read.
pub fn parse_frame(text: &str) -> Option<Frame> {
    let v: serde_json::Value = serde_json::from_str(text).ok()?;
    let s = |v: &serde_json::Value, k: &str| v.get(k).and_then(|x| x.as_str()).map(str::to_string);
    match v.get("t")?.as_str()? {
        "overview" => Some(Frame::Overview(agent_panes(&v))),
        "chat.screen" => {
            let pane = frame_pane(&v)?;
            let prompt = v
                .get("screen")
                .and_then(|screen| screen.get("prompt"))
                .filter(|p| !p.is_null())
                .and_then(|p| serde_json::from_value::<Prompt>(p.clone()).ok());
            Some(Frame::Screen { pane, prompt })
        }
        "chat.info" => {
            let pane = frame_pane(&v)?;
            let info = v.get("info").unwrap_or(&v);
            Some(Frame::Info {
                pane,
                title: s(info, "title").or_else(|| s(info, "name")),
            })
        }
        "notify" => Some(Frame::Notify(Notify {
            pane: PaneRef {
                host: s(&v, "host")?,
                pane: s(&v, "pane")?,
            },
            kind: s(&v, "kind")?,
            title: s(&v, "title").unwrap_or_default(),
            body: s(&v, "body").unwrap_or_default(),
            agent: s(&v, "agent").unwrap_or_default(),
        })),
        "companion.reply" => {
            let mut text = s(&v, "text").unwrap_or_default();
            clip(&mut text, MAX_REPLY_CHARS);
            Some(Frame::CompanionReply(CompanionReply {
                id: s(&v, "id")?,
                host: s(&v, "host")?,
                pane: s(&v, "pane")?,
                text: text.trim().to_string(),
                error: v.get("error").and_then(|e| e.as_bool()).unwrap_or(false),
            }))
        }
        "pong" => Some(Frame::Pong),
        _ => None,
    }
}

/// The pane a chat frame is about: `host` and `pane`, or the `key`
/// ("box:%3") the hub adds.
fn frame_pane(v: &serde_json::Value) -> Option<PaneRef> {
    let s = |k: &str| v.get(k).and_then(|x| x.as_str());
    if let (Some(host), Some(pane)) = (s("host"), s("pane")) {
        return Some(PaneRef {
            host: host.to_string(),
            pane: pane.to_string(),
        });
    }
    let (host, pane) = s("key")?.rsplit_once(':')?;
    Some(PaneRef {
        host: host.to_string(),
        pane: pane.to_string(),
    })
}

/// Every pane running an agent in an overview frame (`hosts` → sessions →
/// windows → panes), read loosely so an added field never breaks it.
fn agent_panes(v: &serde_json::Value) -> Vec<AgentPane> {
    let mut out = Vec::new();
    let empty = Vec::new();
    let list = |v: &serde_json::Value, k: &str| -> Vec<serde_json::Value> {
        v.get(k)
            .and_then(|x| x.as_array())
            .cloned()
            .unwrap_or_default()
    };
    let hosts = v
        .get("hosts")
        .or_else(|| v.get("overview").and_then(|o| o.get("hosts")))
        .and_then(|h| h.as_array())
        .unwrap_or(&empty);
    for host in hosts {
        // HostState keeps its id in `config`.
        let Some(host_id) = host
            .get("config")
            .and_then(|c| c.get("id"))
            .or_else(|| host.get("id"))
            .and_then(|x| x.as_str())
        else {
            continue;
        };
        for session in list(host, "sessions") {
            let session_name = session.get("name").and_then(|x| x.as_str()).unwrap_or("");
            for window in list(&session, "windows") {
                let window_name = window.get("name").and_then(|x| x.as_str()).unwrap_or("");
                for pane in list(&window, "panes") {
                    let (Some(id), Some(agent)) =
                        (pane.get("id").and_then(|x| x.as_str()), pane.get("agent"))
                    else {
                        continue;
                    };
                    if agent.is_null() {
                        continue;
                    }
                    let name = agent
                        .get("kind")
                        .or_else(|| agent.get("name"))
                        .and_then(|x| x.as_str())
                        .unwrap_or("agent");
                    let state = agent.get("state").and_then(|x| x.as_str()).unwrap_or("");
                    // The name given to the session in Inyon, else
                    // session › window.
                    let given = pane.get("label").and_then(|x| x.as_str()).unwrap_or("");
                    let label = if !given.is_empty() {
                        given.to_string()
                    } else if window_name.is_empty() || window_name == session_name {
                        session_name.to_string()
                    } else {
                        format!("{session_name} › {window_name}")
                    };
                    out.push(AgentPane {
                        pane: PaneRef {
                            host: host_id.to_string(),
                            pane: id.to_string(),
                        },
                        agent: name.to_string(),
                        waiting: state == "waiting",
                        label,
                        companion: pane.get("companion").and_then(|c| c.as_bool()) == Some(true),
                        working: state == "working",
                    });
                }
            }
        }
    }
    out
}

/// An agent waiting for you, as the island shows it.
#[derive(Debug, Clone, Serialize, PartialEq, Type)]
pub struct AgentAlert {
    pub host: String,
    pub pane: String,
    /// "claude", "codex"…
    pub agent: String,
    /// Which session: its name, or session › window.
    pub label: String,
    /// What the agent says before the command, like "Bash command" and
    /// what it's for.
    pub body: Vec<String>,
    /// The command it wants to run, when the question has one.
    pub command: Vec<String>,
    /// What it asks, after the command ("Do you want to proceed?").
    pub ask: Vec<String>,
    pub options: Vec<PromptOption>,
    /// Ties an answer to this question, so a late tap can't hit the next.
    pub key: Option<String>,
    /// The option a spoken answer's words go with, if any.
    pub free_text: Option<u32>,
    /// How many more agents wait behind this one.
    pub more: u32,
}

/// The agents waiting for an answer, in the order they started waiting.
/// The island shows the first; answering it (here or anywhere) shows the
/// next.
#[derive(Debug, Default)]
pub struct Waiting {
    /// What the overview says about each agent pane.
    panes: BTreeMap<PaneRef, AgentPane>,
    /// Questions seen on watched panes, oldest first.
    questions: Vec<(PaneRef, Prompt)>,
}

impl Waiting {
    /// A new overview: panes that stopped waiting lose their question.
    /// Returns the panes to watch (waiting, not yet watched) and to stop
    /// watching.
    pub fn overview(&mut self, panes: Vec<AgentPane>) -> (Vec<PaneRef>, Vec<PaneRef>) {
        let before: Vec<PaneRef> = self.watched();
        self.panes = panes.into_iter().map(|p| (p.pane.clone(), p)).collect();
        let panes = &self.panes;
        self.questions
            .retain(|(pane, _)| panes.get(pane).is_some_and(|p| p.waiting));
        let after = self.watched();
        let start = after
            .iter()
            .filter(|p| !before.contains(p))
            .cloned()
            .collect();
        let stop = before.into_iter().filter(|p| !after.contains(p)).collect();
        (start, stop)
    }

    /// The panes whose screens Legato watches: agents waiting for you.
    pub fn watched(&self) -> Vec<PaneRef> {
        self.panes
            .values()
            .filter(|p| p.waiting)
            .map(|p| p.pane.clone())
            .collect()
    }

    /// A watched pane's screen: a question joins the queue (or replaces
    /// that pane's earlier one, keeping its place); no question removes it.
    /// A pane Legato isn't watching is ignored, and a question is trimmed
    /// to what a card can show, so a hub can't flood the island.
    pub fn screen(&mut self, pane: PaneRef, prompt: Option<Prompt>) {
        if !self.panes.get(&pane).is_some_and(|p| p.waiting) {
            return;
        }
        let at = self.questions.iter().position(|(p, _)| *p == pane);
        if at.is_none() && self.questions.len() >= MAX_QUESTIONS {
            return;
        }
        match (
            prompt
                .map(Prompt::trimmed)
                .filter(|p| !p.options.is_empty() || !p.body.is_empty()),
            at,
        ) {
            (Some(prompt), Some(i)) => self.questions[i].1 = prompt,
            (Some(prompt), None) => self.questions.push((pane, prompt)),
            (None, Some(i)) => {
                self.questions.remove(i);
            }
            (None, None) => {}
        }
    }

    /// An answer went through for this question: it leaves at once rather
    /// than waiting for the hub's next screen.
    pub fn answered(&mut self, pane: &PaneRef, key: Option<&str>) {
        self.questions
            .retain(|(p, q)| !(p == pane && (key.is_none() || q.key.as_deref() == key)));
    }

    /// What the island shows: the oldest question, and how many wait
    /// behind it.
    pub fn current(&self) -> Option<AgentAlert> {
        let (pane, prompt) = self.questions.first()?;
        let info = self.panes.get(pane);
        let (body, command, ask) = split_body(&prompt.body);
        Some(AgentAlert {
            host: pane.host.clone(),
            pane: pane.pane.clone(),
            agent: info
                .map(|p| p.agent.clone())
                .unwrap_or_else(|| "agent".into()),
            label: info.map(|p| p.label.clone()).unwrap_or_default(),
            body,
            command,
            ask,
            options: prompt.options.clone(),
            key: prompt.key.clone(),
            free_text: prompt.free_text,
            more: self.questions.len().saturating_sub(1) as u32,
        })
    }

    /// Every pane running an agent, for Send to.
    pub fn agents(&self) -> Vec<AgentPane> {
        self.panes.values().cloned().collect()
    }

    /// Inyon's companion agent, when one runs. A hub runs at most one.
    pub fn companion(&self) -> Option<AgentPane> {
        self.panes.values().find(|p| p.companion).cloned()
    }

    /// The hub went away: nothing waits any more.
    pub fn clear(&mut self) {
        self.panes.clear();
        self.questions.clear();
    }
}

/// Splits a question's lines into what comes before the command, the
/// command and the question after it. The hub sets the command off with
/// lines of "╌"; without them, a first line like "Bash command" means the
/// next line is the command.
pub fn split_body(lines: &[String]) -> (Vec<String>, Vec<String>, Vec<String>) {
    let rule = |l: &str| {
        let l = l.trim();
        !l.is_empty() && l.chars().all(|c| matches!(c, '╌' | '─' | '┄' | '-' | '—'))
    };
    let parts: Vec<Vec<String>> = lines
        .split(|l| rule(l))
        .map(|part| {
            part.iter()
                .map(|l| l.trim_end().to_string())
                .filter(|l| !l.trim().is_empty())
                .collect()
        })
        .collect();
    match parts.as_slice() {
        [head, command, rest @ ..] if !command.is_empty() => {
            (head.clone(), command.clone(), rest.concat())
        }
        [only] if only.len() >= 3 && only[0].ends_with(" command") => (
            vec![only[0].clone()],
            vec![only[1].clone()],
            only[2..].to_vec(),
        ),
        _ => (parts.concat(), Vec::new(), Vec::new()),
    }
}

/// What a spoken answer to a question means.
#[derive(Debug, Clone, PartialEq)]
pub enum VoiceAnswer {
    /// One of the options, by its number or its words.
    Pick(u32),
    /// The option that takes typed text ("No, and tell Claude what to do
    /// differently"), with what was said.
    WithText(u32, String),
}

/// Reads a spoken answer against a question's options: a number ("two",
/// "option 2"), an option's words ("yes", "yes and don't ask again"), or
/// else what was said, for the option that takes text. None when nothing
/// fits, so nothing is sent on a guess.
pub fn voice_answer(
    said: &str,
    options: &[PromptOption],
    free_text: Option<u32>,
) -> Option<VoiceAnswer> {
    let words = |s: &str| -> Vec<String> {
        s.to_lowercase()
            .replace("n't", "nt")
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
            .map(str::to_string)
            .collect()
    };
    let spoken = words(said);
    let spoken = match spoken.as_slice() {
        [first, rest @ ..] if first == "option" || first == "number" => rest.to_vec(),
        _ => spoken,
    };
    if spoken.is_empty() {
        return None;
    }
    // A number on its own.
    const NUMBERS: [&str; 10] = [
        "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine",
    ];
    const ORDINALS: [&str; 10] = [
        "", "first", "second", "third", "fourth", "fifth", "sixth", "seventh", "eighth", "ninth",
    ];
    if let [w] = spoken.as_slice() {
        let n = w.parse::<u32>().ok().or_else(|| {
            NUMBERS
                .iter()
                .chain(ORDINALS.iter())
                .position(|x| x == w)
                .map(|i| (i % 10) as u32)
        });
        if let Some(n) = n.filter(|n| options.iter().any(|o| o.n == *n)) {
            return Some(VoiceAnswer::Pick(n));
        }
    }
    // An option's own words, or a short way of saying it.
    let synonym = |w: &String| -> String {
        match w.as_str() {
            "allow" | "approve" | "sure" | "ok" | "okay" | "yeah" | "yep" => "yes".into(),
            "deny" | "nope" | "dont" => "no".into(),
            _ => w.clone(),
        }
    };
    let spoken: Vec<String> = spoken.iter().map(synonym).collect();
    let labels: Vec<(u32, Vec<String>)> = options
        .iter()
        .map(|o| (o.n, words(&o.label).iter().map(synonym).collect()))
        .collect();
    if let Some((n, _)) = labels.iter().find(|(_, l)| *l == spoken) {
        return Some(VoiceAnswer::Pick(*n));
    }
    // The longest label the words start with, if they say no more than it.
    if let Some((n, _)) = labels
        .iter()
        .filter(|(_, l)| l.len() > 1 && spoken.len() <= l.len() + 1 && spoken.starts_with(l))
        .max_by_key(|(_, l)| l.len())
    {
        return Some(VoiceAnswer::Pick(*n));
    }
    // Anything else goes, as said, to the option the hub says takes text.
    let takes_text = options.iter().find(|o| Some(o.n) == free_text)?;
    let text = said.trim();
    // "No, run the tests first" is the instruction, not the "no".
    let text = ["no,", "no.", "no "]
        .iter()
        .find_map(|p| {
            text.get(..p.len())
                .filter(|h| h.eq_ignore_ascii_case(p))
                .map(|_| text[p.len()..].trim())
        })
        .filter(|t| !t.is_empty())
        .unwrap_or(text);
    Some(VoiceAnswer::WithText(takes_text.n, text.to_string()))
}

/// How long to wait before reconnecting after `failures` failed tries in a
/// row: 1 s, doubling to a minute.
pub fn backoff_secs(failures: u32) -> u64 {
    1u64.checked_shl(failures.min(6)).unwrap_or(64).min(60)
}

/// Why an answer or a send didn't go through, from the hub's `reason`, as a
/// key under `inyon.reason` in the translations.
pub fn reason_key(status: u16, reason: Option<&str>) -> &'static str {
    match (status, reason) {
        (401, _) => "signedOut",
        (426, _) => "contract",
        (_, Some("already-answered")) => "alreadyAnswered",
        (_, Some("pane-gone")) => "paneGone",
        (_, Some("offline")) => "offline",
        (_, Some("bad-option")) => "badOption",
        (_, Some("prompt-open")) => "promptOpen",
        (_, Some("busy")) => "busy",
        (_, Some("not-landed")) => "notLanded",
        (_, Some("no-agent")) => "noAgent",
        (_, Some("scope")) => "scope",
        _ => "failed",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pane(id: &str) -> PaneRef {
        PaneRef {
            host: "box".into(),
            pane: id.into(),
        }
    }

    fn agent(id: &str, waiting: bool) -> AgentPane {
        AgentPane {
            pane: pane(id),
            agent: "claude".into(),
            waiting,
            label: "legato".into(),
            companion: false,
            working: false,
        }
    }

    fn prompt(key: &str) -> Prompt {
        Prompt {
            body: vec!["Bash command".into(), "rm -rf build".into()],
            options: vec![
                PromptOption {
                    n: 1,
                    label: "Yes".into(),
                },
                PromptOption {
                    n: 2,
                    label: "No".into(),
                },
            ],
            multi: false,
            key: Some(key.into()),
            free_text: None,
        }
    }

    #[test]
    fn the_companion_pane_is_found_in_the_overview() {
        let overview = r#"{"t":"overview","hosts":[{"config":{"id":"box"},"sessions":[{"name":"companion","windows":[{"name":"companion","panes":[{"id":"%12","companion":true,"agent":{"kind":"claude","state":"working"}}]}]},{"name":"legato","windows":[{"name":"legato","panes":[{"id":"%3","agent":{"kind":"codex","state":"idle"}}]}]}]}]}"#;
        let Some(Frame::Overview(panes)) = parse_frame(overview) else {
            panic!("no overview");
        };
        let mut waiting = Waiting::default();
        waiting.overview(panes);
        let companion = waiting.companion().expect("a companion");
        assert_eq!(
            companion.pane,
            PaneRef {
                host: "box".into(),
                pane: "%12".into()
            }
        );
        assert!(companion.working);
        assert_eq!(waiting.agents().iter().filter(|a| a.companion).count(), 1);
        waiting.clear();
        assert!(waiting.companion().is_none());
    }

    #[test]
    fn splits_the_command_out_of_a_question() {
        let lines = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        // As the hub sends it.
        let (body, command, ask) = split_body(&lines(&[
            "Bash command",
            "Write hello-inyon to out.txt",
            "╌",
            "echo hello-inyon > out.txt",
            "╌",
            "Do you want to proceed?",
        ]));
        assert_eq!(
            body,
            lines(&["Bash command", "Write hello-inyon to out.txt"])
        );
        assert_eq!(command, lines(&["echo hello-inyon > out.txt"]));
        assert_eq!(ask, lines(&["Do you want to proceed?"]));
        // Without the rules.
        let (body, command, ask) = split_body(&lines(&[
            "Bash command",
            "rm -rf build",
            "Do you want to proceed?",
        ]));
        assert_eq!(
            (body.len(), command, ask.len()),
            (1, lines(&["rm -rf build"]), 1)
        );
        // A question with no command.
        let (body, command, _) = split_body(&lines(&["Which approach?", "Pick one"]));
        assert_eq!((body.len(), command.len()), (2, 0));
    }

    #[test]
    fn a_real_hubs_frames() {
        // Captured from Inyon's companion test hub, trimmed.
        let overview = r#"{"t":"overview","hosts":[{"config":{"id":"local","name":"build-box","kind":"local"},"status":"ok","sessions":[{"id":"$0","name":"legato-test","attached":0,"windows":[{"id":"@0","index":0,"name":"legato-test","active":true,"activity":0,"panes":[{"id":"%1","index":0,"active":true,"command":"claude","cwd":"/x","title":"","width":110,"height":30,"dead":false,"agent":{"kind":"claude","pid":4132496,"state":"waiting","mode":"manual","waitingFor":"Write hello-inyon to out.txt: echo hello-inyon > out.txt","options":[{"n":1,"label":"Yes"},{"n":3,"label":"No"}],"ultracode":false}}]}]},{"id":"$1","name":"work","attached":0,"windows":[{"id":"@1","index":0,"name":"work","active":true,"activity":0,"panes":[{"id":"%0","index":0,"active":true,"command":"bash","cwd":"/x","title":"","width":80,"height":24,"dead":false}]}]}]}]}"#;
        let empty = r#"{"t":"chat.screen","key":"local:%1","screen":{"working":false,"width":0}}"#;
        let asking = r#"{"t":"chat.screen","key":"local:%1","screen":{"prompt":{"body":["Bash command","Write hello-inyon to out.txt","╌","echo hello-inyon > out.txt","╌","Do you want to proceed?"],"wholeLines":true,"options":[{"n":1,"label":"Yes","selected":true},{"n":2,"label":"Yes, and don't ask again for echo commands in this project","selected":false},{"n":3,"label":"No","selected":false}],"footer":"Esc to cancel · Tab to amend","multi":false,"key":"BashcommandWritehello-inyon"},"working":false,"width":110}}"#;
        let mut waiting = Waiting::default();
        let Some(Frame::Overview(panes)) = parse_frame(overview) else {
            panic!("overview");
        };
        let me = PaneRef {
            host: "local".into(),
            pane: "%1".into(),
        };
        assert_eq!(waiting.overview(panes), (vec![me.clone()], vec![]));
        for frame in [empty, asking] {
            let Some(Frame::Screen { pane, prompt }) = parse_frame(frame) else {
                panic!("chat.screen");
            };
            waiting.screen(pane, prompt);
        }
        let alert = waiting.current().expect("a question");
        assert_eq!((alert.host.as_str(), alert.pane.as_str()), ("local", "%1"));
        assert_eq!(alert.label, "legato-test");
        assert_eq!(
            alert.command,
            vec!["echo hello-inyon > out.txt".to_string()]
        );
        assert_eq!(alert.options.len(), 3);
        assert_eq!(alert.key.as_deref(), Some("BashcommandWritehello-inyon"));
        waiting.answered(&me, alert.key.as_deref());
        assert_eq!(waiting.current(), None);
    }

    #[test]
    fn spoken_answers_pick_an_option() {
        let opts = |v: &[&str]| {
            v.iter()
                .enumerate()
                .map(|(i, l)| PromptOption {
                    n: i as u32 + 1,
                    label: l.to_string(),
                })
                .collect::<Vec<_>>()
        };
        let claude = opts(&[
            "Yes",
            "Yes, and don't ask again for rm commands",
            "No, and tell Claude what to do differently",
        ]);
        use VoiceAnswer::*;
        assert_eq!(voice_answer("Yes.", &claude, Some(3)), Some(Pick(1)));
        assert_eq!(voice_answer("Allow", &claude, Some(3)), Some(Pick(1)));
        assert_eq!(voice_answer("Two", &claude, Some(3)), Some(Pick(2)));
        assert_eq!(voice_answer("option 3", &claude, Some(3)), Some(Pick(3)));
        assert_eq!(
            voice_answer(
                "Yes, and don't ask again for rm commands.",
                &claude,
                Some(3)
            ),
            Some(Pick(2))
        );
        assert_eq!(
            voice_answer("No, run the tests first.", &claude, Some(3)),
            Some(WithText(3, "run the tests first.".into()))
        );
        assert_eq!(
            voice_answer("Use cargo test instead", &claude, Some(3)),
            Some(WithText(3, "Use cargo test instead".into()))
        );
        // The test hub's prompt: plain Yes / No, nothing takes text.
        let plain = opts(&["Yes", "Yes, and don't ask again for echo commands", "No"]);
        assert_eq!(voice_answer("no", &plain, None), Some(Pick(3)));
        assert_eq!(voice_answer("Deny.", &plain, None), Some(Pick(3)));
        assert_eq!(voice_answer("Seven", &plain, None), None);
        assert_eq!(voice_answer("maybe later", &plain, None), None);
        assert_eq!(voice_answer("", &plain, None), None);
        // Codex asking which branch.
        let branch = opts(&["main", "win-wip", "A new branch"]);
        assert_eq!(voice_answer("Win wip", &branch, None), Some(Pick(2)));
        assert_eq!(voice_answer("A new branch.", &branch, None), Some(Pick(3)));
        assert_eq!(voice_answer("the first one", &branch, None), None);
    }

    #[test]
    fn plain_http_only_on_this_pc_or_a_vpn() {
        let ok = |s: &str| plain_http_ok(s.parse().unwrap());
        assert!(ok("127.0.0.1") && ok("::1"));
        assert!(ok("100.64.0.1") && ok("100.64.12.34") && ok("100.127.255.254"));
        assert!(ok("fd7a:115c:a1e0::1234") && ok("::ffff:100.64.1.2"));
        assert!(!ok("100.63.255.255") && !ok("100.128.0.1"));
        assert!(!ok("192.168.1.20") && !ok("10.0.0.5") && !ok("8.8.8.8"));
        assert!(!ok("fd7a:115c:a1e1::1") && !ok("fe80::1"));
        assert_eq!(
            host_port("http://[fd7a:115c:a1e0::5]:4777/"),
            Some(("fd7a:115c:a1e0::5".into(), 4777))
        );
        assert_eq!(
            host_port("https://hub.example.com"),
            Some(("hub.example.com".into(), 443))
        );
        assert_eq!(host_port("http://h:81"), Some(("h".into(), 81)));
        assert_eq!(host_port("http://u:p@h:81"), None);
        assert_eq!(host_port("http://:80"), None);
        assert_eq!(host_port("http://h:notaport"), None);
    }

    #[test]
    fn hub_addresses_are_tidied() {
        // A bare address is https unless it's this PC or a VPN address.
        assert_eq!(
            hub_url("192.168.1.20:4777/").as_deref(),
            Some("https://192.168.1.20:4777")
        );
        assert_eq!(
            hub_url("100.64.12.34:4777").as_deref(),
            Some("http://100.64.12.34:4777")
        );
        assert_eq!(
            hub_url("localhost:4777").as_deref(),
            Some("http://localhost:4777")
        );
        assert_eq!(
            hub_url("hub.example.com").as_deref(),
            Some("https://hub.example.com")
        );
        assert_eq!(
            hub_url(" https://hub.example.com ").as_deref(),
            Some("https://hub.example.com")
        );
        assert_eq!(hub_url(""), None);
        assert_eq!(hub_url("ftp://x"), None);
        assert_eq!(hub_url("http://"), None);
        assert_eq!(hub_url("two words"), None);
        // Nothing may hide another host or a path in the address.
        assert_eq!(hub_url("https://hub.example.com@evil.example"), None);
        assert_eq!(hub_url("100.64.12.34:4777@evil.example"), None);
        assert_eq!(hub_url("http://evil.example\\@100.64.12.34"), None);
        assert_eq!(hub_url("https://hub.example.com/x"), None);
        assert_eq!(hub_url("https://hub.example.com/?a=1"), None);
        assert_eq!(hub_url("https://hub.example.com/#/hosts"), None);
        assert_eq!(
            hub_url("HTTPS://Hub.Example.com:443/").as_deref(),
            Some("https://hub.example.com")
        );
        assert_eq!(
            hub_url("[fd7a:115c:a1e0::5]:4777").as_deref(),
            Some("http://[fd7a:115c:a1e0::5]:4777")
        );
        assert_eq!(
            ws_url("http://h:4777"),
            "ws://h:4777/ws?contract=1&catalog=2"
        );
        assert_eq!(ws_url("https://h"), "wss://h/ws?contract=1&catalog=2");
    }

    #[test]
    fn codes_and_routes() {
        assert_eq!(pairing_code("k7m-2qx").as_deref(), Some("K7M2QX"));
        assert_eq!(pairing_code("K7M 2QX").as_deref(), Some("K7M2QX"));
        assert_eq!(pairing_code("K7M2Q"), None);
        assert_eq!(
            pane_route("http://h", "my box", "%3", "answer"),
            "http://h/api/hosts/my%20box/panes/3/answer"
        );
    }

    #[test]
    fn frames_are_read() {
        let screen = r#"{"t":"chat.screen","key":"box:%3","screen":{"working":false,"prompt":{"body":["Bash command","rm -rf build"],"options":[{"n":1,"label":"Yes","selected":true},{"n":2,"label":"No","selected":false}],"multi":false,"key":"k1"}}}"#;
        let Some(Frame::Screen {
            pane: p,
            prompt: Some(q),
        }) = parse_frame(screen)
        else {
            panic!("chat.screen with a prompt");
        };
        assert_eq!(p, pane("%3"));
        assert_eq!(q.options.len(), 2);
        assert_eq!(q.key.as_deref(), Some("k1"));

        let idle = r#"{"t":"chat.screen","key":"box:%3","screen":{"working":true,"prompt":null}}"#;
        assert_eq!(
            parse_frame(idle),
            Some(Frame::Screen {
                pane: pane("%3"),
                prompt: None
            })
        );

        let notify = r#"{"t":"notify","host":"box","pane":"%3","kind":"waiting","title":"Claude needs you","body":"Bash: rm -rf build","agent":"claude"}"#;
        assert!(matches!(parse_frame(notify), Some(Frame::Notify(n)) if n.kind == "waiting"));

        let overview = r#"{"t":"overview","hosts":[{"config":{"id":"box","name":"Box"},"sessions":[{"name":"legato","windows":[{"name":"legato","panes":[{"id":"%3","agent":{"kind":"claude","state":"waiting"}},{"id":"%4","agent":null},{"id":"%5"}]}]}]}]}"#;
        assert_eq!(
            parse_frame(overview),
            Some(Frame::Overview(vec![agent("%3", true)]))
        );

        assert_eq!(parse_frame(r#"{"t":"pong"}"#), Some(Frame::Pong));
        let reply = r#"{"t":"companion.reply","id":"r7","host":"build-box","pane":"%12","text":" Two sessions wait for you. ","at":1792866545000}"#;
        assert_eq!(
            parse_frame(reply),
            Some(Frame::CompanionReply(CompanionReply {
                id: "r7".into(),
                host: "build-box".into(),
                pane: "%12".into(),
                text: "Two sessions wait for you.".into(),
                error: false,
            }))
        );
        let failed = r#"{"t":"companion.reply","id":"r8","host":"h","pane":"%1","text":"Not signed in","error":true}"#;
        assert!(matches!(parse_frame(failed), Some(Frame::CompanionReply(r)) if r.error));
        // A reply without an id can't be told from a repeat.
        assert_eq!(
            parse_frame(r#"{"t":"companion.reply","host":"h","pane":"%1","text":"x"}"#),
            None
        );
        assert_eq!(parse_frame(r#"{"t":"something.new"}"#), None);
        assert_eq!(parse_frame("not json"), None);
    }

    #[test]
    fn the_oldest_question_shows_and_answers_move_on() {
        let mut w = Waiting::default();
        let (start, stop) = w.overview(vec![
            agent("%3", true),
            agent("%4", true),
            agent("%5", false),
        ]);
        assert_eq!(start, vec![pane("%3"), pane("%4")]);
        assert!(stop.is_empty());

        w.screen(pane("%4"), Some(prompt("k4")));
        w.screen(pane("%3"), Some(prompt("k3")));
        let first = w.current().expect("a question shows");
        assert_eq!((first.pane.as_str(), first.more), ("%4", 1));

        // A new screen for the same pane keeps its place.
        w.screen(pane("%4"), Some(prompt("k4b")));
        assert_eq!(w.current().unwrap().key.as_deref(), Some("k4b"));

        // A stale key doesn't remove the newer question.
        w.answered(&pane("%4"), Some("k4"));
        assert_eq!(w.current().unwrap().pane, "%4");
        w.answered(&pane("%4"), Some("k4b"));
        assert_eq!(w.current().unwrap().pane, "%3");

        // Answered in the terminal: the overview stops waiting, the card goes.
        let (_, stop) = w.overview(vec![agent("%3", false), agent("%4", false)]);
        assert_eq!(stop, vec![pane("%3"), pane("%4")]);
        assert_eq!(w.current(), None);
    }

    #[test]
    fn a_screen_without_a_question_clears_it() {
        let mut w = Waiting::default();
        w.overview(vec![agent("%3", true)]);
        w.screen(pane("%3"), Some(prompt("k")));
        w.screen(pane("%3"), None);
        assert_eq!(w.current(), None);
    }

    #[test]
    fn only_watched_panes_queue_and_questions_are_trimmed() {
        let mut w = Waiting::default();
        w.overview(vec![agent("%3", true), agent("%5", false)]);
        w.screen(pane("%9"), Some(prompt("k")));
        w.screen(pane("%5"), Some(prompt("k")));
        assert_eq!(w.current(), None);

        let mut long = prompt("k");
        long.body = vec!["x".repeat(2000); 100];
        long.options = (0..50)
            .map(|n| PromptOption {
                n,
                label: "é".repeat(900),
            })
            .collect();
        w.screen(pane("%3"), Some(long));
        let alert = w.current().unwrap();
        assert!(alert.options.len() <= MAX_OPTIONS);
        assert!(alert.options[0].label.chars().count() == MAX_LINE_CHARS);
        assert!(alert.body.len() + alert.command.len() + alert.ask.len() <= MAX_LINES);
    }

    #[test]
    fn the_hub_names_the_option_that_takes_text() {
        let frame = r#"{"t":"chat.screen","key":"box:%3","screen":{"prompt":{"body":["Edit file"],"options":[{"n":1,"label":"Yes"},{"n":3,"label":"No, and tell Claude what to do differently"}],"key":"k","freeTextOption":3}}}"#;
        let Some(Frame::Screen { prompt, .. }) = parse_frame(frame) else {
            panic!("chat.screen");
        };
        assert_eq!(prompt.and_then(|p| p.free_text), Some(3));
    }

    #[test]
    fn terminal_text_drops_control_characters() {
        assert_eq!(
            terminal_text("a\u{1b}[201~b\r\nc\rd\te\u{7f}\u{9b}f\n"),
            "a[201~b\nc\nd\tef\n"
        );
    }

    #[test]
    fn reconnects_back_off_to_a_minute() {
        assert_eq!(backoff_secs(0), 1);
        assert_eq!(backoff_secs(3), 8);
        assert_eq!(backoff_secs(10), 60);
    }

    #[test]
    fn reasons_have_messages() {
        assert_eq!(reason_key(409, Some("already-answered")), "alreadyAnswered");
        assert_eq!(reason_key(401, None), "signedOut");
        assert_eq!(reason_key(500, None), "failed");
    }

    #[test]
    fn a_pairing_link_carries_the_hub_the_code_and_its_certificate() {
        let fp = "AB".repeat(32);
        let link = pairing_link(&format!(
            "inyon://companion?u=https%3A%2F%2Fhub.example.com%3A4777&c=k7q-2xp&f={fp}"
        ))
        .unwrap();
        assert_eq!(link.hub, "https://hub.example.com:4777");
        assert_eq!(link.code, "K7Q2XP");
        assert_eq!(link.fingerprint, Some("ab".repeat(32)));
        // No fingerprint on plain http, and none needed.
        let plain =
            pairing_link("inyon://companion?u=http%3A%2F%2F100.64.1.2%3A4777&c=K7Q2XP&f=00")
                .map(|l| l.fingerprint);
        assert_eq!(plain, None, "a bad fingerprint refuses the link");
        let plain = pairing_link(&format!(
            "inyon://companion?u=http%3A%2F%2F100.64.1.2%3A4777&c=K7Q2XP&f={fp}"
        ))
        .unwrap();
        assert_eq!(plain.fingerprint, None);
        assert!(
            pairing_link("inyon://companion?u=https%3A%2F%2Fhub.example.com&c=K7Q2XP")
                .is_some_and(|l| l.fingerprint.is_none())
        );
        // Not a link, not Inyon's, a hub with a path, or no code.
        assert_eq!(pairing_link("K7Q2XP"), None);
        assert_eq!(pairing_link("https://hub.example.com/?c=K7Q2XP"), None);
        assert_eq!(
            pairing_link("inyon://other?u=https%3A%2F%2Fhub.example.com&c=K7Q2XP"),
            None
        );
        assert_eq!(
            pairing_link("inyon://companion?u=https%3A%2F%2Fhub.example.com%2Fx&c=K7Q2XP"),
            None
        );
        assert_eq!(
            pairing_link("inyon://companion?u=hub.example.com&c=K7Q2XP"),
            None
        );
        assert_eq!(
            pairing_link("inyon://companion?u=https%3A%2F%2Fhub.example.com"),
            None
        );
    }

    #[test]
    fn a_claim_may_carry_the_hub_certificate() {
        let with: Claimed = serde_json::from_str(
            r#"{"token":"t","hubId":"h","contractVersion":1,"tlsFingerprint":"ab"}"#,
        )
        .unwrap();
        assert_eq!(with.tls_fingerprint.as_deref(), Some("ab"));
        let without: Claimed =
            serde_json::from_str(r#"{"token":"t","hubId":"h","contractVersion":1}"#).unwrap();
        assert_eq!(without.tls_fingerprint, None);
        assert_eq!(tls_fingerprint(&"F0".repeat(32)), Some("f0".repeat(32)));
        assert_eq!(tls_fingerprint(&"g0".repeat(32)), None);
        assert_eq!(tls_fingerprint("abcd"), None);
    }
}
