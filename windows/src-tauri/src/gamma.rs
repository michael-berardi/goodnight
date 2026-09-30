//! Tints every display through its hardware gamma ramp (the path f.lux uses on Windows).
//! Nothing is drawn on screen and the GPU does no extra work.
//!
//! Windows refuses strong ramps unless `GdiIcmGammaRange` is 256 under
//! HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\ICM; the installer sets it.
//! Unlike macOS, Windows keeps a ramp after the process exits, so `restore` runs on
//! quit and from the panic hook.

use crate::warmth::knee;

#[cfg(windows)]
mod imp {
    use std::sync::Mutex;
    use windows_sys::Win32::Graphics::Gdi::{CreateDCW, DeleteDC, EnumDisplayDevicesW, DISPLAY_DEVICEW, DISPLAY_DEVICE_ATTACHED_TO_DESKTOP};
    use windows_sys::Win32::UI::ColorSystem::{GetDeviceGammaRamp, SetDeviceGammaRamp};

    pub type Ramp = [[u16; 256]; 3];

    pub struct Screen {
        pub device: Vec<u16>,
        pub base: Ramp,
    }

    pub static SCREENS: Mutex<Vec<Screen>> = Mutex::new(Vec::new());

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub fn identity() -> Ramp {
        let mut r = [[0u16; 256]; 3];
        for ch in r.iter_mut() {
            for (i, v) in ch.iter_mut().enumerate() {
                *v = (i as u32 * 257) as u16;
            }
        }
        r
    }

    /// Runs `f` with a device context for the display named `device` (NUL-terminated UTF-16).
    pub fn with_dc<T>(device: &[u16], f: impl FnOnce(*mut core::ffi::c_void) -> T) -> Option<T> {
        let driver = wide("DISPLAY");
        // SAFETY: both strings are NUL-terminated and outlive the call; the DC is deleted below.
        let dc = unsafe { CreateDCW(driver.as_ptr(), device.as_ptr(), std::ptr::null(), std::ptr::null()) };
        if dc.is_null() {
            return None;
        }
        let out = f(dc as _);
        unsafe { DeleteDC(dc) };
        Some(out)
    }

    pub fn displays() -> Vec<Vec<u16>> {
        let mut out = Vec::new();
        for i in 0.. {
            let mut dd: DISPLAY_DEVICEW = unsafe { std::mem::zeroed() };
            dd.cb = std::mem::size_of::<DISPLAY_DEVICEW>() as u32;
            // SAFETY: dd is a properly sized, zeroed DISPLAY_DEVICEW.
            if unsafe { EnumDisplayDevicesW(std::ptr::null(), i, &mut dd, 0) } == 0 {
                break;
            }
            if dd.StateFlags & DISPLAY_DEVICE_ATTACHED_TO_DESKTOP != 0 {
                let len = dd.DeviceName.iter().position(|&c| c == 0).unwrap_or(dd.DeviceName.len());
                let mut name = dd.DeviceName[..len].to_vec();
                name.push(0);
                out.push(name);
            }
        }
        out
    }

    pub fn get(dc: *mut core::ffi::c_void) -> Option<Ramp> {
        let mut r: Ramp = [[0; 256]; 3];
        // SAFETY: r is 3×256 u16, the layout GetDeviceGammaRamp expects.
        (unsafe { GetDeviceGammaRamp(dc as _, r.as_mut_ptr() as _) } != 0).then_some(r)
    }

    pub fn set(dc: *mut core::ffi::c_void, r: &Ramp) -> bool {
        // SAFETY: r is 3×256 u16, the layout SetDeviceGammaRamp expects.
        unsafe { SetDeviceGammaRamp(dc as _, r.as_ptr() as _) != 0 }
    }
}

/// Reads each display's current ramp to keep calibration curves. A ramp that already looks
/// tinted (left behind by a crash or another app) is replaced by the identity ramp.
pub fn capture() -> usize {
    #[cfg(windows)]
    {
        let mut screens = imp::SCREENS.lock().unwrap();
        screens.clear();
        for device in imp::displays() {
            let base = imp::with_dc(&device, imp::get).flatten().filter(|r| {
                let (red, blue) = (r[0][255] as f64, r[2][255] as f64);
                red > 60000.0 && blue > red * 0.9
            });
            screens.push(imp::Screen { device, base: base.unwrap_or_else(imp::identity) });
        }
        return screens.len();
    }
    #[cfg(not(windows))]
    0
}

/// Applies gains, highlight dim and brightness. Returns a plain-language error naming the cause.
pub fn apply(gain: [f64; 3], dim: f64, brightness: f64) -> Result<(), String> {
    #[cfg(windows)]
    {
        let screens = imp::SCREENS.lock().unwrap();
        let mut failed = Vec::new();
        for s in screens.iter() {
            let mut r = [[0u16; 256]; 3];
            for ch in 0..3 {
                let k = gain[ch] * brightness;
                for i in 0..256 {
                    let e = s.base[ch][i] as f64 / 65535.0;
                    r[ch][i] = (knee(e, dim) * k * 65535.0).round().clamp(0.0, 65535.0) as u16;
                }
            }
            if imp::with_dc(&s.device, |dc| imp::set(dc, &r)) != Some(true) {
                failed.push(String::from_utf16_lossy(&s.device[..s.device.len() - 1]));
            }
        }
        if !failed.is_empty() {
            return Err(format!(
                "Windows refused the colour change on {}. Reinstall Good Night so it can allow the full colour range (GdiIcmGammaRange).",
                failed.join(", ")
            ));
        }
        return Ok(());
    }
    #[cfg(not(windows))]
    {
        let _ = (gain, dim, brightness, knee as fn(f64, f64) -> f64);
        Ok(())
    }
}

/// Puts every display back the way Good Night found it.
pub fn restore() {
    #[cfg(windows)]
    if let Ok(screens) = imp::SCREENS.try_lock() {
        for s in screens.iter() {
            imp::with_dc(&s.device, |dc| imp::set(dc, &s.base));
        }
    }
}

/// Number of displays attached right now, to notice plugging and unplugging.
pub fn display_count() -> usize {
    #[cfg(windows)]
    return imp::displays().len();
    #[cfg(not(windows))]
    0
}
