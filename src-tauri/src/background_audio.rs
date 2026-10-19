//! Other apps' sound while you dictate: lowered to a whisper, paused, or left
//! alone (muting everything stays in the audio manager). Call apps are never
//! touched, so a dictation during a meeting doesn't hide what people say.

use std::sync::Mutex;

use log::{debug, warn};

/// How loud other apps play while you dictate, as a share of their volume.
pub const LOWERED_TO: f32 = 0.05;

/// What was changed, so it can be put back.
enum Quieted {
    /// Audio sessions by instance ID, with the volume they had and the one
    /// Legato set.
    Lowered(Vec<(String, f32, f32)>),
    /// Players Legato paused through the Windows media controls.
    #[cfg(windows)]
    Paused(Vec<windows::Media::Control::GlobalSystemMediaTransportControlsSession>),
}

/// Held while sound is being lowered or put back, so a dictation that starts
/// during the ramp back up reads the real volumes.
static QUIETED: Mutex<Option<Quieted>> = Mutex::new(None);

fn lock() -> std::sync::MutexGuard<'static, Option<Quieted>> {
    QUIETED.lock().unwrap_or_else(|e| e.into_inner())
}

/// Lowers every other app that is playing to [`LOWERED_TO`] of its volume.
pub fn lower() {
    let mut quieted = lock();
    if quieted.is_some() {
        return;
    }
    #[cfg(windows)]
    match win::lower(LOWERED_TO) {
        Ok(lowered) if !lowered.is_empty() => {
            debug!("Lowered {} apps while dictating", lowered.len());
            *quieted = Some(Quieted::Lowered(lowered));
        }
        Ok(_) => {}
        Err(e) => warn!("Couldn't lower other apps: {e}"),
    }
}

/// Pauses players that show in the Windows media controls.
pub fn pause() {
    let mut quieted = lock();
    if quieted.is_some() {
        return;
    }
    #[cfg(windows)]
    match win::pause() {
        Ok(paused) if !paused.is_empty() => {
            debug!("Paused {} players while dictating", paused.len());
            *quieted = Some(Quieted::Paused(paused));
        }
        Ok(_) => {}
        Err(e) => warn!("Couldn't pause players: {e}"),
    }
}

/// Puts back what [`lower`] or [`pause`] changed. Volumes ramp up over a
/// moment rather than jumping; a volume someone changed meanwhile is left as
/// they set it.
pub fn restore() {
    let Some(quieted) = lock().take() else {
        return;
    };
    std::thread::spawn(move || {
        // Hold the lock for the ramp; see QUIETED.
        let _held = lock();
        match quieted {
            Quieted::Lowered(lowered) => {
                #[cfg(windows)]
                if let Err(e) = win::raise(&lowered) {
                    warn!("Couldn't put other apps' volume back: {e}");
                }
                #[cfg(not(windows))]
                let _ = lowered;
            }
            #[cfg(windows)]
            Quieted::Paused(paused) => win::resume(&paused),
        }
    });
}

/// The volume to step to on the way back up: `step` of `steps`.
pub fn ramp(from: f32, to: f32, step: u32, steps: u32) -> f32 {
    from + (to - from) * step as f32 / steps as f32
}

#[cfg(windows)]
mod win {
    use std::time::Duration;

