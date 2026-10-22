//! Working in apps run as administrator. Windows keeps a normal app from
//! typing into an elevated one and from hearing its keys, so with the
//! setting on, Legato runs elevated too: a scheduled task set to run with
//! the highest privileges starts it, which Windows asks about once, when
//! the task is made.
//!
//! The task skips that question every later time, so it must not become
//! a way for other programs to get administrator rights: it's offered
//! only when Legato is installed where a normal program can't change its
//! files (Program Files, as the .msi installs it), and Legato checks the
//! task still starts this very exe before handing over to it.

use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;

use tauri::AppHandle;

const TASK: &str = "Legato\\Run as administrator";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
/// Left by the instance that hands over, when it would have started hidden.
const HIDDEN_MARKER: &str = "start-hidden-after-elevation";

/// Whether this process runs elevated.
pub fn is_elevated() -> bool {
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::Security::{
        GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY,
    };
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
    // SAFETY: a query on this process's own token, closed before returning.
    unsafe {
        let mut token = HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
            return false;
        }
        let mut elevation = TOKEN_ELEVATION::default();
        let mut len = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut _),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut len,
        )
        .is_ok();
        let _ = CloseHandle(token);
        ok && elevation.TokenIsElevated != 0
    }
}

/// Whether this account can be an administrator after a Windows question
/// (an admin with UAC), or already is. A standard user would register the
/// task under the admin who typed the password, so it's not offered there.
fn can_elevate() -> bool {
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::Security::{
        GetTokenInformation, TokenElevationType, TokenElevationTypeLimited, TOKEN_ELEVATION_TYPE,
        TOKEN_QUERY,
    };
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
    if is_elevated() {
        return true;
    }
    // SAFETY: a query on this process's own token, closed before returning.
    unsafe {
        let mut token = HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
            return false;
        }
        let mut kind = TOKEN_ELEVATION_TYPE::default();
        let mut len = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevationType,
            Some(&mut kind as *mut _ as *mut _),
            std::mem::size_of::<TOKEN_ELEVATION_TYPE>() as u32,
            &mut len,
        )
        .is_ok();
        let _ = CloseHandle(token);
        ok && kind == TokenElevationTypeLimited
    }
}

/// The Program Files folders, from Windows rather than the environment.
fn program_files() -> Vec<PathBuf> {
    use windows::Win32::System::Com::CoTaskMemFree;
    use windows::Win32::UI::Shell::{
        FOLDERID_ProgramFiles, FOLDERID_ProgramFilesX86, SHGetKnownFolderPath, KF_FLAG_DEFAULT,
    };
    [FOLDERID_ProgramFiles, FOLDERID_ProgramFilesX86]
        .iter()
        .filter_map(|id| {
            // SAFETY: Windows allocates the path; it's copied, then freed.
            unsafe {
                let path = SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, None).ok()?;
                let text = path.to_string().ok();
                CoTaskMemFree(Some(path.0 as *const _));
                text.map(PathBuf::from)
            }
        })
        .collect()
}

/// Whether this exe sits under Program Files, where only an administrator
/// can replace it or plant a DLL beside it.
fn installed_for_all_users() -> bool {
    let Ok(exe) = exe().and_then(|e| e.canonicalize().map_err(|e| e.to_string())) else {
        return false;
    };
    program_files().iter().any(|dir| {
        dir.canonicalize()
            .is_ok_and(|dir| exe.starts_with(&dir) && exe != dir)
    })
}

/// Why the setting can't be turned on here, as a key under
/// `settings.advanced.adminApps`, or None when it can.
pub fn unavailable() -> Option<&'static str> {
    if !can_elevate() {
        Some("notAdmin")
    } else if !installed_for_all_users() {
        Some("notAllUsers")
    } else {
        None
    }
}

fn schtasks(args: &[&str]) -> bool {
    Command::new("schtasks.exe")
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .is_ok_and(|o| o.status.success())
}

pub fn task_exists() -> bool {
    schtasks(&["/Query", "/TN", TASK])
}

