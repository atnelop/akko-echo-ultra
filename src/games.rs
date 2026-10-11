//! DPI por jogo: descobre qual programa está em primeiro plano e lista os programas com janela
//! aberta (para escolher o jogo na tela). Só consulta o Windows; não fala com o mouse.

use windows_sys::Win32::Foundation::{CloseHandle, BOOL, HWND, LPARAM};
use windows_sys::Win32::System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION};
use windows_sys::Win32::UI::WindowsAndMessaging::{EnumWindows, GetForegroundWindow, GetWindow, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible, GW_OWNER};

/// Nome do executável (minúsculo, ex.: "cs2.exe") de um processo.
pub fn process_exe(pid: u32) -> Option<String> {
    unsafe {
        let hp = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if hp.is_null() {
            return None;
        }
        let mut buf = [0u16; 520];
        let mut n = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(hp, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut n);
        CloseHandle(hp);
        if ok == 0 {
            return None;
        }
        let path = String::from_utf16_lossy(&buf[..n as usize]);
        path.rsplit('\\').next().map(|s| s.to_lowercase())
    }
}

fn window_exe(h: HWND) -> Option<String> {
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(h, &mut pid) };
    (pid != 0).then(|| process_exe(pid)).flatten()
}

/// Executável da janela em primeiro plano.
pub fn foreground_exe() -> Option<String> {
    let h = unsafe { GetForegroundWindow() };
    if h.is_null() {
        return None;
    }
    window_exe(h)
}

/// Programas com janela visível agora: (executável, título da janela), um por executável.
pub fn window_apps() -> Vec<(String, String)> {
    unsafe extern "system" fn each(h: HWND, l: LPARAM) -> BOOL {
        let out = &mut *(l as *mut Vec<(String, String)>);
        if IsWindowVisible(h) == 0 || !GetWindow(h, GW_OWNER).is_null() {
            return 1;
        }
        let len = GetWindowTextLengthW(h);
        if len <= 0 {
            return 1;
        }
        let mut buf = vec![0u16; len as usize + 1];
        let n = GetWindowTextW(h, buf.as_mut_ptr(), buf.len() as i32);
        let title = String::from_utf16_lossy(&buf[..n.max(0) as usize]);
        if let Some(exe) = window_exe(h) {
            const SKIP: [&str; 5] = ["explorer.exe", "akkoechoultra.exe", "textinputhost.exe", "applicationframehost.exe", "systemsettings.exe"];
            if !SKIP.contains(&exe.as_str()) && !out.iter().any(|(e, _)| *e == exe) {
                out.push((exe, title));
            }
        }
        1
    }
    let mut out: Vec<(String, String)> = Vec::new();
    unsafe { EnumWindows(Some(each), &mut out as *mut _ as LPARAM) };
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}
