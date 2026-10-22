//! Phone sync on the PC: pairing a phone with a QR code, keeping both copies
//! of the Dictionary, snippets, history, notes, Scratchpad, transforms and
//! Style up to date over the home network, Netbird or Bluetooth, and the shared
//! clipboard. docs/phone-sync.md has the protocol; the modules below it are
//! plain code the stand-in phone (phone/core) runs too.

mod bluetooth;
pub mod clipboard;
pub mod engine;
pub mod hub;
pub mod identity;
pub mod kinds;
pub mod ledger;
pub mod link;
mod net;
pub mod protocol;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{anyhow, Result};
use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use serde::Serialize;
use serde_json::{json, Value};
use specta::Type;
use tauri::{AppHandle, Emitter, Listener, Manager};
use tauri_specta::Event;
use tokio::sync::{watch, Notify};

use crate::managers::history::{HistoryManager, HistoryUpdatePayload};
use crate::settings::{get_settings, write_settings, AppSettings};
use engine::{now_s, Accepted, Engine, Extra};
use hub::{Config, Hub};
use identity::Identity;
use ledger::{ClipRow, Item, Merge, Peer};
use link::{Command, Host, Sessions, Via};
use protocol::{Clip, ClipKind, PairLink, Refusal};

/// Credential Manager name of this PC's sync identity.
const IDENTITY_SECRET: &str = "sync-identity";
/// How often a connected phone is checked for local changes, besides right
/// after the app reports one.
const REFRESH_EVERY: Duration = Duration::from_secs(15);
const STYLE_KINDS: [&str; 7] = [
    "personal",
    "work",
    "email",
    "documents",
    "ai",
    "code",
    "other",
];

/// Sent when anything Settings › Devices shows changed.
#[derive(Clone, Debug, Serialize, Type, tauri_specta::Event)]
pub struct SyncChanged;

/// An address the phone can reach this PC on.
#[derive(Clone, Debug, Serialize, Type)]
pub struct SyncAddress {
    /// "lan", "netbird" or "bluetooth".
    pub kind: String,
    /// "192.168.1.20:47613", or "bt:AA:BB:CC:DD:EE:FF" for Bluetooth.
    pub addr: String,
}

#[derive(Clone, Debug, Serialize, Type)]
pub struct PhoneState {
    pub peer: Peer,
    pub online: bool,
    /// How it's connected: "lan", "netbird" or "bluetooth".
    pub via: Option<String>,
}

#[derive(Clone, Debug, Serialize, Type)]
pub struct KindCount {
    pub kind: String,
    pub count: u32,
}

/// Everything Settings › Devices shows.
#[derive(Clone, Debug, Serialize, Type)]
pub struct SyncState {
    pub pc_name: String,
    pub addresses: Vec<SyncAddress>,
    /// Listening for phones; false with no phone paired and no pairing open.
    pub listening: bool,
    /// Why listening failed, if it did.
    pub listen_error: Option<String>,
    pub pairing: bool,
    pub phones: Vec<PhoneState>,
    pub counts: Vec<KindCount>,
    /// Merges logged so far (See merges).
    pub merges: u32,
}

/// What Pair a phone shows.
#[derive(Clone, Debug, Serialize, Type)]
pub struct PairingInfo {
    pub link: String,
    /// The link as a QR code, an SVG document.
    pub qr_svg: String,
    pub minutes: u32,
    pub pc_name: String,
    pub addresses: Vec<SyncAddress>,
}

struct Listening {
    port: u16,
    stop: watch::Sender<bool>,
}

pub struct SyncManager {
    app: AppHandle,
    hub: Arc<Hub>,
    identity: Identity,
    sessions: Sessions,
    listening: Mutex<Option<Listening>>,
    listen_error: Mutex<Option<String>>,
    /// Listening on Bluetooth too, while listening at all and the PC has a
    /// radio that's on.
    bluetooth: Mutex<Option<Arc<bluetooth::Listener>>>,
    online: Mutex<HashMap<String, Via>>,
    wake: Arc<Notify>,
    clips_dir: PathBuf,
}

fn pc_name(settings: &AppSettings) -> String {
    settings
        .sync
        .pc_name
        .clone()
        .filter(|n| !n.trim().is_empty())
        .or_else(|| std::env::var("COMPUTERNAME").ok())
        .unwrap_or_else(|| "This PC".into())
}

