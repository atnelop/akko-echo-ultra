//! Inicialização com o Windows (HKCU\...\Run) e desligamento da inicialização do Akko.

use std::path::PathBuf;
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

fn startup_folder() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join(r"Microsoft\Windows\Start Menu\Programs\Startup"))
}

const AKKO_STARTUP_FILES: [&str; 2] = ["Akko.lnk", "akko-minimizado.vbs"];

/// O Akko abre sozinho no login (entrada "Akko" no Run ou atalhos na pasta Inicializar)?
pub fn akko_enabled() -> bool {
    run_value_exists("Akko")
        || startup_folder().map_or(false, |d| AKKO_STARTUP_FILES.iter().any(|f| d.join(f).exists()))
}

/// Desliga a inicialização do Akko: remove a entrada "Akko" do Run, move os atalhos da pasta
/// Inicializar para a pasta de dados deste programa (dá para devolver) e desmarca a opção do
/// próprio Akko (senão ele recria a entrada).
pub fn disable_akko() {
    unsafe {
        let mut key = null_mut();
        if RegOpenKeyExW(HKEY_CURRENT_USER, wide(RUN).as_ptr(), 0, KEY_SET_VALUE, &mut key) == ERROR_SUCCESS {
            RegDeleteValueW(key, wide("Akko").as_ptr());
            RegCloseKey(key);
        }
    }
    if let Some(dir) = startup_folder() {
        let backup = crate::state::data_dir().join("inicializacao-akko-removida");
        let _ = std::fs::create_dir_all(&backup);
        for f in AKKO_STARTUP_FILES {
            let p = dir.join(f);
            if p.exists() {
                let _ = std::fs::rename(&p, backup.join(f)).or_else(|_| std::fs::remove_file(&p));
            }
        }
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        let setting = PathBuf::from(local).join(r"Akko\Setting.json");
        if let Ok(s) = std::fs::read_to_string(&setting) {
            let _ = std::fs::write(&setting, s.replace("\"run_on_windows_startup\" : true", "\"run_on_windows_startup\" : false"));
        }
    }
    crate::log("inicialização do Akko desligada");
}
