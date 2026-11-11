//! Inicialização com o Windows (HKCU\...\Run).

use std::ptr::null_mut;

use windows_sys::Win32::Foundation::ERROR_SUCCESS;
use windows_sys::Win32::System::Registry::*;

use crate::mouse::wide_str as wide;

const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const NAME: &str = "Akko Echo Ultra";

fn run_value_exists(name: &str) -> bool {
    unsafe {
        let mut size = 0u32;
        RegGetValueW(HKEY_CURRENT_USER, wide(RUN).as_ptr(), wide(name).as_ptr(), RRF_RT_REG_SZ, null_mut(), null_mut(), &mut size)
            == ERROR_SUCCESS
    }
}

pub fn is_enabled() -> bool {
    run_value_exists(NAME)
}

pub fn set_enabled(on: bool) {
    unsafe {
        let mut key = null_mut();
        if RegOpenKeyExW(HKEY_CURRENT_USER, wide(RUN).as_ptr(), 0, KEY_SET_VALUE, &mut key) != ERROR_SUCCESS {
            return;
        }
        if on {
            let exe = std::env::current_exe().unwrap_or_default();
            let cmd = wide(&format!("\"{}\" --bandeja", exe.display()));
            RegSetValueExW(key, wide(NAME).as_ptr(), 0, REG_SZ, cmd.as_ptr() as *const u8, (cmd.len() * 2) as u32);
        } else {
            RegDeleteValueW(key, wide(NAME).as_ptr());
        }
        RegCloseKey(key);
    }
}