fn config(app: &AppHandle) -> Config {
    let settings = get_settings(app);
    Config {
        name: pc_name(&settings),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        kinds: settings.sync.kinds.clone(),
        clipboard: settings.sync.clipboard,
    }
}

fn load_identity() -> Result<Identity> {
    if let Some(saved) = crate::secrets::read_secret(IDENTITY_SECRET).map_err(|e| anyhow!(e))? {
        if let Ok(identity) = serde_json::from_str(&saved) {
            return Ok(identity);
        }
        log::warn!("The saved sync identity is unreadable; making a new one");
    }
    let identity = Identity::generate()?;
    crate::secrets::write_secret(IDENTITY_SECRET, &serde_json::to_string(&identity)?)
        .map_err(|e| anyhow!(e))?;
    Ok(identity)
}

/// Sets up phone sync once the history database is migrated. Listens only
/// when a phone is paired.
pub fn init(app: &AppHandle) {
    let result = (|| -> Result<Arc<SyncManager>> {
        let data = crate::portable::app_data_dir(app)?;
        let identity = load_identity()?;
        let engine = Engine {
            db: data.join("history.db"),
            me: identity.device_id.clone(),
            extra: Some(Box::new(SettingsKinds(app.clone()))),
        };
        let hub = Arc::new(Hub::new(
            engine,
            config(app),
            Box::new(AppEvents(app.clone())),
        ));
        Ok(Arc::new(SyncManager {
            app: app.clone(),
            hub,
            identity,
            sessions: Sessions::default(),
            listening: Mutex::new(None),
            listen_error: Mutex::new(None),
            bluetooth: Mutex::new(None),
            online: Mutex::new(HashMap::new()),
            wake: Arc::new(Notify::new()),
            clips_dir: data.join("clips"),
        }))
    })();
    let sync = match result {
        Ok(sync) => sync,
        Err(e) => {
            log::error!("Phone sync is off: {e:#}");
            return;
        }
    };
    app.manage(sync.clone());
    if sync.hub.peers().is_ok_and(|p| !p.is_empty()) {
        sync.listen();
    }
    // What the app reports changing is checked soon after, not only every
    // REFRESH_EVERY.
    for event in [
        "dictionary-changed",
        "snippets-changed",
        "scratchpad-changed",
        "notes-changed",
        "history-update-payload",
        "settings-changed",
    ] {
        let wake = sync.wake.clone();
        app.listen_any(event, move |_| wake.notify_one());
    }
    let looping = sync.clone();
    tauri::async_runtime::spawn(async move { looping.refresh_loop().await });
    clipboard::watch(app.clone());
}

impl SyncManager {
    fn changed(&self) {
        let _ = SyncChanged.emit(&self.app);
    }

    /// Starts listening for phones, if it isn't already.
    fn listen(self: &Arc<Self>) {
        if self
            .listening
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_some()
        {
            return;
        }
        let this = self.clone();
        tauri::async_runtime::spawn(async move {
            let listener = match link::bind(protocol::PORT).await {
                Ok(listener) => listener,
                Err(e) => {
                    log::error!("Phone sync can't listen: {e:#}");
                    *this.listen_error.lock().unwrap_or_else(|e| e.into_inner()) =
                        Some(e.to_string());
                    this.changed();
                    return;
                }
            };
            let port = listener.local_addr().map_or(protocol::PORT, |a| a.port());
            let (stop, stopped) = watch::channel(false);
            {
                let mut slot = this.listening.lock().unwrap_or_else(|e| e.into_inner());
                if slot.is_some() {
                    return;
                }
                *slot = Some(Listening { port, stop });
            }
            *this.listen_error.lock().unwrap_or_else(|e| e.into_inner()) = None;
            log::info!("Phone sync listening on port {port}");
            this.listen_bluetooth();
            this.publish_addrs();
            this.changed();
            let host: Arc<dyn Host> = this.hub.clone();
            if let Err(e) = link::serve(
                listener,
                this.identity.clone(),
                host,
                this.sessions.clone(),
                stopped,
            )
            .await
            {
                log::error!("Phone sync stopped listening: {e:#}");
            }
        });
    }

    /// Stops listening when no phone is paired and pairing isn't open.
    fn listen_if_needed(self: &Arc<Self>) {
        let needed = self.hub.pairing_open() || self.hub.peers().is_ok_and(|p| !p.is_empty());
        if needed {
            self.listen();
        } else if let Some(listening) = self
            .listening
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
        {
            let _ = listening.stop.send(true);
            if let Some(radio) = self
                .bluetooth
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .take()
            {
                radio.close();
            }
            self.sessions.close_all();
            self.hub.set_addrs(Vec::new());
        }
    }