    use windows::core::Interface;
    use windows::Media::Control::{
        GlobalSystemMediaTransportControlsSession as Session,
        GlobalSystemMediaTransportControlsSessionManager as Manager,
        GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
    };
    use windows::Win32::Media::Audio::{
        eConsole, eRender, AudioSessionStateActive, IAudioSessionControl2, IAudioSessionManager2,
        IMMDeviceEnumerator, ISimpleAudioVolume, MMDeviceEnumerator,
    };
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_ALL, COINIT_MULTITHREADED,
    };

    const STEPS: u32 = 6;
    const STEP: Duration = Duration::from_millis(50);

    /// Every audio session on the default output other than Legato's, the
    /// system sounds and call apps: its instance ID, whether it is playing,
    /// and its volume control.
    unsafe fn sessions() -> windows::core::Result<Vec<(String, bool, ISimpleAudioVolume)>> {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;
        let manager: IAudioSessionManager2 = device.Activate(CLSCTX_ALL, None)?;
        let list = manager.GetSessionEnumerator()?;
        let own = std::process::id();
        let mut out = Vec::new();
        for i in 0..list.GetCount()? {
            let Ok(control) = list.GetSession(i) else {
                continue;
            };
            let Ok(control) = control.cast::<IAudioSessionControl2>() else {
                continue;
            };
            // S_OK means this is the system sounds session.
            if control.IsSystemSoundsSession().0 == 0 {
                continue;
            }
            let Ok(pid) = control.GetProcessId() else {
                continue;
            };
            if pid == 0 || pid == own {
                continue;
            }
            if crate::context::app_for_process(pid)
                .is_some_and(|app| crate::loopback::is_call_app(&app.exe_path))
            {
                continue;
            }
            let Ok(id) = control.GetSessionInstanceIdentifier() else {
                continue;
            };
            let key = id.to_string().unwrap_or_default();
            CoTaskMemFree(Some(id.0 as *const _));
            let Ok(volume) = control.cast::<ISimpleAudioVolume>() else {
                continue;
            };
            let playing = control
                .GetState()
                .is_ok_and(|s| s == AudioSessionStateActive);
            out.push((key, playing, volume));
        }
        Ok(out)
    }

    pub fn lower(share: f32) -> windows::core::Result<Vec<(String, f32, f32)>> {
        let mut lowered = Vec::new();
        // SAFETY: COM calls on this thread, which `sessions` initialises.
        unsafe {
            for (id, playing, volume) in sessions()? {
                if !playing {
                    continue;
                }
                let Ok(before) = volume.GetMasterVolume() else {
                    continue;
                };
                let quiet = before * share;
                if volume.SetMasterVolume(quiet, std::ptr::null()).is_ok() {
                    lowered.push((id, before, quiet));
                }
            }
        }
        Ok(lowered)
    }

    pub fn raise(lowered: &[(String, f32, f32)]) -> windows::core::Result<()> {
        // SAFETY: as in `lower`.
        unsafe {
            let controls: Vec<(ISimpleAudioVolume, f32, f32)> = sessions()?
                .into_iter()
                .filter_map(|(id, _, volume)| {
                    let (_, before, quiet) = lowered.iter().find(|(key, ..)| *key == id)?;
                    // Someone changed it while dictating: theirs to keep.
                    let now = volume.GetMasterVolume().ok()?;
                    ((now - quiet).abs() < 0.01).then_some((volume, *before, *quiet))
                })
                .collect();
            for step in 1..=STEPS {
                for (volume, before, quiet) in &controls {
                    let level = super::ramp(*quiet, *before, step, STEPS);
                    let _ = volume.SetMasterVolume(level, std::ptr::null());
                }
                if step < STEPS {
                    std::thread::sleep(STEP);
                }
            }
        }
        Ok(())
    }

    pub fn pause() -> windows::core::Result<Vec<Session>> {
        let manager = Manager::RequestAsync()?.get()?;
        let mut paused = Vec::new();
        for session in manager.GetSessions()? {
            let playing = session
                .GetPlaybackInfo()
                .and_then(|info| info.PlaybackStatus())
                .is_ok_and(|status| status == Status::Playing);
            if playing && session.TryPauseAsync()?.get().unwrap_or(false) {
                paused.push(session);
            }
        }
        Ok(paused)
    }

    pub fn resume(paused: &[Session]) {
        for session in paused {
            // Paused by hand meanwhile would read as Paused too; there's no
            // telling the two apart, so play resumes either way.
            if let Err(e) = session.TryPlayAsync().and_then(|op| op.get()) {
                log::debug!("A player didn't resume: {e}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ramp_goes_from_quiet_back_to_the_volume_it_had() {
        assert_eq!(ramp(0.05, 1.0, 0, 6), 0.05);
        assert!((ramp(0.05, 1.0, 3, 6) - 0.525).abs() < 1e-6);
        assert_eq!(ramp(0.05, 1.0, 6, 6), 1.0);
    }
}
