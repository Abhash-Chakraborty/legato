//! Connections between paired devices: the PC's TLS listener, the phone's
//! side (used by the stand-in phone and the tests), and the session both run
//! once they've said hello. The wire is TCP over Wi-Fi or Netbird, or
//! Bluetooth when they share no network; TLS runs the same over either. What a device stores and shows is the `Host`'s
//! business, so the same session code serves the PC and the stand-in.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, watch};
use tokio_rustls::{TlsAcceptor, TlsConnector};

use super::identity::{fingerprint, secret, server_name, Identity};
use super::protocol::{self, Change, Clip, Hello, Message, Refusal, Welcome, PROTOCOL};

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
/// Connections allowed to be in their handshake at once; more are dropped
/// until one is admitted or gives up, so a flood can't pile up.
const MAX_PENDING: usize = 8;
static PENDING: AtomicUsize = AtomicUsize::new(0);

/// One connection still in its handshake, counted against `MAX_PENDING`.
pub struct Pending;

impl Pending {
    /// None when `MAX_PENDING` connections are already in their handshake.
    pub fn start() -> Option<Self> {
        if PENDING.fetch_add(1, Ordering::SeqCst) < MAX_PENDING {
            Some(Pending)
        } else {
            PENDING.fetch_sub(1, Ordering::SeqCst);
            None
        }
    }
}

impl Drop for Pending {
    fn drop(&mut self) {
        PENDING.fetch_sub(1, Ordering::SeqCst);
    }
}
/// How long the phone waits for any network address before Bluetooth.
const NET_TIMEOUT: Duration = Duration::from_secs(4);

/// The PC answered and said no: a pairing secret that's wrong or used up,
/// or a phone it doesn't know any more. Only this ends a pairing; a PC
/// that can't be reached (off, asleep, Legato closed, a firewall) is
/// tried again later.
#[derive(Debug)]
pub struct Refused(pub String);

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "the PC refused: {}", self.0)
    }
}

impl std::error::Error for Refused {}

/// The phone's hello never left: no address answered, or the TLS handshake
/// failed. A pairing secret is still unspent, so the next try sends it.
#[derive(Debug)]
pub struct Unreachable(pub String);

impl std::fmt::Display for Unreachable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Unreachable {}

/// How a device reached the other.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Via {
    /// Over TCP from this address (home network or Netbird).
    Net(SocketAddr),
    Bluetooth,
}

/// The phone app's way to open Bluetooth: given the PC's Bluetooth address
/// ("AA:BB:CC:DD:EE:FF") and a one-time token, it connects and returns a
/// port on 127.0.0.1 that carries the link, or None. It blocks while it
/// connects. The port carries only a connection that sends the token first,
/// so another app on the phone that finds the port can't take the link.
pub type Dialer = Arc<dyn Fn(&str, &str) -> Option<u16> + Send + Sync>;

/// Transcribing a recording, which takes a while: run off the session.
pub type TranscribeJob = Box<dyn FnOnce() -> std::result::Result<String, String> + Send>;

/// What a device does with what arrives. Methods are quick database work and
/// run on the session's task.
pub trait Host: Send + Sync + 'static {
    /// This device, as its `welcome` (or, on a phone, its `hello`) says.
    fn describe(&self) -> Welcome;
    /// The PC's side: may this device in? Pairs it when its secret is right.
    fn admit(&self, hello: &Hello, fingerprint: &str) -> std::result::Result<(), Refusal>;
    /// The phone's side: the PC said welcome (again, if what it syncs changed).
    fn welcomed(&self, _welcome: &Welcome, _fingerprint: &str) {}
    /// Our changes after `after`, for `peer`; also records that `peer` holds
    /// everything up to `after`.
    fn changes(
        &self,
        peer: &str,
        after: u64,
        kinds: &[String],
        limit: usize,
    ) -> Result<(Vec<Change>, u64, bool)>;
    /// Takes `peer`'s changes, then records `upto` as held.
    fn accept(&self, peer: &str, items: Vec<Change>, upto: u64) -> Result<()>;
    /// The newest of `peer`'s changes we hold.
    fn recv_upto(&self, peer: &str) -> u64;
    /// The kinds to ask `peer` for.
    fn wanted(&self, peer: &str) -> Vec<String>;
    fn clip(&self, _peer: &str, _clip: Clip) {}
    /// `peer` connected, and how, or went away (None).
    fn online(&self, _peer: &str, _via: Option<Via>) {}
    /// `peer` unpaired us.
    fn forgotten(&self, _peer: &str) {}
    /// The PC's side: how to transcribe `peer`'s recording, if this device
    /// can, for an app of the kind `app`; the session runs it and sends the
    /// answer.
    fn transcribe(
        &self,
        _peer: &str,
        _samples: Vec<f32>,
        _app: Option<String>,
    ) -> Option<TranscribeJob> {
        None
    }
    /// The phone's side: the answer to a `transcribe` it sent.
    fn transcript(&self, _peer: &str, _id: &str, _result: std::result::Result<String, String>) {}
}