    /// Starts listening on Bluetooth as well, when listening and not on it
    /// yet. Without a radio, or with it off, it tries again later.
    fn listen_bluetooth(self: &Arc<Self>) {
        let stop = {
            let listening = self.listening.lock().unwrap_or_else(|e| e.into_inner());
            match listening.as_ref() {
                Some(l) => l.stop.subscribe(),
                None => return,
            }
        };
        let mut slot = self.bluetooth.lock().unwrap_or_else(|e| e.into_inner());
        if slot.is_some() {
            return;
        }
        let radio = match bluetooth::Listener::open() {
            Ok(radio) => Arc::new(radio),
            Err(e) => {
                log::debug!("Phone sync isn't on Bluetooth: {e:#}");
                return;
            }
        };
        log::info!("Phone sync listening on Bluetooth");
        *slot = Some(radio.clone());
        drop(slot);
        self.publish_addrs();
        let this = self.clone();
        std::thread::spawn(move || loop {
            let conn = match radio.accept() {
                Ok(conn) => conn,
                Err(e) => {
                    // Closed on purpose, or the radio went off: drop it and
                    // let the refresh loop try again.
                    if !radio.is_closed() {
                        log::info!("Phone sync stopped on Bluetooth: {e:#}");
                        radio.close();
                    }
                    let mut slot = this.bluetooth.lock().unwrap_or_else(|e| e.into_inner());
                    if slot.as_ref().is_some_and(|r| Arc::ptr_eq(r, &radio)) {
                        *slot = None;
                    }
                    drop(slot);
                    this.publish_addrs();
                    this.changed();
                    return;
                }
            };
            // Counted before the bridge's two threads start, so a flood of
            // Bluetooth connections can't pile them up.
            let Some(pending) = link::Pending::start() else {
                log::debug!("Phone sync: dropped a Bluetooth connection, too many handshakes");
                drop(conn);
                continue;
            };
            let stream = bluetooth::bridge(conn);
            let (identity, host, sessions, stop): (_, Arc<dyn Host>, _, _) = (
                this.identity.clone(),
                this.hub.clone(),
                this.sessions.clone(),
                stop.clone(),
            );
            tauri::async_runtime::spawn(async move {
                if let Err(e) = link::serve_stream(
                    stream,
                    Via::Bluetooth,
                    &identity,
                    host,
                    sessions,
                    stop,
                    pending,
                )
                .await
                {
                    log::info!("Phone connection over Bluetooth ended: {e:#}");
                }
            });
        });
    }

    fn port(&self) -> Option<u16> {
        self.listening
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .map(|l| l.port)
    }

    /// Tells phones, in each `welcome`, where this PC is now.
    fn publish_addrs(&self) {
        self.hub
            .set_addrs(self.addresses().into_iter().map(|a| a.addr).collect());
    }

    fn addresses(&self) -> Vec<SyncAddress> {
        let port = self.port().unwrap_or(protocol::PORT);
        let mut found: Vec<SyncAddress> = net::addresses()
            .into_iter()
            .map(|(kind, ip)| SyncAddress {
                kind: kind.into(),
                addr: format!("{ip}:{port}"),
            })
            .collect();
        if let Some(radio) = self
            .bluetooth
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
        {
            found.push(SyncAddress {
                kind: "bluetooth".into(),
                addr: format!("{}{}", protocol::BT_PREFIX, radio.address),
            });
        }
        found
    }

    pub(crate) fn any_online(&self) -> bool {
        !self
            .online
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_empty()
    }

