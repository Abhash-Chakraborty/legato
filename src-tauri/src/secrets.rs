//! API keys for AI providers. They live in Windows Credential Manager, one
//! generic credential per provider ("Legato/anthropic"), and never in the
//! settings file, so a copied settings file or a bug report can't carry them.

use crate::settings::{self, AppSettings};
use log::{info, warn};
use std::collections::HashMap;
use tauri::AppHandle;

fn target(provider_id: &str) -> String {
    format!("Legato/{provider_id}")
}

#[cfg(windows)]
mod vault {
    use windows::core::{HSTRING, PWSTR};
    use windows::Win32::Foundation::ERROR_NOT_FOUND;
    use windows::Win32::Security::Credentials::{
        CredDeleteW, CredFree, CredReadW, CredWriteW, CREDENTIALW, CRED_PERSIST_LOCAL_MACHINE,
        CRED_TYPE_GENERIC,
    };

    fn not_found(error: &windows::core::Error) -> bool {
        error.code() == ERROR_NOT_FOUND.to_hresult()
    }

    pub fn read(target: &str) -> Result<Option<String>, String> {
        let name = HSTRING::from(target);
        let mut credential: *mut CREDENTIALW = std::ptr::null_mut();
        match unsafe { CredReadW(&name, CRED_TYPE_GENERIC, None, &mut credential) } {
            Ok(()) => {
                // SAFETY: CredReadW succeeded, so `credential` points at a
                // CREDENTIALW it allocated; it is freed once, after the copy.
                let bytes = unsafe {
                    let c = &*credential;
                    let bytes = if c.CredentialBlob.is_null() || c.CredentialBlobSize == 0 {
                        Vec::new()
                    } else {
                        std::slice::from_raw_parts(c.CredentialBlob, c.CredentialBlobSize as usize)
                            .to_vec()
                    };
                    CredFree(credential as *const _);
                    bytes
                };
                String::from_utf8(bytes)
                    .map(Some)
                    .map_err(|_| "the saved key isn't valid text".to_string())
            }
            Err(e) if not_found(&e) => Ok(None),
            Err(e) => Err(e.message()),
        }
    }

    pub fn write(target: &str, secret: &str) -> Result<(), String> {
        let mut name: Vec<u16> = target.encode_utf16().chain(Some(0)).collect();
        let mut user: Vec<u16> = "Legato".encode_utf16().chain(Some(0)).collect();
        let mut blob = secret.as_bytes().to_vec();
        let credential = CREDENTIALW {
            Type: CRED_TYPE_GENERIC,
            TargetName: PWSTR(name.as_mut_ptr()),
            CredentialBlobSize: blob.len() as u32,
            CredentialBlob: blob.as_mut_ptr(),
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            UserName: PWSTR(user.as_mut_ptr()),
            ..Default::default()
        };
        // SAFETY: every pointer in `credential` outlives the call.
        unsafe { CredWriteW(&credential, 0) }.map_err(|e| e.message())
    }

    pub fn delete(target: &str) -> Result<(), String> {
        let name = HSTRING::from(target);
        match unsafe { CredDeleteW(&name, CRED_TYPE_GENERIC, None) } {
            Ok(()) => Ok(()),
            Err(e) if not_found(&e) => Ok(()),
            Err(e) => Err(e.message()),
        }
    }
}

/// Legato only ships on Windows; this keeps other builds compiling.
#[cfg(not(windows))]
mod vault {
    use std::collections::HashMap;
    use std::sync::Mutex;

    static KEYS: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);

    pub fn read(target: &str) -> Result<Option<String>, String> {
        let keys = KEYS.lock().map_err(|e| e.to_string())?;
        Ok(keys.as_ref().and_then(|k| k.get(target).cloned()))
    }

    pub fn write(target: &str, secret: &str) -> Result<(), String> {
        let mut keys = KEYS.lock().map_err(|e| e.to_string())?;
        keys.get_or_insert_with(HashMap::new)
            .insert(target.to_string(), secret.to_string());
        Ok(())
    }

    pub fn delete(target: &str) -> Result<(), String> {
        let mut keys = KEYS.lock().map_err(|e| e.to_string())?;
        if let Some(keys) = keys.as_mut() {
            keys.remove(target);
        }
        Ok(())
    }
}

/// The key for a provider, or an empty string when none is saved. A key
/// still in the settings file (one Credential Manager refused to take) is
/// used until it can be moved.
pub fn api_key(settings: &AppSettings, provider_id: &str) -> String {
    let stored = || {
        settings
            .post_process_api_keys
            .get(provider_id)
            .cloned()
            .unwrap_or_default()
    };
    match vault::read(&target(provider_id)) {
        Ok(Some(key)) if !key.is_empty() => key,
        Ok(_) => stored(),
        Err(e) => {
            warn!("Couldn't read the {provider_id} key from Credential Manager: {e}");
            stored()
        }
    }
}

/// Saves a provider's key, or removes it when `key` is blank, and clears any
/// copy left in the settings file.
pub fn set_api_key(app: &AppHandle, provider_id: &str, key: &str) -> Result<(), String> {
    let key = key.trim();
    let target = target(provider_id);
    if key.is_empty() {
        vault::delete(&target)
    } else {
        vault::write(&target, key)
    }
    .map_err(|e| format!("Couldn't save the key in Windows Credential Manager: {e}"))?;

    let mut settings = settings::get_settings(app);
    if settings
        .post_process_api_keys
        .get(provider_id)
        .is_some_and(|k| !k.is_empty())
    {
        settings
            .post_process_api_keys
            .insert(provider_id.to_string(), String::new());
        settings::write_settings(app, settings);
    }
    Ok(())
}

/// The last four characters of each saved key, for the key fields.
pub fn key_hints(settings: &AppSettings) -> HashMap<String, String> {
    settings
        .post_process_providers
        .iter()
        .filter_map(|provider| {
            let key = api_key(settings, &provider.id);
            if key.is_empty() {
                return None;
            }
            let chars: Vec<char> = key.chars().collect();
            let hint: String = chars[chars.len().saturating_sub(4)..].iter().collect();
            Some((provider.id.clone(), hint))
        })
        .collect()
}

/// Moves keys an older version kept in the settings file into Credential
/// Manager and blanks them in the file. Runs once per start; a key that can't
/// be moved stays where it was and is tried again next time.
pub fn move_keys_out_of_settings(app: &AppHandle) {
    let mut settings = settings::get_settings(app);
    let mut moved = 0;
    let ids: Vec<String> = settings
        .post_process_api_keys
        .iter()
        .filter(|(_, key)| !key.trim().is_empty())
        .map(|(id, _)| id.clone())
        .collect();
    for id in ids {
        let key = settings.post_process_api_keys[&id].trim().to_string();
        match vault::write(&target(&id), &key) {
            Ok(()) => {
                settings.post_process_api_keys.insert(id, String::new());
                moved += 1;
            }
            Err(e) => warn!("Couldn't move the {id} key into Credential Manager: {e}"),
        }
    }
    if moved > 0 {
        settings::write_settings(app, settings);
        info!("Moved {moved} API key(s) from the settings file into Credential Manager");
    }
}
