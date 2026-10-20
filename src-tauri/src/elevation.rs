//! Working in apps run as administrator. Windows keeps a normal app from
//! typing into an elevated one and from hearing its keys, so with the
//! setting on, Legato runs elevated too: a scheduled task set to run with
//! the highest privileges starts it, which Windows asks about once, when
//! the task is made.

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

fn exe() -> Result<PathBuf, String> {
    std::env::current_exe().map_err(|e| e.to_string())
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// The task, as Task Scheduler XML: no trigger, started on demand, at
/// normal priority (tasks default to below normal) and without a time limit.
fn task_xml(exe: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Description>Starts Legato as administrator so it can type into apps run as administrator.</Description>
  </RegistrationInfo>
  <Principals>
    <Principal id="Author">
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>HighestAvailable</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <AllowStartOnDemand>true</AllowStartOnDemand>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Priority>4</Priority>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>{}</Command>
    </Exec>
  </Actions>
</Task>
"#,
        xml_escape(exe)
    )
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
pub fn create_task() -> Result<(), String> {
    let exe = exe()?;
    let xml = task_xml(&exe.to_string_lossy());
    let path = std::env::temp_dir().join("legato-admin-task.xml");
    // Task Scheduler reads UTF-16 with a byte order mark.
    let mut bytes = vec![0xFF, 0xFE];
    bytes.extend(xml.encode_utf16().flat_map(u16::to_le_bytes));
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    let path_text = path.to_string_lossy().to_string();
    let result = if is_elevated() {
        if schtasks(&["/Create", "/TN", TASK, "/XML", &path_text, "/F"]) {
            Ok(())
        } else {
            Err("Couldn't set up starting as administrator.".into())
        }
    } else {
        run_as_admin(
            "schtasks.exe",
            &format!("/Create /TN \"{TASK}\" /XML \"{path_text}\" /F"),
        )
    };
    let _ = std::fs::remove_file(&path);
    result
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
        format!("schtasks /Run /TN \"{TASK}\"")
    } else {
        format!("explorer.exe \"{}\"", exe()?.to_string_lossy())
    };
    Command::new("cmd.exe")
        .raw_arg(format!("/c ping -n 3 127.0.0.1 >nul & {then}"))
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
    if !task_exists() {
        // Removed from outside; work as a normal app again.
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