    async fn refresh_loop(self: Arc<Self>) {
        loop {
            tokio::select! {
                _ = tokio::time::sleep(REFRESH_EVERY) => {}
                _ = self.wake.notified() => {
                    // Let a burst of changes settle.
                    tokio::time::sleep(Duration::from_millis(700)).await;
                }
            }
            // Bluetooth turned on since, or the radio came back.
            let off_bluetooth = self.port().is_some()
                && self
                    .bluetooth
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .is_none();
            if off_bluetooth {
                self.listen_bluetooth();
                if self
                    .bluetooth
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .is_some()
                {
                    self.changed();
                }
            }
            if !self.any_online() {
                continue;
            }
            let hub = self.hub.clone();
            let kinds = hub.config().kinds;
            match tauri::async_runtime::spawn_blocking(move || hub.engine.refresh(&kinds)).await {
                Ok(Ok(0)) => {}
                Ok(Ok(changed)) => {
                    log::debug!("{changed} changes for the phone");
                    self.sessions.broadcast(Command::Nudge);
                }
                Ok(Err(e)) => log::warn!("Couldn't look for changes to sync: {e:#}"),
                Err(e) => log::warn!("Sync refresh stopped: {e}"),
            }
        }
    }

    pub fn state(&self) -> Result<SyncState> {
        let settings = get_settings(&self.app);
        let conn = self.hub.engine.conn()?;
        let online = self
            .online
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        let phones = self
            .hub
            .peers()?
            .into_iter()
            .map(|peer| {
                let via = online.get(&peer.device_id);
                PhoneState {
                    online: via.is_some(),
                    via: via.map(|via| match via {
                        Via::Bluetooth => "bluetooth".into(),
                        Via::Net(addr) => match addr.ip() {
                            std::net::IpAddr::V4(ip) => {
                                hub::address_kind(ip).unwrap_or("lan").to_string()
                            }
                            std::net::IpAddr::V6(_) => "lan".into(),
                        },
                    }),
                    peer,
                }
            })
            .collect();
        let mut counts = Vec::new();
        for kind in protocol::KINDS {
            let count = match kind {
                "transforms" => settings.transforms.len() as i64,
                "style" => STYLE_KINDS.len() as i64,
                _ => kinds::count(&conn, kind)?,
            };
            counts.push(KindCount {
                kind: kind.into(),
                count: count as u32,
            });
        }
        Ok(SyncState {
            pc_name: pc_name(&settings),
            addresses: self.addresses(),
            listening: self.port().is_some(),
            listen_error: self
                .listen_error
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone(),
            pairing: self.hub.pairing_open(),
            phones,
            counts,
            merges: ledger::merges(&conn)?.len() as u32,
        })
    }

    pub fn start_pairing(self: &Arc<Self>) -> Result<PairingInfo> {
        let secret = self.hub.open_pairing();
        self.listen();
        let settings = get_settings(&self.app);
        let addresses = self.addresses();
        let link = PairLink {
            device_id: self.identity.device_id.clone(),
            name: pc_name(&settings),
            fingerprint: self.identity.fingerprint(),
            addrs: addresses.iter().map(|a| a.addr.clone()).collect(),
            secret,
        }
        .to_uri();
        let code =
            qrcode::QrCode::with_error_correction_level(link.as_bytes(), qrcode::EcLevel::M)?;
        let qr_svg = code
            .render::<qrcode::render::svg::Color>()
            .min_dimensions(232, 232)
            .quiet_zone(true)
            .dark_color(qrcode::render::svg::Color("#000000"))
            .light_color(qrcode::render::svg::Color("#ffffff"))
            .build();
        self.changed();
        Ok(PairingInfo {
            link,
            qr_svg,
            minutes: (hub::PAIRING_FOR.as_secs() / 60) as u32,
            pc_name: pc_name(&settings),
            addresses,
        })
    }

    pub fn cancel_pairing(self: &Arc<Self>) {
        self.hub.close_pairing();
        self.listen_if_needed();
        self.changed();
    }

    pub fn unpair(self: &Arc<Self>, device_id: &str) -> Result<()> {
        self.sessions
            .send(device_id, Command::Bye("unpaired".into()));
        // Even if the goodbye didn't fit in its queue, the session ends.
        self.sessions.close(device_id);
        ledger::remove_peer(&self.hub.engine.conn()?, device_id)?;
        self.online
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(device_id);
        self.listen_if_needed();
        self.changed();
        crate::tray::update_tray_menu(&self.app);
        Ok(())
    }

    pub fn rename(&self, device_id: &str, name: &str) -> Result<()> {
        let name: String = name.trim().chars().take(40).collect();
        if name.is_empty() {
            return Err(anyhow!("A name can't be empty"));
        }
        ledger::rename_peer(&self.hub.engine.conn()?, device_id, &name)?;
        self.changed();
        crate::tray::update_tray_menu(&self.app);
        Ok(())
    }

