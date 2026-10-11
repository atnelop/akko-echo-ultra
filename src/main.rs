//! Akko Echo Ultra: configuração do mouse Akko TAN8 Ultra sem o app da Akko.
//!
//! Modos:
//!   AkkoEchoUltra.exe            abre a bandeja (se ainda não estiver aberta) e a janela
//!   AkkoEchoUltra.exe --bandeja  só a bandeja (usado ao iniciar com o Windows)
//!   AkkoEchoUltra.exe --ui       só a janela de configuração (aberta pela bandeja)
#![windows_subsystem = "windows"]

mod autostart;
mod i18n;
mod keys;
mod mouse;
mod osd;
mod state;
mod tray;
mod ui;

use std::io::Write;

/// Registro simples em %LOCALAPPDATA%\Akko Echo Ultra\echo.log (recomeça ao passar de 1 MB).
pub fn log(msg: &str) {
    let path = state::data_dir().join("echo.log");
    if std::fs::metadata(&path).map_or(false, |m| m.len() > 1_000_000) {
        let _ = std::fs::remove_file(&path);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let _ = writeln!(f, "{t} {msg}");
    }
}

/// Registra exceções graves (endereço, módulo e thread) antes de o processo cair.
unsafe extern "system" fn on_exception(info: *mut windows_sys::Win32::System::Diagnostics::Debug::EXCEPTION_POINTERS) -> i32 {
    use windows_sys::Win32::System::LibraryLoader::*;
    let rec = &*(*info).ExceptionRecord;
    let code = rec.ExceptionCode as u32;
    if code == 0xC0000005 || code == 0xC000041D || code == 0xC0000409 {
        let addr = rec.ExceptionAddress as usize;
        let mut module = std::ptr::null_mut();
        GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            addr as *const u16,
            &mut module,
        );
        let mut name = [0u16; 260];
        let n = GetModuleFileNameW(module, name.as_mut_ptr(), 260) as usize;
        let thread = std::thread::current().name().unwrap_or("?").to_string();
        log(&format!(
            "EXCECAO 0x{code:08X} em {}+0x{:x} thread '{thread}' (leitura/escrita em 0x{:x})",
            String::from_utf16_lossy(&name[..n]),
            addr - module as usize,
            rec.ExceptionInformation[1]
        ));
    }
    0 // continua a busca de tratador
}

fn main() {
    std::panic::set_hook(Box::new(|info| log(&format!("erro interno: {info}"))));
    unsafe { windows_sys::Win32::System::Diagnostics::Debug::AddVectoredExceptionHandler(1, Some(on_exception)) };
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() >= 3 && args[0] == "--icone-png" {
        tray::dump_icon(&args[1], args[2].parse().unwrap_or(-1));
        return;
    }
    if args.iter().any(|a| a == "--ui") {
        ui::run();
        return;
    }
    let bandeja = args.iter().any(|a| a == "--bandeja");
    // Uma única bandeja (só ela fala com o mouse): o mutex evita corrida entre dois inícios.
    let first = unsafe {
        use windows_sys::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
        use windows_sys::Win32::System::Threading::CreateMutexW;
        CreateMutexW(std::ptr::null(), 1, mouse::wide_str("Local\\AkkoEchoUltra.Bandeja").as_ptr());
        GetLastError() != ERROR_ALREADY_EXISTS
    };
    if !first {
        if !bandeja {
            // Espera a janela da outra instância existir e pede para ela abrir a configuração.
            for _ in 0..50 {
                if let Some(h) = tray::find_existing() {
                    tray::post_open_ui(h);
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
        return;
    }
    log("iniciado");
    tray::run(!bandeja);
}
