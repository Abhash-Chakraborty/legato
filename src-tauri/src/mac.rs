//! What Legato asks of macOS itself: whether it may watch for its shortcuts
//! and send keys (Accessibility), the System Settings panes that grant that
//! and the microphone, which modifier keys are held, and the key Cmd+V sits
//! on in the keyboard layout in use.

use enigo::Key;
use log::{debug, warn};
use std::ffi::c_void;
use std::process::Command;

/// Where the main window's close, minimise and zoom buttons sit: 18 px in
/// from the left, and centred in the page's 48 px title bar. The second
/// number isn't a distance from the top: the buttons, 14 px tall, sit in
/// the middle of a strip that many pixels taller than they are, so 34
/// puts their centre at (14 + 34) / 2 = 24.
pub const TRAFFIC_LIGHTS: (f64, f64) = (18.0, 34.0);

/// The same for the Scratchpad's 44 px bar: centred at (14 + 30) / 2 = 22.
pub const SCRATCHPAD_TRAFFIC_LIGHTS: (f64, f64) = (14.0, 30.0);

const ACCESSIBILITY_PANE: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility";
const MICROPHONE_PANE: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone";

/// Whether Legato is allowed to control the computer (System Settings ›
/// Privacy & Security › Accessibility). The shortcuts' event tap and the
/// paste keystroke both need it.
pub fn accessibility_granted() -> bool {
    handy_keys::check_accessibility()
}

fn open_pane(url: &str) -> Result<(), String> {
    Command::new("open")
        .arg(url)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Couldn't open System Settings: {e}"))
}

/// Opens System Settings on Accessibility, where the switch for Legato is.
pub fn open_accessibility_settings() -> Result<(), String> {
    open_pane(ACCESSIBILITY_PANE)
}

/// Opens System Settings on Microphone.
pub fn open_microphone_settings() -> Result<(), String> {
    open_pane(MICROPHONE_PANE)
}

type CgEventFlags = u64;
/// `kCGEventSourceStateHIDSystemState`: the keys as the hardware has them.
const HID_SYSTEM_STATE: i32 = 1;
const FLAG_SHIFT: CgEventFlags = 0x0002_0000;
const FLAG_CONTROL: CgEventFlags = 0x0004_0000;
const FLAG_OPTION: CgEventFlags = 0x0008_0000;
const FLAG_COMMAND: CgEventFlags = 0x0010_0000;

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventSourceFlagsState(state_id: i32) -> CgEventFlags;
}

/// Whether Command, Option, Control or Shift is held.
pub fn modifiers_down() -> bool {
    // SAFETY: a plain query with no arguments to keep alive.
    let flags = unsafe { CGEventSourceFlagsState(HID_SYSTEM_STATE) };
    flags & (FLAG_SHIFT | FLAG_CONTROL | FLAG_OPTION | FLAG_COMMAND) != 0
}

type TisInputSourceRef = *const c_void;
type CfDataRef = *const c_void;
type CfStringRef = *const c_void;

/// `kVK_ANSI_V`: where V is on a US keyboard, and where layouts that keep
/// Latin shortcuts under Command (Russian, Greek, Hebrew) leave it.
const ANSI_V_KEYCODE: u16 = 9;
const KEYCODE_COUNT: u16 = 128;
const UC_KEY_ACTION_DISPLAY: u16 = 3;
const UC_KEY_TRANSLATE_NO_DEAD_KEYS_MASK: u32 = 1;
/// Carbon's `cmdKey` is bit 8; `UCKeyTranslate` wants the modifiers shifted
/// right by 8, so Command is bit 0 here.
const COMMAND_MODIFIER_STATE: u32 = 1;

#[link(name = "Carbon", kind = "framework")]
extern "C" {
    fn TISCopyCurrentKeyboardLayoutInputSource() -> TisInputSourceRef;
    fn TISGetInputSourceProperty(
        input_source: TisInputSourceRef,
        property_key: CfStringRef,
    ) -> CfDataRef;
    #[allow(non_upper_case_globals)]
    static kTISPropertyUnicodeKeyLayoutData: CfStringRef;
    fn UCKeyTranslate(
        key_layout: *const u8,
        virtual_key_code: u16,
        key_action: u16,
        modifier_key_state: u32,
        keyboard_type: u32,
        key_translate_options: u32,
        dead_key_state: *mut u32,
        max_string_length: usize,
        actual_string_length: *mut usize,
        unicode_string: *mut u16,
    ) -> i32;
    fn LMGetKbdType() -> u8;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFDataGetBytePtr(data: CfDataRef) -> *const u8;
    fn CFRelease(value: *const c_void);
}

struct InputSource(TisInputSourceRef);