    /// Sends this PC's changes and asks for the phone's. False when the
    /// phone isn't connected.
    pub fn sync_now(&self, device_id: &str) -> Result<bool> {
        if !self.sessions.connected(device_id) {
            return Ok(false);
        }
        self.hub.engine.refresh(&self.hub.config().kinds)?;
        self.sessions.send(device_id, Command::Nudge);
        self.sessions.send(device_id, Command::Pull);
        Ok(true)
    }

    pub fn merges(&self) -> Result<Vec<Merge>> {
        ledger::merges(&self.hub.engine.conn()?)
    }

    /// Takes new sync settings: kinds switched on start over, and the phone
    /// hears what this PC syncs now.
    pub fn settings_changed(&self, before: &AppSettings, after: &AppSettings) {
        for kind in &after.sync.kinds {
            if !before.sync.kinds.contains(kind) {
                if let Err(e) = self.hub.engine.restart_kind(kind) {
                    log::warn!("Couldn't start syncing {kind}: {e:#}");
                }
            }
        }
        let config = config(&self.app);
        if config != self.hub.config() {
            self.hub.set_config(config);
            self.sessions
                .broadcast(Command::Welcome(self.hub.describe()));
            self.sessions.broadcast(Command::Pull);
        }
        self.changed();
        crate::tray::update_tray_menu(&self.app);
    }

    /// Whether copies are shared right now.
    pub fn clipboard_live(&self) -> bool {
        let settings = get_settings(&self.app).sync;
        settings.clipboard && !settings.clipboard_paused(now_s()) && self.any_online()
    }

    /// The first paired phone's name, for the tray.
    pub fn phone_name(&self) -> Option<String> {
        self.hub.peers().ok()?.into_iter().next().map(|p| p.name)
    }

    /// Sends a clip to every connected phone and keeps it in Recent clips.
    pub fn send_clip(&self, clip: Clip) {
        let peers = self.hub.peers().unwrap_or_default();
        let online = self
            .online
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        let mut sent_to = Vec::new();
        for peer in peers.iter().filter(|p| online.contains_key(&p.device_id)) {
            if self
                .sessions
                .send(&peer.device_id, Command::Clip(clip.clone()))
            {
                sent_to.push(peer.name.clone());
            }
        }
        if !sent_to.is_empty() {
            self.keep_clip(&clip, &sent_to.join(", "), false, false);
        }
    }

    /// Sends what's on the clipboard now, from the tray or Settings.
    /// Returns the phone's name.
    pub fn send_clipboard_now(&self) -> Result<String> {
        if !self.any_online() {
            return Err(anyhow!(
                "{} isn't connected",
                self.phone_name().unwrap_or_else(|| "The phone".into())
            ));
        }
        let clip = clipboard::read(&self.app, std::time::Instant::now())?
            .ok_or_else(|| anyhow!("Nothing on the clipboard can be shared"))?;
        self.send_clip(clip);
        Ok(self.phone_name().unwrap_or_default())
    }

    fn keep_clip(&self, clip: &Clip, peer: &str, incoming: bool, late: bool) {
        let (image, bytes) = match (&clip.kind, &clip.png) {
            (ClipKind::Image, Some(png)) => {
                // The id comes from the other device: as a file name only
                // when it's a plain id, so it can't point outside the folder.
                let id = if safe_clip_name(&clip.id) {
                    clip.id.clone()
                } else {
                    identity::uuid()
                };
                let file = format!("{id}.png");
                let decoded = STANDARD.decode(png).unwrap_or_default();
                let saved = std::fs::create_dir_all(&self.clips_dir)
                    .and_then(|_| std::fs::write(self.clips_dir.join(&file), &decoded));
                if let Err(e) = saved {
                    log::warn!("Couldn't keep a clip's image: {e}");
                }
                (Some(file), decoded.len() as u64)
            }
            _ => (None, clip.text.as_ref().map_or(0, |t| t.len() as u64)),
        };
        let row = ClipRow {
            id: clip.id.clone(),
            at: now_s(),
            peer: peer.to_string(),
            incoming,
            kind: match clip.kind {
                ClipKind::Text => "text".into(),
                ClipKind::Image => "image".into(),
            },
            text: clip.text.clone(),
            image,
            bytes,
            late,
            from_app: clip.from_app.clone(),
        };
        match self
            .hub
            .engine
            .conn()
            .and_then(|conn| ledger::add_clip(&conn, &row))
        {
            Ok(dropped) => {
                for file in dropped {
                    if let Some(path) = self.clip_path(&file) {
                        let _ = std::fs::remove_file(path);
                    }
                }
            }
            Err(e) => log::warn!("Couldn't keep a clip: {e:#}"),
        }
        self.changed();
    }