/// Something to send down a live session.
#[derive(Clone, Debug)]
pub enum Command {
    /// "I have new changes."
    Nudge,
    /// Ask for theirs now.
    Pull,
    Clip(Clip),
    /// What this device syncs changed.
    Welcome(Welcome),
    /// Say goodbye with this reason and close.
    Bye(String),
    /// Send this message as it is.
    Send(Message),
}

/// Live sessions by device ID, each with a number so a session that ends
/// after its device reconnected doesn't remove the new one.
#[derive(Clone, Default)]
pub struct Sessions(Arc<Mutex<Live>>);

/// The last session number, and each device's session.
type Live = (u64, HashMap<String, (u64, mpsc::Sender<Command>)>);

impl Sessions {
    fn lock(&self) -> std::sync::MutexGuard<'_, Live> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Sends to one device; false when it isn't connected.
    pub fn send(&self, device_id: &str, command: Command) -> bool {
        let sender = self.lock().1.get(device_id).map(|(_, s)| s.clone());
        sender.is_some_and(|s| s.try_send(command).is_ok())
    }

    pub fn broadcast(&self, command: Command) {
        let senders: Vec<_> = self.lock().1.values().map(|(_, s)| s.clone()).collect();
        for sender in senders {
            let _ = sender.try_send(command.clone());
        }
    }

    pub fn connected(&self, device_id: &str) -> bool {
        self.lock().1.contains_key(device_id)
    }

    /// Ends every session.
    pub fn close_all(&self) {
        self.lock().1.clear();
    }

    /// Ends this device's session here: commands already queued (a
    /// goodbye) still go, then the link drops.
    pub fn close(&self, device_id: &str) {
        self.lock().1.remove(device_id);
    }

    fn insert(&self, device_id: &str) -> (u64, mpsc::Receiver<Command>) {
        let (tx, rx) = mpsc::channel(64);
        let mut guard = self.lock();
        guard.0 += 1;
        let number = guard.0;
        // A device that reconnects replaces its old session, which ends when
        // its sender drops.
        guard.1.insert(device_id.to_string(), (number, tx));
        (number, rx)
    }

    fn remove(&self, device_id: &str, number: u64) {
        let mut guard = self.lock();
        if guard.1.get(device_id).is_some_and(|(n, _)| *n == number) {
            guard.1.remove(device_id);
        }
    }
}

/// Binds the first free port from `first`, on every interface.
pub async fn bind(first: u16) -> Result<TcpListener> {
    let mut last = None;
    for port in first..first.saturating_add(protocol::PORT_TRIES) {
        match TcpListener::bind(("0.0.0.0", port)).await {
            Ok(listener) => return Ok(listener),
            Err(e) => last = Some(e),
        }
    }
    Err(anyhow!("no free port from {first}: {last:?}"))
}

/// The PC's side: accepts phones until `stop` turns true.
pub async fn serve(
    listener: TcpListener,
    identity: Identity,
    host: Arc<dyn Host>,
    sessions: Sessions,
    mut stop: watch::Receiver<bool>,
) -> Result<()> {
    let acceptor = TlsAcceptor::from(identity.server_config()?);
    loop {
        let (tcp, addr) = tokio::select! {
            accepted = listener.accept() => match accepted {
                Ok(accepted) => accepted,
                // A connection reset before it was accepted, or the system
                // short of sockets: keep listening.
                Err(e) => {
                    log::debug!("Phone sync accept failed: {e}");
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    continue;
                }
            },
            _ = stop.changed() => return Ok(()),
        };
        if !protocol::local_peer(addr.ip()) {
            log::debug!("Phone sync: dropped a connection from {addr}, not a local or VPN address");
            continue;
        }
        let Some(pending) = Pending::start() else {
            log::debug!("Phone sync: dropped a connection from {addr}, too many handshakes");
            continue;
        };
        let (acceptor, host, sessions, stop) = (
            acceptor.clone(),
            host.clone(),
            sessions.clone(),
            stop.clone(),
        );
        tokio::spawn(async move {
            let _ = tcp.set_nodelay(true);
            if let Err(e) =
                welcome_phone(tcp, Via::Net(addr), acceptor, host, sessions, stop, pending).await
            {
                // A port probe or a phone that just dropped the socket isn't
                // worth a line at info level.
                let text = format!("{e:#}");
                if text.contains("close_notify") || text.contains("handshake eof") {
                    log::debug!("Phone connection from {addr} ended: {text}");
                } else {
                    log::info!("Phone connection from {addr} ended: {text}");
                }
            }
        });
    }
}

