//! Who is in a Google Meet call, read from the call window: Windows' UI
//! Automation gives the names the page shows to screen readers. Meet's
//! People panel lists everyone as "Participants"; with it closed, only the
//! controls on a tile ("More options for Priya Shah") carry names, and only
//! some of the time. `calls_rules::meet_people` picks the people out. Only
//! names are read: nothing said in the call, no screenshots, nothing sent.

/// Most elements read from one window, so a huge page can't stall a note.
#[cfg_attr(not(windows), allow(dead_code))]
const MAX_ELEMENTS: i32 = 8000;

/// The people the Meet windows on screen show, without repeats.
pub fn in_meet() -> Vec<String> {
    #[cfg(windows)]
    {
        let (listed, labels) = win::meet_window_names();
        crate::calls_rules::meet_people(&listed, &labels)
    }
    #[cfg(not(windows))]
    Vec::new()
}

#[cfg(windows)]
mod win {
    use super::MAX_ELEMENTS;
    use crate::calls_rules;
    use log::debug;
    use windows::core::BOOL;
    use windows::Win32::Foundation::{HWND, LPARAM};
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
    };
    use windows::Win32::System::Variant::VARIANT;
    use windows::Win32::UI::Accessibility::{
        CUIAutomation, IUIAutomation, IUIAutomationCacheRequest, IUIAutomationCondition,
        IUIAutomationElement, TreeScope, TreeScope_Children, TreeScope_Descendants,
        UIA_ControlTypePropertyId, UIA_ListControlTypeId, UIA_ListItemControlTypeId,
        UIA_NamePropertyId,
    };
    use windows::Win32::UI::WindowsAndMessaging::{EnumWindows, GetWindowTextW, IsWindowVisible};

    unsafe extern "system" fn visit(hwnd: HWND, data: LPARAM) -> BOOL {
        let found = &mut *(data.0 as *mut Vec<HWND>);
        if !IsWindowVisible(hwnd).as_bool() {
            return BOOL(1);
        }
        let mut buf = [0u16; 512];
        let len = GetWindowTextW(hwnd, &mut buf);
        if len > 0 && calls_rules::is_meet_title(&String::from_utf16_lossy(&buf[..len as usize])) {
            found.push(hwnd);
        }
        BOOL(1)
    }

    /// The names of the elements `within` finds, in tree order.
    unsafe fn names(
        element: &IUIAutomationElement,
        within: TreeScope,
        condition: &IUIAutomationCondition,
        cache: &IUIAutomationCacheRequest,
    ) -> Vec<String> {
        let Ok(found) = element.FindAllBuildCache(within, condition, cache) else {
            return Vec::new();
        };
        let count = found.Length().unwrap_or(0).min(MAX_ELEMENTS);
        (0..count)
            .filter_map(|i| found.GetElement(i).and_then(|e| e.CachedName()).ok())
            .map(|name| name.to_string())
            .filter(|name| !name.trim().is_empty())
            .collect()
    }

    /// From the visible Meet windows: the names in the People panel's
    /// "Participants" list, and every element name, in tree order.
    pub fn meet_window_names() -> (Vec<String>, Vec<String>) {
        let mut windows: Vec<HWND> = Vec::new();
        // SAFETY: `windows` outlives the enumeration, which is synchronous.
        unsafe {
            let _ = EnumWindows(Some(visit), LPARAM(&mut windows as *mut Vec<HWND> as isize));
        }
        if windows.is_empty() {
            return (Vec::new(), Vec::new());
        }
        // SAFETY: COM is initialised for this thread, which owns every UI
        // Automation object made here; they're dropped before it returns.
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let automation: IUIAutomation =
                match CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER) {
                    Ok(a) => a,
                    Err(e) => {
                        debug!("UI Automation isn't available: {e}");
                        return (Vec::new(), Vec::new());
                    }
                };
            let conditions = (
                automation.CreateTrueCondition(),
                automation.CreatePropertyCondition(
                    UIA_ControlTypePropertyId,
                    &VARIANT::from(UIA_ListControlTypeId.0),
                ),
                automation
                    .CreatePropertyCondition(UIA_NamePropertyId, &VARIANT::from("Participants")),
                automation.CreatePropertyCondition(
                    UIA_ControlTypePropertyId,
                    &VARIANT::from(UIA_ListItemControlTypeId.0),
                ),
                automation.CreateCacheRequest(),
            );
            let (Ok(all), Ok(is_list), Ok(named), Ok(is_item), Ok(cache)) = conditions else {
                return (Vec::new(), Vec::new());
            };
            let Ok(participants) = automation.CreateAndCondition(&is_list, &named) else {
                return (Vec::new(), Vec::new());
            };
            // The names come back with the elements: one round trip per search.
            let _ = cache.AddProperty(UIA_NamePropertyId);
            let (mut listed, mut labels) = (Vec::new(), Vec::new());
            for hwnd in windows {
                let Ok(window) = automation.ElementFromHandle(hwnd) else {
                    continue;
                };
                if let Ok(list) = window.FindFirst(TreeScope_Descendants, &participants) {
                    listed.extend(names(&list, TreeScope_Children, &is_item, &cache));
                }
                labels.extend(names(&window, TreeScope_Descendants, &all, &cache));
            }
            (listed, labels)
        }
    }
}