/// The exe the task starts, read back from Task Scheduler.
fn task_command() -> Option<String> {
    let out = Command::new("schtasks.exe")
        .args(["/Query", "/TN", TASK, "/XML"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()
        .filter(|o| o.status.success())?;
    // schtasks prints the XML in the console's code page or UTF-16.
    let text = if out.stdout.starts_with(&[0xFF, 0xFE]) {
        let units: Vec<u16> = out.stdout[2..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| u16::from_le_bytes(*c))
            .collect();
        String::from_utf16_lossy(&units)
    } else {
        String::from_utf8_lossy(&out.stdout).into_owned()
    };
    let start = text.find("<Command>")? + "<Command>".len();
    let end = start + text[start..].find("</Command>")?;
    Some(xml_unescape(text[start..end].trim().trim_matches('"')))
}

/// Whether the task starts this exe, and this exe can't be swapped by a
/// normal program; anything else is a task not to hand over to.
fn task_is_ours() -> bool {
    let (Some(command), Ok(exe)) = (task_command(), exe()) else {
        return false;
    };
    command.eq_ignore_ascii_case(&exe.to_string_lossy()) && installed_for_all_users()
}

fn exe() -> Result<PathBuf, String> {
    std::env::current_exe().map_err(|e| e.to_string())
}

fn xml_unescape(s: &str) -> String {
    s.replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

/// Runs `program` elevated and waits for it; Windows asks first. Err when
/// the question was declined or the program failed.
fn run_as_admin(program: &str, params: &str) -> Result<(), String> {
    use windows::core::{w, HSTRING};
    use windows::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
    use windows::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject, INFINITE};
    use windows::Win32::UI::Shell::{
        ShellExecuteExW, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW,
    };
    use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;
    let program = HSTRING::from(program);
    let params = HSTRING::from(params);
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC,
        lpVerb: w!("runas"),
        lpFile: windows::core::PCWSTR(program.as_ptr()),
        lpParameters: windows::core::PCWSTR(params.as_ptr()),
        nShow: SW_HIDE.0,
        ..Default::default()
    };
    // SAFETY: the strings outlive the call; the process handle it returns
    // is closed below.
    unsafe {
        if ShellExecuteExW(&mut info).is_err() {
            return Err("Windows didn't allow it.".into());
        }
        if info.hProcess.is_invalid() {
            return Err("Windows didn't allow it.".into());
        }
        let waited = WaitForSingleObject(info.hProcess, INFINITE);
        let mut code = 1u32;
        let _ = GetExitCodeProcess(info.hProcess, &mut code);
        let _ = CloseHandle(info.hProcess);
        if waited != WAIT_OBJECT_0 || code != 0 {
            return Err("Couldn't set up starting as administrator.".into());
        }
    }
    Ok(())
}

/// Makes the task, asking Windows first unless Legato already runs elevated.
/// The definition travels inside the command line, never through a file a
/// normal program could swap while Windows waits for the answer.
pub fn create_task() -> Result<(), String> {
    use base64::Engine;
    if let Some(reason) = unavailable() {
        return Err(reason.into());
    }
    let exe = exe()?.to_string_lossy().replace('\'', "''");
    let (path, name) = TASK.rsplit_once('\\').unwrap_or(("", TASK));
    // No trigger, started on demand, at normal priority (tasks default to
    // below normal) and without a time limit.
    let script = format!(
        "$ErrorActionPreference='Stop'\n\
         try {{\n\
         $a=New-ScheduledTaskAction -Execute '{exe}'\n\
         $p=New-ScheduledTaskPrincipal -UserId ([Security.Principal.WindowsIdentity]::GetCurrent().Name) -LogonType Interactive -RunLevel Highest\n\
         $s=New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -ExecutionTimeLimit 0 -Priority 4 -MultipleInstances IgnoreNew\n\
         Register-ScheduledTask -TaskPath '\\{path}\\' -TaskName '{name}' -Action $a -Principal $p -Settings $s -Description 'Starts Legato as administrator.' -Force | Out-Null\n\
         }} catch {{ exit 1 }}\n"
    );
    let encoded = base64::engine::general_purpose::STANDARD.encode(
        script
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<u8>>(),
    );
    let params =
        format!("-NoProfile -NonInteractive -WindowStyle Hidden -EncodedCommand {encoded}");
    if is_elevated() {
        let ok = Command::new("powershell.exe")
            .raw_arg(&params)
            .creation_flags(CREATE_NO_WINDOW)
            .status()
            .is_ok_and(|s| s.success());
        if ok {
            Ok(())
        } else {
            Err("Couldn't set up starting as administrator.".into())
        }
    } else {
        run_as_admin("powershell.exe", &params)
    }
}

pub fn delete_task() {
    if task_exists() && !schtasks(&["/Delete", "/TN", TASK, "/F"]) {
        log::warn!("Couldn't remove the run-as-administrator task");
    }
}

/// Starts the next Legato after this one has gone, so the single-instance
/// check lets it in: through the task (elevated), or through Explorer
/// (not elevated, even from an elevated Legato).
fn relaunch_after_exit(elevated: bool) -> Result<(), String> {
    let then = if elevated {
        format!("schtasks.exe /Run /TN '{TASK}'")
    } else {
        format!(
            "explorer.exe '{}'",
            exe()?.to_string_lossy().replace('\'', "''")
        )
    };
    // Waits for this Legato to be gone (however long saving takes), so the
    // next one isn't turned away as a second copy.
    let script = format!(
        "Wait-Process -Id {} -Timeout 30 -ErrorAction SilentlyContinue; {then}",
        std::process::id()
    );
    Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-WindowStyle",
            "Hidden",
            "-Command",
        ])
        .arg(script)
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

fn marker(app: &AppHandle) -> Option<PathBuf> {
    crate::portable::app_data_dir(app)
        .ok()
        .map(|d| d.join(HIDDEN_MARKER))
}

/// Saves the settings and sends for the next Legato; the caller quits.
pub fn restart(app: &AppHandle, elevated: bool, hidden: bool) -> Result<(), String> {
    if hidden {
        if let Some(path) = marker(app) {
            let _ = std::fs::write(path, b"");
        }
    }
    relaunch_after_exit(elevated)?;
    crate::settings::save_now(app);
    log::info!(
        "Restarting Legato {}",
        if elevated {
            "as administrator"
        } else {
            "without administrator rights"
        }
    );
    Ok(())
}

/// Whether the Legato that handed over would have started hidden; read once.
pub fn take_hidden_marker(app: &AppHandle) -> bool {
    marker(app).is_some_and(|p| std::fs::remove_file(p).is_ok())
}

/// At startup: with the setting on and not elevated, hand over to the
/// elevated Legato. True when this one is on its way out.
pub fn hand_over_at_startup(app: &AppHandle, hidden: bool) -> bool {
    let mut settings = crate::settings::get_settings(app);
    if !settings.admin_apps || is_elevated() {
        return false;
    }
    if !task_is_ours() {
        // Removed from outside, or it starts another exe (Legato moved, or
        // the task was changed): work as a normal app again.
        log::warn!("The run-as-administrator task doesn't start this Legato; not using it");
        settings.admin_apps = false;
        crate::settings::write_settings(app, settings);
        return false;
    }
    match restart(app, true, hidden) {
        Ok(()) => true,
        Err(e) => {
            log::warn!("Couldn't start Legato as administrator: {e}");
            false
        }
    }
}