    /// A clip's image file in the clips folder; None for a name that could
    /// reach outside it.
    fn clip_path(&self, file: &str) -> Option<PathBuf> {
        let id = file.strip_suffix(".png")?;
        safe_clip_name(id).then(|| self.clips_dir.join(file))
    }

    /// A clip from the phone: onto the clipboard, unless it came too late.
    fn take_clip(&self, peer: &Peer, clip: Clip) {
        let settings = get_settings(&self.app).sync;
        if !settings.clipboard || settings.clipboard_paused(now_s()) {
            return;
        }
        if clip.kind == ClipKind::Image {
            // Base64 is a third larger than the image it carries.
            let bytes = clip.png.as_ref().map_or(0, |p| p.len() as u64 / 4 * 3);
            if bytes > settings.clipboard_images_mb as u64 * 1024 * 1024 {
                return;
            }
        }
        let late = clip.age_ms > clipboard::LATE_MS;
        if !late {
            if let Err(e) = clipboard::write(&self.app, &clip) {
                log::warn!("Couldn't put the phone's clip on the clipboard: {e:#}");
            } else if settings.phone_pill {
                crate::overlay::show_overlay_notice(
                    &self.app,
                    &crate::transforms::BarNotice::FromPhone {
                        name: peer.name.clone(),
                        image: clip.kind == ClipKind::Image,
                    },
                );
            }
        }
        self.keep_clip(&clip, &peer.name, true, late);
    }

    pub fn clips(&self) -> Result<Vec<ClipRow>> {
        ledger::clips(&self.hub.engine.conn()?)
    }

    pub fn clear_clips(&self) -> Result<()> {
        for file in ledger::clear_clips(&self.hub.engine.conn()?)? {
            if let Some(path) = self.clip_path(&file) {
                let _ = std::fs::remove_file(path);
            }
        }
        self.changed();
        Ok(())
    }

    /// Puts a recent clip back on the clipboard.
    pub fn copy_clip(&self, id: &str) -> Result<()> {
        let row = self
            .clips()?
            .into_iter()
            .find(|c| c.id == id)
            .ok_or_else(|| anyhow!("That clip is gone"))?;
        let clip = Clip {
            id: row.id,
            kind: if row.kind == "image" {
                ClipKind::Image
            } else {
                ClipKind::Text
            },
            text: row.text,
            png: match row.image {
                Some(file) => {
                    let path = self
                        .clip_path(&file)
                        .ok_or_else(|| anyhow!("bad clip file"))?;
                    Some(STANDARD.encode(std::fs::read(path)?))
                }
                None => None,
            },
            age_ms: 0,
            from_app: None,
        };
        clipboard::write(&self.app, &clip)
    }

    /// A recent clip's image as a data URL, for its thumbnail.
    pub fn clip_image(&self, id: &str) -> Result<String> {
        let file = self
            .clips()?
            .into_iter()
            .find(|c| c.id == id)
            .and_then(|c| c.image)
            .ok_or_else(|| anyhow!("That clip has no image"))?;
        let png = std::fs::read(
            self.clip_path(&file)
                .ok_or_else(|| anyhow!("bad clip file"))?,
        )?;
        Ok(format!("data:image/png;base64,{}", STANDARD.encode(png)))
    }

    /// The app reloads what a sync changed.
    fn reload(&self, accepted: &Accepted) {
        let app = &self.app;
        for kind in &accepted.kinds {
            match kind.as_str() {
                "dictionary" => {
                    if let Some(m) =
                        app.try_state::<Arc<crate::managers::dictionary::DictionaryManager>>()
                    {
                        m.changed();
                    }
                }
                "snippets" => {
                    if let Some(m) =
                        app.try_state::<Arc<crate::managers::snippets::SnippetManager>>()
                    {
                        m.changed();
                    }
                }
                "scratchpad" => {
                    if let Some(m) = app.try_state::<Arc<crate::scratchpad::ScratchpadManager>>() {
                        m.changed();
                    }
                }
                "notes" => {
                    if let Some(m) = app.try_state::<Arc<crate::notes::NotesManager>>() {
                        m.changed();
                    }
                }
                "history" => {
                    let _ = HistoryUpdatePayload::Synced.emit(app);
                }
                _ => {}
            }
        }
        for (kind, file) in &accepted.removed_audio {
            let path = match kind.as_str() {
                "history" => app
                    .try_state::<Arc<HistoryManager>>()
                    .map(|h| h.get_audio_file_path(file)),
                "notes" => app
                    .try_state::<Arc<crate::notes::NotesManager>>()
                    .map(|n| n.audio_dir().join(file)),
                _ => None,
            };
            if let Some(path) = path {
                let _ = std::fs::remove_file(path);
            }
        }
    }
}