impl Drop for InputSource {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: TISCopyCurrentKeyboardLayoutInputSource returned this
            // retained reference; this gives it back.
            unsafe { CFRelease(self.0) };
        }
    }
}

/// The key macOS reads as `v` while Command is held, in the layout in use.
/// Command matters: Dvorak moves V, while most non-Latin layouts keep the
/// Latin letters for shortcuts.
///
/// The text input APIs must be called on the main thread.
fn resolve_command_v_keycode() -> Result<u16, String> {
    // SAFETY: called on the main thread; the source is released on drop.
    let source = InputSource(unsafe { TISCopyCurrentKeyboardLayoutInputSource() });
    if source.0.is_null() {
        return Err("macOS named no keyboard layout".into());
    }
    // SAFETY: the source stays retained while its property is read.
    let layout_data =
        unsafe { TISGetInputSourceProperty(source.0, kTISPropertyUnicodeKeyLayoutData) };
    if layout_data.is_null() {
        return Err("the keyboard layout has no Unicode layout data".into());
    }
    // SAFETY: the data belongs to the retained source.
    let layout = unsafe { CFDataGetBytePtr(layout_data) };
    if layout.is_null() {
        return Err("the keyboard layout's data is empty".into());
    }
    // SAFETY: no arguments; the keyboard type UCKeyTranslate expects.
    let keyboard_type = u32::from(unsafe { LMGetKbdType() });
    (0..KEYCODE_COUNT)
        .find(|&keycode| {
            let mut dead_key_state = 0;
            let mut chars = [0_u16; 4];
            let mut length = 0_usize;
            // SAFETY: `layout` points at the layout's bytes while `source`
            // lives; every out pointer is a local of the size passed.
            let status = unsafe {
                UCKeyTranslate(
                    layout,
                    keycode,
                    UC_KEY_ACTION_DISPLAY,
                    COMMAND_MODIFIER_STATE,
                    keyboard_type,
                    UC_KEY_TRANSLATE_NO_DEAD_KEYS_MASK,
                    &mut dead_key_state,
                    chars.len(),
                    &mut length,
                    chars.as_mut_ptr(),
                )
            };
            status == 0 && length == 1 && chars[0] == u16::from(b'v')
        })
        .ok_or_else(|| "no key gives Cmd+V in this keyboard layout".to_string())
}

/// The key to press with Command for a paste. Off the main thread the
/// layout can't be asked, so the US position is used.
pub fn command_v_key() -> Key {
    // SAFETY: a query of the calling thread; no arguments.
    let on_main_thread = unsafe { libc::pthread_main_np() } != 0;
    if !on_main_thread {
        debug!("Cmd+V asked for off the main thread; using the US key position");
        return Key::Other(u32::from(ANSI_V_KEYCODE));
    }
    match resolve_command_v_keycode() {
        Ok(keycode) => Key::Other(u32::from(keycode)),
        Err(error) => {
            warn!("Couldn't find Cmd+V in the keyboard layout ({error}); using the US key");
            Key::Other(u32::from(ANSI_V_KEYCODE))
        }
    }
}

/// Windows virtual-key codes the selection and transform code names keys
/// by, as the Mac keys that do the same job. None for a key the Mac has no
/// match for.
pub fn key_for_windows_vk(vk: u32) -> Option<Key> {
    Some(match vk {
        0x25 => Key::LeftArrow,
        0x26 => Key::UpArrow,
        0x27 => Key::RightArrow,
        0x28 => Key::DownArrow,
        // Letters by their US positions, which Command shortcuts follow.
        0x41 => Key::Other(0), // A
        0x43 => Key::Other(8), // C
        0x56 => Key::Other(9), // V
        0x58 => Key::Other(7), // X
        0x5A => Key::Other(6), // Z
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_key_codes_become_mac_keys() {
        assert_eq!(key_for_windows_vk(0x43), Some(Key::Other(8))); // C
        assert_eq!(key_for_windows_vk(0x5A), Some(Key::Other(6))); // Z
        assert_eq!(key_for_windows_vk(0x27), Some(Key::RightArrow));
        // The menu mask key has no Mac counterpart.
        assert_eq!(key_for_windows_vk(0xE8), None);
    }

    /// Tests run off the main thread, where the layout can't be asked.
    #[test]
    fn off_the_main_thread_v_is_the_us_key() {
        assert_eq!(command_v_key(), Key::Other(9));
    }

    /// The calls into macOS link and come back. What they answer depends
    /// on the Mac: a build machine has granted nothing and holds no key.
    #[test]
    fn macos_answers_the_permission_and_key_queries() {
        let _ = accessibility_granted();
        let _ = modifiers_down();
    }
}
