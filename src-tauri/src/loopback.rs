//! What the PC plays, for a meeting's "Them" track: one app's sound through
//! process loopback, or everything on the default output. Audio arrives as
//! 16 kHz mono on a thread of its own.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use specta::Type;

/// An app that has a sound session open, offered as the meeting's other
/// side.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct AudioApp {
    pub pid: u32,
    pub name: String,
    pub exe_path: String,
    /// Playing sound right now, not only holding a session.
    pub active: bool,
    /// A call app (Teams, Zoom, Slack, Discord, Webex), picked by default.
    pub call: bool,
}

/// What to record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// Everything the default output plays.
    Everything,
    /// One process and the processes it started.
    App(u32),
}

/// A running capture. Dropping it stops it.
pub struct Capture {
    /// What is recorded, by name: the app, or the output device.
    pub name: String,
    /// One app was asked for, but this Windows can't record one app, so
    /// everything is recorded.
    pub fallback: bool,
    #[cfg_attr(not(windows), allow(dead_code))]
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Drop for Capture {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

const CALL_APPS: &[&str] = &[
    "ms-teams.exe",
    "teams.exe",
    "zoom.exe",
    "slack.exe",
    "discord.exe",
    "webex.exe",
    "ciscocollabhost.exe",
    "skype.exe",
];

/// Whether an exe is one of the call apps a meeting records by default.
pub fn is_call_app(exe_path: &str) -> bool {
    let exe = exe_path
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or(exe_path)
        .to_ascii_lowercase();
    CALL_APPS.contains(&exe.as_str())
}

/// The default output device's name, as Windows shows it.
pub fn default_output_name() -> Option<String> {
    use rodio::cpal::traits::HostTrait;
    crate::audio_toolkit::get_cpal_host()
        .default_output_device()
        .and_then(|d| crate::audio_toolkit::audio::output_device_name(&d))
}

/// Apps with a sound session on the default output, call apps and apps
/// playing now first.
pub fn audio_apps() -> Vec<AudioApp> {
    #[cfg(windows)]
    {
        let mut apps = win::sessions().unwrap_or_else(|e| {
            log::warn!("Couldn't list the apps playing sound: {e}");
            Vec::new()
        });
        apps.sort_by(|a, b| {
            b.call
                .cmp(&a.call)
                .then(b.active.cmp(&a.active))
                .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        apps
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

/// Starts recording `source`; `on_audio` gets 16 kHz mono samples.
pub fn start(source: Source, on_audio: impl FnMut(&[f32]) + Send + 'static) -> Result<Capture> {
    #[cfg(windows)]
    {
        win::start(source, Box::new(on_audio))
    }
    #[cfg(not(windows))]
    {
        let _ = (source, on_audio);
        Err(anyhow::anyhow!("Recording system audio needs Windows"))
    }
}

/// Turns any interleaved capture format into 16 kHz mono, by averaging the
/// channels and interpolating between samples.
#[cfg_attr(not(windows), allow(dead_code))]
struct Converter {
    rate: u32,
    channels: usize,
    /// Position of the next output sample, in input samples.
    pos: f64,
    /// The last input sample of the previous block.
    last: f32,
}

#[cfg_attr(not(windows), allow(dead_code))]
impl Converter {
    fn new(rate: u32, channels: usize) -> Self {
        Self {
            rate,
            channels: channels.max(1),
            pos: 0.0,
            last: 0.0,
        }
    }

    fn convert(&mut self, interleaved: &[f32], out: &mut Vec<f32>) {
        let mono: Vec<f32> = interleaved
            .chunks(self.channels)
            .map(|c| c.iter().sum::<f32>() / c.len() as f32)
            .collect();
        if self.rate == 16_000 {
            out.extend_from_slice(&mono);
            return;
        }
        let step = self.rate as f64 / 16_000.0;
        // Index -1 is the previous block's last sample.
        let sample = |i: isize| -> f32 {
            if i < 0 {
                self.last
            } else {
                mono[i as usize]
            }
        };
        while self.pos < mono.len() as f64 - 1.0 {
            let i = self.pos.floor() as isize;
            let frac = (self.pos - i as f64) as f32;
            out.push(sample(i) * (1.0 - frac) + sample(i + 1) * frac);
            self.pos += step;
        }
        self.pos -= mono.len() as f64;
        if let Some(&last) = mono.last() {
            self.last = last;
        }
    }
}

#[cfg(windows)]
mod win {
    use super::{AudioApp, Capture, Converter, Source};
    use anyhow::{anyhow, Result};
    use log::{debug, info, warn};
    use std::mem::ManuallyDrop;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc::{sync_channel, SyncSender};
    use std::sync::Arc;
    use std::time::Duration;
    use windows::core::{implement, Interface, Ref, HRESULT};
    use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
    use windows::Win32::Media::Audio::{
        eConsole, eRender, ActivateAudioInterfaceAsync, AudioSessionStateActive,
        IActivateAudioInterfaceAsyncOperation, IActivateAudioInterfaceCompletionHandler,
        IActivateAudioInterfaceCompletionHandler_Impl, IAudioCaptureClient, IAudioClient,
        IAudioSessionControl2, IAudioSessionManager2, IMMDeviceEnumerator, MMDeviceEnumerator,
        AUDCLNT_BUFFERFLAGS_SILENT, AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM,
        AUDCLNT_STREAMFLAGS_EVENTCALLBACK, AUDCLNT_STREAMFLAGS_LOOPBACK,
        AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY, AUDIOCLIENT_ACTIVATION_PARAMS,
        AUDIOCLIENT_ACTIVATION_PARAMS_0, AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK,
        AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS, PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE,
        VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK, WAVEFORMATEX, WAVEFORMATEXTENSIBLE, WAVE_FORMAT_PCM,
    };
    use windows::Win32::System::Com::StructuredStorage::{
        PROPVARIANT, PROPVARIANT_0, PROPVARIANT_0_0, PROPVARIANT_0_0_0,
    };
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoTaskMemFree, BLOB, CLSCTX_ALL, COINIT_MULTITHREADED,
    };
    use windows::Win32::System::Threading::{CreateEventW, WaitForSingleObject};
    use windows::Win32::System::Variant::VT_BLOB;

    /// 200 ms, in the 100 ns units WASAPI counts in.
    const BUFFER: i64 = 2_000_000;
    const WAVE_FORMAT_IEEE_FLOAT: u16 = 3;
    const WAVE_FORMAT_EXTENSIBLE: u16 = 0xFFFE;

    type OnAudio = Box<dyn FnMut(&[f32]) + Send>;

    pub fn sessions() -> Result<Vec<AudioApp>> {
        // SAFETY: COM calls on this thread, which initialises COM first.
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
            let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;
            let manager: IAudioSessionManager2 = device.Activate(CLSCTX_ALL, None)?;
            let sessions = manager.GetSessionEnumerator()?;
            let own = std::process::id();
            let mut apps: Vec<AudioApp> = Vec::new();
            for i in 0..sessions.GetCount()? {
                let Ok(control) = sessions.GetSession(i) else {
                    continue;
                };
                let Ok(control) = control.cast::<IAudioSessionControl2>() else {
                    continue;
                };
                if control.IsSystemSoundsSession().0 == 0 {
                    continue;
                }
                let Ok(pid) = control.GetProcessId() else {
                    continue;
                };
                if pid == 0 || pid == own {
                    continue;
                }
                let Some(info) = crate::context::app_for_process(pid) else {
                    continue;
                };
                let active = control
                    .GetState()
                    .is_ok_and(|s| s == AudioSessionStateActive);
                if let Some(seen) = apps.iter_mut().find(|a| a.exe_path == info.exe_path) {
                    seen.active |= active;
                    continue;
                }
                apps.push(AudioApp {
                    pid,
                    call: super::is_call_app(&info.exe_path),
                    name: info.name,
                    exe_path: info.exe_path,
                    active,
                });
            }
            Ok(apps)
        }
    }

    pub fn start(source: Source, on_audio: OnAudio) -> Result<Capture> {
        let stop = Arc::new(AtomicBool::new(false));
        let (ready_tx, ready_rx) = sync_channel::<Result<bool, String>>(1);
        let stop_for_thread = Arc::clone(&stop);
        let thread = std::thread::Builder::new()
            .name("notes-loopback".into())
            .spawn(move || run(source, on_audio, stop_for_thread, ready_tx))?;
        let fallback = match ready_rx.recv_timeout(Duration::from_secs(10)) {
            Ok(Ok(fallback)) => fallback,
            Ok(Err(e)) => {
                let _ = thread.join();
                return Err(anyhow!(e));
            }
            Err(_) => {
                stop.store(true, Ordering::SeqCst);
                return Err(anyhow!("System audio didn't start in time"));
            }
        };
        let name = match source {
            Source::App(pid) if !fallback => crate::context::app_for_process(pid)
                .map(|a| a.name)
                .unwrap_or_default(),
            _ => super::default_output_name().unwrap_or_default(),
        };
        info!("Recording system audio from {name}");
        Ok(Capture {
            name,
            fallback,
            stop,
            thread: Some(thread),
        })
    }

    #[implement(IActivateAudioInterfaceCompletionHandler)]
    struct Activated(SyncSender<()>);

    impl IActivateAudioInterfaceCompletionHandler_Impl for Activated_Impl {
        fn ActivateCompleted(
            &self,
            _operation: Ref<'_, IActivateAudioInterfaceAsyncOperation>,
        ) -> windows::core::Result<()> {
            let _ = self.0.try_send(());
            Ok(())
        }
    }

    /// An audio client on one process tree's sound. Needs Windows 10 build
    /// 20348 or later.
    unsafe fn process_client(pid: u32) -> Result<IAudioClient> {
        // On the heap, so a late answer from Windows after the timeout below
        // still reads live memory: it's leaked then rather than freed.
        let mut params = Box::new(AUDIOCLIENT_ACTIVATION_PARAMS {
            ActivationType: AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK,
            Anonymous: AUDIOCLIENT_ACTIVATION_PARAMS_0 {
                ProcessLoopbackParams: AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS {
                    TargetProcessId: pid,
                    ProcessLoopbackMode: PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE,
                },
            },
        });
        // The blob borrows `params`. The windows crate's PROPVARIANT clears
        // itself on drop, and clearing a VT_BLOB frees its data with
        // CoTaskMemFree, which on memory Rust owns corrupts the heap, so the
        // variant must never be dropped.
        let variant = ManuallyDrop::new(PROPVARIANT {
            Anonymous: PROPVARIANT_0 {
                Anonymous: ManuallyDrop::new(PROPVARIANT_0_0 {
                    vt: VT_BLOB,
                    wReserved1: 0,
                    wReserved2: 0,
                    wReserved3: 0,
                    Anonymous: PROPVARIANT_0_0_0 {
                        blob: BLOB {
                            cbSize: std::mem::size_of::<AUDIOCLIENT_ACTIVATION_PARAMS>() as u32,
                            pBlobData: &mut *params as *mut _ as *mut u8,
                        },
                    },
                }),
            },
        });
        let (tx, rx) = sync_channel(1);
        let handler: IActivateAudioInterfaceCompletionHandler = Activated(tx).into();
        let operation = ActivateAudioInterfaceAsync(
            VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK,
            &IAudioClient::IID,
            Some(&*variant),
            &handler,
        )?;
        if rx.recv_timeout(Duration::from_secs(5)).is_err() {
            Box::leak(params);
            return Err(anyhow!(
                "Windows didn't answer the process loopback request"
            ));
        }
        let mut result = HRESULT(0);
        let mut activated = None;
        operation.GetActivateResult(&mut result, &mut activated)?;
        result.ok()?;
        Ok(activated
            .ok_or_else(|| anyhow!("Process loopback returned no client"))?
            .cast()?)
    }

    unsafe fn endpoint_client() -> Result<IAudioClient> {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;
        Ok(device.Activate(CLSCTX_ALL, None)?)
    }

    /// The stream's format, as the converter needs it.
    struct Format {
        rate: u32,
        channels: usize,
        float: bool,
        bits: u16,
    }

    /// Asks for 16 kHz mono 16-bit, which Windows converts to; an endpoint
    /// that refuses gets its own mix format, converted here.
    unsafe fn initialize(client: &IAudioClient, process: bool) -> Result<Format> {
        let flags = AUDCLNT_STREAMFLAGS_LOOPBACK
            | AUDCLNT_STREAMFLAGS_EVENTCALLBACK
            | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM
            | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY;
        let wanted = WAVEFORMATEX {
            wFormatTag: WAVE_FORMAT_PCM as u16,
            nChannels: 1,
            nSamplesPerSec: 16_000,
            nAvgBytesPerSec: 32_000,
            nBlockAlign: 2,
            wBitsPerSample: 16,
            cbSize: 0,
        };
        let first = client.Initialize(AUDCLNT_SHAREMODE_SHARED, flags, BUFFER, 0, &wanted, None);
        if first.is_ok() {
            return Ok(Format {
                rate: 16_000,
                channels: 1,
                float: false,
                bits: 16,
            });
        }
        if process {
            first?;
        }
        debug!("The output refused 16 kHz; using its own format");
        let mix = client.GetMixFormat()?;
        let format = &*mix;
        let tag = if format.wFormatTag == WAVE_FORMAT_EXTENSIBLE {
            let ext = &*(mix as *const WAVEFORMATEXTENSIBLE);
            // KSDATAFORMAT_SUBTYPE_IEEE_FLOAT's first field is 3.
            if ext.SubFormat.data1 == 3 {
                WAVE_FORMAT_IEEE_FLOAT
            } else {
                WAVE_FORMAT_PCM as u16
            }
        } else {
            format.wFormatTag
        };
        let result = Format {
            rate: format.nSamplesPerSec,
            channels: format.nChannels as usize,
            float: tag == WAVE_FORMAT_IEEE_FLOAT,
            bits: format.wBitsPerSample,
        };
        let init = client.Initialize(
            AUDCLNT_SHAREMODE_SHARED,
            AUDCLNT_STREAMFLAGS_LOOPBACK | AUDCLNT_STREAMFLAGS_EVENTCALLBACK,
            BUFFER,
            0,
            mix,
            None,
        );
        CoTaskMemFree(Some(mix as *const _));
        init?;
        if !result.float && result.bits != 16 {
            return Err(anyhow!(
                "The output's {}-bit format isn't supported",
                result.bits
            ));
        }
        Ok(result)
    }

    /// Opens the client for `source`. True when one app was asked for but
    /// everything is recorded instead.
    unsafe fn open(source: Source) -> Result<(IAudioClient, Format, bool)> {
        if let Source::App(pid) = source {
            match process_client(pid).and_then(|c| initialize(&c, true).map(|f| (c, f))) {
                Ok((client, format)) => return Ok((client, format, false)),
                Err(e) => {
                    warn!("Recording one app isn't possible here ({e}); recording everything")
                }
            }
        }
        let client = endpoint_client()?;
        let format = initialize(&client, false)?;
        Ok((client, format, matches!(source, Source::App(_))))
    }

    fn run(
        source: Source,
        mut on_audio: OnAudio,
        stop: Arc<AtomicBool>,
        ready: SyncSender<Result<bool, String>>,
    ) {
        // SAFETY: every COM object is created and used on this thread.
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let event: HANDLE = match CreateEventW(None, false, false, None) {
                Ok(e) => e,
                Err(e) => {
                    let _ = ready.send(Err(e.to_string()));
                    return;
                }
            };
            let mut announced = false;
            let mut source = source;
            // Reopened when the output device changes or goes away.
            while !stop.load(Ordering::SeqCst) {
                let opened = open(source).and_then(|(client, format, fallback)| {
                    client.SetEventHandle(event)?;
                    let capture: IAudioCaptureClient = client.GetService()?;
                    client.Start()?;
                    Ok((client, capture, format, fallback))
                });
                let (client, capture, format, fallback) = match opened {
                    Ok(o) => o,
                    Err(e) => {
                        if !announced {
                            let _ = ready.send(Err(e.to_string()));
                            break;
                        }
                        warn!("System audio stopped and couldn't restart: {e}");
                        std::thread::sleep(Duration::from_millis(500));
                        continue;
                    }
                };
                if !announced {
                    let _ = ready.send(Ok(fallback));
                    announced = true;
                }
                if fallback {
                    source = Source::Everything;
                }
                let mut converter = Converter::new(format.rate, format.channels);
                let mut samples: Vec<f32> = Vec::new();
                let mut out: Vec<f32> = Vec::new();
                let result: Result<()> = (|| {
                    while !stop.load(Ordering::SeqCst) {
                        if WaitForSingleObject(event, 100) != WAIT_OBJECT_0 {
                            continue;
                        }
                        loop {
                            let frames = capture.GetNextPacketSize()?;
                            if frames == 0 {
                                break;
                            }
                            let mut data = std::ptr::null_mut();
                            let mut frames = 0u32;
                            let mut flags = 0u32;
                            capture.GetBuffer(&mut data, &mut frames, &mut flags, None, None)?;
                            let count = frames as usize * format.channels;
                            samples.clear();
                            if flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 || data.is_null() {
                                samples.resize(count, 0.0);
                            } else if format.float {
                                samples.extend_from_slice(std::slice::from_raw_parts(
                                    data as *const f32,
                                    count,
                                ));
                            } else {
                                samples.extend(
                                    std::slice::from_raw_parts(data as *const i16, count)
                                        .iter()
                                        .map(|&s| s as f32 / 32_768.0),
                                );
                            }
                            capture.ReleaseBuffer(frames)?;
                            out.clear();
                            converter.convert(&samples, &mut out);
                            if !out.is_empty() {
                                on_audio(&out);
                            }
                        }
                    }
                    Ok(())
                })();
                let _ = client.Stop();
                if let Err(e) = result {
                    debug!("System audio stream ended ({e}); reopening");
                }
            }
            let _ = CloseHandle(event);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn call_apps_are_known_by_exe() {
        assert!(is_call_app(
            r"C:\Program Files\WindowsApps\MSTeams\ms-teams.exe"
        ));
        assert!(is_call_app(r"C:\Users\a\AppData\Roaming\Zoom\bin\Zoom.exe"));
        assert!(!is_call_app(
            r"C:\Program Files\Google\Chrome\Application\chrome.exe"
        ));
    }

    #[test]
    fn the_converter_makes_16_khz_mono() {
        let mut c = Converter::new(48_000, 2);
        let mut out = Vec::new();
        let block: Vec<f32> = (0..9_600)
            .map(|i| if i % 2 == 0 { 0.5 } else { -0.5 })
            .collect();
        for _ in 0..10 {
            c.convert(&block, &mut out);
        }
        // 10 blocks of 100 ms.
        assert!((15_990..=16_010).contains(&out.len()), "{}", out.len());
        assert!(out.iter().all(|s| s.abs() < 1e-6));
    }
}