/// What the hub reports, turned into app events.
struct AppEvents(AppHandle);

impl AppEvents {
    fn sync(&self) -> Option<Arc<SyncManager>> {
        self.0
            .try_state::<Arc<SyncManager>>()
            .map(|s| s.inner().clone())
    }
}

impl hub::Events for AppEvents {
    fn paired(&self, peer: &Peer) {
        log::info!("Paired with {} ({})", peer.name, peer.platform);
        let _ = SyncChanged.emit(&self.0);
        crate::tray::update_tray_menu(&self.0);
    }

    fn online(&self, device_id: &str, via: Option<Via>) {
        if let Some(sync) = self.sync() {
            let mut online = sync.online.lock().unwrap_or_else(|e| e.into_inner());
            match via {
                Some(via) => {
                    online.insert(device_id.to_string(), via);
                }
                // An old session ending after the phone already came back
                // mustn't mark the new one offline.
                None if sync.sessions.connected(device_id) => {}
                None => {
                    online.remove(device_id);
                }
            }
        }
        let _ = SyncChanged.emit(&self.0);
    }

    fn synced(&self, _device_id: &str, accepted: &Accepted) {
        if let Some(sync) = self.sync() {
            sync.reload(accepted);
        }
        let _ = SyncChanged.emit(&self.0);
    }

    fn clip(&self, peer: &Peer, clip: Clip) {
        if let Some(sync) = self.sync() {
            // The clipboard can block for a moment, so clips go to one
            // thread of their own, a few at a time; a burst beyond that is
            // dropped rather than piling up threads and memory.
            static QUEUE: std::sync::OnceLock<
                std::sync::mpsc::SyncSender<(Arc<SyncManager>, Peer, Clip)>,
            > = std::sync::OnceLock::new();
            let queue = QUEUE.get_or_init(|| {
                let (tx, rx) = std::sync::mpsc::sync_channel::<(Arc<SyncManager>, Peer, Clip)>(4);
                std::thread::spawn(move || {
                    for (sync, peer, clip) in rx {
                        sync.take_clip(&peer, clip);
                    }
                });
                tx
            });
            if queue.try_send((sync, peer.clone(), clip)).is_err() {
                log::debug!("Phone sync: dropped a clip, too many waiting");
            }
        }
    }

    fn forgotten(&self, device_id: &str) {
        log::info!("{device_id} unpaired this PC");
        if let Some(sync) = self.sync() {
            sync.online
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .remove(device_id);
            sync.listen_if_needed();
        }
        let _ = SyncChanged.emit(&self.0);
        crate::tray::update_tray_menu(&self.0);
    }

    fn transcribe(
        &self,
        device_id: &str,
        samples: Vec<f32>,
        app: Option<String>,
    ) -> Option<link::TranscribeJob> {
        let handle = self.0.clone();
        let device = device_id.to_string();
        Some(Box::new(move || {
            use crate::managers::transcription::TranscriptionManager;
            let tm = handle
                .try_state::<Arc<TranscriptionManager>>()
                .map(|s| s.inner().clone())
                .ok_or("Legato on the PC isn't ready yet")?;
            let seconds = samples.len() as f32 / 16_000.0;
            let started = std::time::Instant::now();
            crate::notes::ensure_model(&tm).map_err(|e| e.to_string())?;
            // An empty recording is the phone saying it started listening:
            // the model is loaded now, so the real one doesn't wait for it.
            if samples.is_empty() {
                return Ok(String::new());
            }
            let heard = tm.transcribe(samples).map_err(|e| e.to_string())?;
            if heard.text.trim().is_empty() {
                return Ok(String::new());
            }
            // The same Dictionary, snippets, cleanup and Style as a
            // dictation on the PC, in the kind of app the phone typed into.
            let target = phone_app(app.as_deref());
            let processed =
                tauri::async_runtime::block_on(crate::actions::process_transcription_output(
                    &handle,
                    &heard.text,
                    crate::actions::DictationMode::Plain,
                    target.as_ref(),
                    &crate::actions::no_stage,
                ));
            log::info!(
                "Transcribed {seconds:.1} s from {device} in {:.1} s",
                started.elapsed().as_secs_f32()
            );
            Ok(processed.final_text)
        }))
    }