/// The PC's side of one phone that came in some other way than the TCP
/// listener (Bluetooth): the same handshake and session.
pub async fn serve_stream<S>(
    stream: S,
    via: Via,
    identity: &Identity,
    host: Arc<dyn Host>,
    sessions: Sessions,
    stop: watch::Receiver<bool>,
    pending: Pending,
) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let acceptor = TlsAcceptor::from(identity.server_config()?);
    welcome_phone(stream, via, acceptor, host, sessions, stop, pending).await
}

async fn welcome_phone<S>(
    stream: S,
    via: Via,
    acceptor: TlsAcceptor,
    host: Arc<dyn Host>,
    sessions: Sessions,
    stop: watch::Receiver<bool>,
    pending: Pending,
) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let mut tls = tokio::time::timeout(HANDSHAKE_TIMEOUT, acceptor.accept(stream))
        .await
        .context("TLS handshake timed out")??;
    let fp = tls
        .get_ref()
        .1
        .peer_certificates()
        .and_then(|certs| certs.first())
        .map(fingerprint)
        .ok_or_else(|| anyhow!("no client certificate"))?;
    let first = protocol::read_limited(&mut tls, protocol::MAX_HELLO);
    let hello = match tokio::time::timeout(HANDSHAKE_TIMEOUT, first).await {
        Ok(Ok(Message::Hello(hello))) => hello,
        // Not the frame itself: a stranger chooses what's in it.
        Ok(Ok(_)) => bail!("expected hello first"),
        Ok(Err(e)) => return Err(e),
        Err(_) => bail!("no hello"),
    };
    let verdict = if hello.protocol != PROTOCOL {
        Err(Refusal::Protocol)
    } else {
        host.admit(&hello, &fp)
    };
    if let Err(reason) = verdict {
        protocol::write(&mut tls, &Message::Refused { reason }).await?;
        let name: String = hello.name.chars().take(40).collect();
        bail!("refused {name:?} ({reason:?})");
    }
    drop(pending);
    protocol::write(&mut tls, &Message::Welcome(host.describe())).await?;
    let session = sessions.insert(&hello.device_id);
    host.online(&hello.device_id, Some(via));
    let result = run(
        tls,
        host.clone(),
        &hello.device_id,
        session,
        &sessions,
        stop,
    )
    .await;
    host.online(&hello.device_id, None);
    result
}

/// The first network address to answer, trying them all at once.
async fn first_answer(addrs: &[&String]) -> Result<(TcpStream, SocketAddr)> {
    let mut tries = tokio::task::JoinSet::new();
    for addr in addrs {
        let addr = addr.to_string();
        tries.spawn(async move {
            match tokio::time::timeout(NET_TIMEOUT, TcpStream::connect(&addr)).await {
                Ok(Ok(stream)) => {
                    let peer = stream.peer_addr()?;
                    Ok((stream, peer))
                }
                Ok(Err(e)) => Err(anyhow!("{addr}: {e}")),
                Err(_) => Err(anyhow!("{addr}: timed out")),
            }
        });
    }
    let mut last = anyhow!("no network address to try");
    while let Some(done) = tries.join_next().await {
        match done {
            Ok(Ok(found)) => return Ok(found),
            Ok(Err(e)) => last = e,
            Err(e) => last = anyhow!(e),
        }
    }
    Err(last)
}

/// Whether any of the PC's network addresses (not Bluetooth) answers now.
#[allow(dead_code)] // Used by the phone (phone/core), not the app.
pub async fn network_answers(addrs: &[String]) -> bool {
    let net: Vec<&String> = addrs
        .iter()
        .filter(|a| !a.starts_with(protocol::BT_PREFIX))
        .collect();
    !net.is_empty() && first_answer(&net).await.is_ok()
}

