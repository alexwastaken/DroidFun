//! Read display power only; never sleep, wake, capture, or reconfigure a display.

#[cfg(any(target_os = "macos", test))]
fn all_displays_asleep(states: impl IntoIterator<Item = bool>) -> bool {
    let mut states = states.into_iter().peekable();
    states.peek().is_some() && states.all(|asleep| asleep)
}

pub fn asleep() -> bool {
    state().unwrap_or(false)
}

// None means the system could not provide a complete display-power snapshot.
pub fn state() -> Option<bool> {
    #[cfg(target_os = "macos")]
    {
        #[link(name = "CoreGraphics", kind = "framework")]
        extern "C" {
            fn CGGetOnlineDisplayList(max: u32, displays: *mut u32, count: *mut u32) -> i32;
            // CoreGraphics uses Mach boolean_t (32 bits), not a C/Rust bool.
            fn CGDisplayIsAsleep(display: u32) -> u32;
        }
        let mut total = 0;
        let mut displays = [0u32; 64];
        let mut count = 0;
        // Include sleeping and mirrored displays. Any awake external display
        // keeps live widgets active, even with a laptop lid closed.
        unsafe {
            if CGGetOnlineDisplayList(0, std::ptr::null_mut(), &mut total) != 0
                || total == 0
                || total >= displays.len() as u32
                || CGGetOnlineDisplayList(displays.len() as u32, displays.as_mut_ptr(), &mut count)
                    != 0
                || count == 0
                || count > displays.len() as u32
            {
                return None;
            }
            Some(all_displays_asleep(
                displays[..count as usize]
                    .iter()
                    .map(|id| CGDisplayIsAsleep(*id) != 0),
            ))
        }
    }
    #[cfg(not(target_os = "macos"))]
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_online_display_must_be_asleep() {
        assert!(all_displays_asleep([true]));
        assert!(all_displays_asleep([true, true]));
        assert!(!all_displays_asleep([false]));
        assert!(!all_displays_asleep([true, false]));
        assert!(!all_displays_asleep([false, true]));
        assert!(!all_displays_asleep([]));
    }
}