    fn pairing_failed(&self, reason: Refusal, closed: bool) {
        log::warn!(
            "A phone failed to pair ({reason:?}){}",
            if closed { "; pairing closed" } else { "" }
        );
        if closed {
            if let Some(sync) = self.sync() {
                sync.listen_if_needed();
            }
        }
        let _ = SyncChanged.emit(&self.0);
    }
}

/// The app a phone typed into, as the pipeline styles for it: only its kind
/// is known. None for no kind or one this version doesn't know ("other").
fn phone_app(kind: Option<&str>) -> Option<crate::context::AppInfo> {
    let category = serde_json::from_value(json!(kind?)).ok()?;
    Some(crate::context::AppInfo {
        exe_path: String::new(),
        exe: String::new(),
        name: "Phone".into(),
        title: String::new(),
        site: None,
        category,
    })
}

/// Transforms and Style, which live in the settings.
struct SettingsKinds(AppHandle);

impl Extra for SettingsKinds {
    fn handles(&self, kind: &str) -> bool {
        kind == "transforms" || kind == "style"
    }

    fn export(&self, kind: &str) -> Result<Vec<Item>> {
        let settings = get_settings(&self.0);
        Ok(match kind {
            "transforms" => settings
                .transforms
                .iter()
                .enumerate()
                .map(|(position, t)| Item {
                    key: t.id.clone(),
                    body: json!({
                        "name": t.name, "description": t.description, "prompt": t.prompt,
                        "enabled": t.enabled, "built_in": t.built_in, "output": t.output,
                        "show_changes": t.show_changes, "position": position,
                    }),
                    at: None,
                })
                .collect(),
            _ => STYLE_KINDS
                .iter()
                .map(|category| Item {
                    key: category.to_string(),
                    body: json!({ "tone": settings.styles.get(category).tone }),
                    at: None,
                })
                .collect(),
        })
    }

    fn apply(&self, kind: &str, key: &str, body: Option<&Value>) -> Result<()> {
        let before = get_settings(&self.0);
        let mut after = before.clone();
        if kind == "style" {
            let Some(body) = body else {
                return Ok(());
            };
            let mut style = after.styles.get(key);
            style.tone = serde_json::from_value(body["tone"].clone())?;
            if after.styles.set(key, style) {
                write_settings(&self.0, after);
                let _ = self
                    .0
                    .emit("settings-changed", json!({ "setting": "styles" }));
            }
            return Ok(());
        }
        let at = after.transforms.iter().position(|t| t.id == key);
        match (body, at) {
            (None, Some(i)) if !after.transforms[i].built_in => {
                after.transforms.remove(i);
                after
                    .bindings
                    .remove(&crate::transform_rules::binding_id(key));
            }
            (None, _) => return Ok(()),
            (Some(body), at) => {
                let mut transform = match at {
                    Some(i) => after.transforms.remove(i),
                    None => serde_json::from_value(json!({
                        "id": key, "name": "", "prompt": "",
                    }))?,
                };
                let text = |f: &str| body[f].as_str().unwrap_or_default().to_string();
                transform.name = text("name");
                transform.description = text("description");
                transform.prompt = text("prompt");
                transform.enabled = body["enabled"].as_bool().unwrap_or(true);
                transform.show_changes = body["show_changes"].as_bool().unwrap_or(true);
                if let Ok(output) = serde_json::from_value(body["output"].clone()) {
                    transform.output = output;
                }
                let position = body["position"].as_u64().unwrap_or(u64::MAX) as usize;
                after
                    .transforms
                    .insert(position.min(after.transforms.len()), transform);
            }
        }
        crate::commands::transforms::commit(&self.0, &before, after);
        let _ = self
            .0
            .emit("settings-changed", json!({ "setting": "transforms" }));
        Ok(())
    }
}

/// Whether a clip id is safe as a file name: letters, digits and dashes
/// only, as the ids Legato makes are.
fn safe_clip_name(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}