/// Opens the wire to the PC: any network address that answers, else
/// Bluetooth through the app's `dial`.
async fn reach(addrs: &[String], dial: Option<Dialer>) -> Result<(TcpStream, Via)> {
    let (bluetooth, net): (Vec<&String>, Vec<&String>) = addrs
        .iter()
        .partition(|a| a.starts_with(protocol::BT_PREFIX));
    let mut last = match first_answer(&net).await {
        Ok((stream, addr)) => return Ok((stream, Via::Net(addr))),
        Err(e) => e,
    };
    let Some(dial) = dial.filter(|_| !bluetooth.is_empty()) else {
        return Err(last);
    };
    for addr in bluetooth {
        let mac = addr[protocol::BT_PREFIX.len()..].to_string();
        let dial = dial.clone();
        let token = secret();
        let port = {
            let token = token.clone();
            tokio::task::spawn_blocking(move || dial(&mac, &token))
                .await
                .ok()
                .flatten()
        };
        let Some(port) = port else {
            last = anyhow!("{last:#}; Bluetooth didn't connect");
            continue;
        };
        let opened = async {
            let mut stream = TcpStream::connect(("127.0.0.1", port)).await?;
            stream.write_all(token.as_bytes()).await?;
            std::io::Result::Ok(stream)
        };
        match opened.await {
            Ok(stream) => return Ok((stream, Via::Bluetooth)),
            Err(e) => last = anyhow!("Bluetooth: {e}"),
        }
    }
    Err(last)
}

/// The phone's side: reaches the PC (see `reach`), says hello (with
/// `secret` when pairing) and runs the session until it ends. Commands
/// reach it through `sessions` under the PC's device ID.
#[allow(dead_code)] // Used by the phone (phone/core), not the app.
#[allow(clippy::too_many_arguments)]
pub async fn connect(
    addrs: &[String],
    pc_fingerprint: &str,
    identity: &Identity,
    secret: Option<String>,
    dial: Option<Dialer>,
    host: Arc<dyn Host>,
    sessions: Sessions,
    stop: watch::Receiver<bool>,
) -> Result<Welcome> {
    let connector = TlsConnector::from(identity.client_config(pc_fingerprint)?);
    let (tcp, via) = reach(addrs, dial)
        .await
        .map_err(|e| Unreachable(format!("{e:#}")))?;
    tcp.set_nodelay(true)?;
    let mut tls = connector
        .connect(server_name(), tcp)
        .await
        .map_err(|e| Unreachable(format!("TLS: {e}")))?;
    let me = host.describe();
    let hello = Hello {
        protocol: PROTOCOL,
        device_id: me.device_id,
        name: me.name,
        platform: me.platform,
        app_version: me.app_version,
        secret,
    };
    protocol::write(&mut tls, &Message::Hello(hello)).await?;
    let welcome = match tokio::time::timeout(HANDSHAKE_TIMEOUT, protocol::read(&mut tls)).await {
        Ok(Ok(Message::Welcome(w))) => w,
        Ok(Ok(Message::Refused { reason })) => return Err(Refused(format!("{reason:?}")).into()),
        Ok(Ok(other)) => bail!("expected welcome, got {other:?}"),
        Ok(Err(e)) => return Err(e),
        Err(_) => bail!("no welcome"),
    };
    host.welcomed(&welcome, pc_fingerprint);
    let session = sessions.insert(&welcome.device_id);
    host.online(&welcome.device_id, Some(via));
    let result = run(
        tls,
        host.clone(),
        &welcome.device_id,
        session,
        &sessions,
        stop,
    )
    .await;
    host.online(&welcome.device_id, None);
    result?;
    Ok(welcome)
}

