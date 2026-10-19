//! Connections between paired devices: the PC's TLS listener, the phone's
//! side (used by the stand-in phone and the tests), and the session both run
//! once they've said hello. What a device stores and shows is the `Host`'s
//! business, so the same session code serves the PC and the stand-in.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, watch};
use tokio_rustls::{TlsAcceptor, TlsConnector};

use super::identity::{fingerprint, server_name, Identity};
use super::protocol::{self, Change, Clip, Hello, Message, Refusal, Welcome, PROTOCOL};

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

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
    /// `peer` connected from `addr`, or went away (None).
    fn online(&self, _peer: &str, _addr: Option<SocketAddr>) {}
    /// `peer` unpaired us.
    fn forgotten(&self, _peer: &str) {}
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
            accepted = listener.accept() => accepted?,
            _ = stop.changed() => return Ok(()),
        };
        let (acceptor, host, sessions, stop) = (
            acceptor.clone(),
            host.clone(),
            sessions.clone(),
            stop.clone(),
        );
        tokio::spawn(async move {
            if let Err(e) = welcome_phone(tcp, addr, acceptor, host, sessions, stop).await {
                log::info!("Phone connection from {addr} ended: {e:#}");
            }
        });
    }
}

async fn welcome_phone(
    tcp: TcpStream,
    addr: SocketAddr,
    acceptor: TlsAcceptor,
    host: Arc<dyn Host>,
    sessions: Sessions,
    stop: watch::Receiver<bool>,
) -> Result<()> {
    tcp.set_nodelay(true)?;
    let mut tls = tokio::time::timeout(HANDSHAKE_TIMEOUT, acceptor.accept(tcp))
        .await
        .context("TLS handshake timed out")??;
    let fp = tls
        .get_ref()
        .1
        .peer_certificates()
        .and_then(|certs| certs.first())
        .map(fingerprint)
        .ok_or_else(|| anyhow!("no client certificate"))?;
    let hello = match tokio::time::timeout(HANDSHAKE_TIMEOUT, protocol::read(&mut tls)).await {
        Ok(Ok(Message::Hello(hello))) => hello,
        Ok(Ok(other)) => bail!("expected hello, got {other:?}"),
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
        bail!("refused {} ({reason:?})", hello.name);
    }
    protocol::write(&mut tls, &Message::Welcome(host.describe())).await?;
    let session = sessions.insert(&hello.device_id);
    host.online(&hello.device_id, Some(addr));
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

/// The phone's side: connects to the first address that answers, says
/// hello (with `secret` when pairing) and runs the session until it ends.
/// Commands reach it through `sessions` under the PC's device ID.
#[allow(dead_code)] // Used by tools/stand-in-phone, not the app.
pub async fn connect(
    addrs: &[String],
    pc_fingerprint: &str,
    identity: &Identity,
    secret: Option<String>,
    host: Arc<dyn Host>,
    sessions: Sessions,
    stop: watch::Receiver<bool>,
) -> Result<Welcome> {
    let connector = TlsConnector::from(identity.client_config(pc_fingerprint)?);
    let mut last = anyhow!("no address to try");
    let mut tcp = None;
    for addr in addrs {
        match tokio::time::timeout(Duration::from_secs(4), TcpStream::connect(addr)).await {
            Ok(Ok(stream)) => {
                tcp = Some(stream);
                break;
            }
            Ok(Err(e)) => last = anyhow!("{addr}: {e}"),
            Err(_) => last = anyhow!("{addr}: timed out"),
        }
    }
    let tcp = tcp.ok_or(last)?;
    tcp.set_nodelay(true)?;
    let mut tls = connector.connect(server_name(), tcp).await?;
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
        Ok(Ok(Message::Refused { reason })) => bail!("the PC refused: {reason:?}"),
        Ok(Ok(other)) => bail!("expected welcome, got {other:?}"),
        Ok(Err(e)) => return Err(e),
        Err(_) => bail!("no welcome"),
    };
    host.welcomed(&welcome, pc_fingerprint);
    let session = sessions.insert(&welcome.device_id);
    run(
        tls,
        host.clone(),
        &welcome.device_id,
        session,
        &sessions,
        stop,
    )
    .await?;
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
                            if more {
                                protocol::write(&mut wr, &pull(&host)).await?;
                            }
                        }
                        Message::Nudge => protocol::write(&mut wr, &pull(&host)).await?,
                        Message::Clip(clip) => host.clip(peer, clip),
                        Message::Ping => protocol::write(&mut wr, &Message::Pong).await?,
                        Message::Welcome(welcome) => host.welcomed(&welcome, ""),
                        Message::Bye { reason } => {
                            log::info!("{peer} said goodbye: {reason}");
                            host.forgotten(peer);
                            return Ok(());
                        }
                        Message::Pong | Message::Hello(_) | Message::Refused { .. } | Message::Unknown => {}
                    }
                }
                command = commands.recv() => {
                    let Some(command) = command else { return Ok(()) };
                    match command {
                        Command::Nudge => protocol::write(&mut wr, &Message::Nudge).await?,
                        Command::Pull => protocol::write(&mut wr, &pull(&host)).await?,
                        Command::Clip(clip) => protocol::write(&mut wr, &Message::Clip(clip)).await?,
                        Command::Welcome(welcome) => protocol::write(&mut wr, &Message::Welcome(welcome)).await?,
                        Command::Bye(reason) => {
                            protocol::write(&mut wr, &Message::Bye { reason }).await?;
                            return Ok(());
                        }
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