/// One session after the handshake. Both sides pull at once, answer pulls,
/// pull again after a nudge, and ping to notice a dead connection.
async fn run<S>(
    stream: S,
    host: Arc<dyn Host>,
    peer: &str,
    (number, mut commands): (u64, mpsc::Receiver<Command>),
    sessions: &Sessions,
    mut stop: watch::Receiver<bool>,
) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let (mut rd, mut wr) = tokio::io::split(stream);
    // Reads go through a task, since a read cut off by select! loses bytes.
    let (in_tx, mut incoming) = mpsc::channel::<Result<Message>>(16);
    let reader = tokio::spawn(async move {
        loop {
            let message = protocol::read(&mut rd).await;
            let failed = message.is_err();
            if in_tx.send(message).await.is_err() || failed {
                break;
            }
        }
    });
    let pull = |host: &Arc<dyn Host>| Message::Pull {
        after: host.recv_upto(peer),
        kinds: host.wanted(peer),
        limit: Some(protocol::PULL_LIMIT),
    };
    let mut ping = tokio::time::interval(protocol::PING_EVERY);
    ping.tick().await;
    let mut heard = Instant::now();
    // One pull in flight at a time: nudges that arrive while the answer is
    // still coming are folded into a single pull afterwards, so three quick
    // edits on the other side don't fetch the same changes three times.
    let mut pulling = true;
    let mut pull_again = false;
    protocol::write(&mut wr, &pull(&host)).await?;
    let result: Result<()> = async {
        loop {
            tokio::select! {
                message = incoming.recv() => {
                    let Some(message) = message else { return Ok(()) };
                    let message = message?;
                    heard = Instant::now();
                    match message {
                        Message::Pull { after, kinds, limit } => {
                            let limit = limit.unwrap_or(protocol::PULL_LIMIT).clamp(1, protocol::PULL_LIMIT);
                            let (items, upto, more) = host.changes(peer, after, &kinds, limit)?;
                            protocol::write(&mut wr, &Message::Changes { items, upto, more }).await?;
                        }
                        Message::Changes { items, upto, more } => {
                            host.accept(peer, items, upto)?;
                            if more || std::mem::take(&mut pull_again) {
                                protocol::write(&mut wr, &pull(&host)).await?;
                            } else {
                                pulling = false;
                            }
                        }
                        Message::Nudge => {
                            if pulling {
                                pull_again = true;
                            } else {
                                pulling = true;
                                protocol::write(&mut wr, &pull(&host)).await?;
                            }
                        }
                        Message::Clip(clip) => host.clip(peer, clip),
                        Message::Ping => protocol::write(&mut wr, &Message::Pong).await?,
                        Message::Welcome(welcome) => host.welcomed(&welcome, ""),
                        Message::Bye { reason } => {
                            log::info!("{peer} said goodbye: {reason}");
                            host.forgotten(peer);
                            return Ok(());
                        }
                        Message::Transcribe { id, pcm, app } => {
                            let job = protocol::pcm_decode(&pcm)
                                .ok()
                                .and_then(|samples| host.transcribe(peer, samples, app));
                            match job {
                                Some(job) => {
                                    let (sessions, peer) = (sessions.clone(), peer.to_string());
                                    tokio::task::spawn_blocking(move || {
                                        let (text, error) = match job() {
                                            Ok(text) => (Some(text), None),
                                            Err(e) => (None, Some(e)),
                                        };
                                        sessions.send(&peer, Command::Send(Message::Transcript { id, text, error }));
                                    });
                                }
                                None => {
                                    let error = Some("This device doesn't transcribe".to_string());
                                    protocol::write(&mut wr, &Message::Transcript { id, text: None, error }).await?;
                                }
                            }
                        }
                        Message::Transcript { id, text, error } => {
                            let result = text.ok_or_else(|| error.unwrap_or_else(|| "No text".into()));
                            host.transcript(peer, &id, result);
                        }
                        Message::Pong | Message::Hello(_) | Message::Refused { .. } | Message::Unknown => {}
                    }
                }
                command = commands.recv() => {
                    let Some(command) = command else { return Ok(()) };
                    match command {
                        Command::Nudge => protocol::write(&mut wr, &Message::Nudge).await?,
                        Command::Pull => {
                            if pulling {
                                pull_again = true;
                            } else {
                                pulling = true;
                                protocol::write(&mut wr, &pull(&host)).await?;
                            }
                        }
                        Command::Clip(clip) => protocol::write(&mut wr, &Message::Clip(clip)).await?,
                        Command::Welcome(welcome) => protocol::write(&mut wr, &Message::Welcome(welcome)).await?,
                        Command::Bye(reason) => {
                            protocol::write(&mut wr, &Message::Bye { reason }).await?;
                            return Ok(());
                        }
                        Command::Send(message) => protocol::write(&mut wr, &message).await?,
                    }
                }
                _ = ping.tick() => {
                    if heard.elapsed() > protocol::SILENCE_LIMIT {
                        bail!("{peer} went quiet");
                    }
                    protocol::write(&mut wr, &Message::Ping).await?;
                }
                _ = stop.changed() => return Ok(()),
            }
        }
    }
    .await;
    reader.abort();
    sessions.remove(peer, number);
    result
}
