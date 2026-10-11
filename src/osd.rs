//! Aviso na tela ao trocar de DPI: canto inferior direito, some sozinho, não rouba o foco.

use std::ptr::{null, null_mut};
use std::sync::Mutex;

use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::HiDpi::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::mouse::wide_str as wide;

const WM_OSD_SHOW: u32 = WM_APP + 10;
const TIMER_HIDE: usize = 1;

struct Content {
    big: String,
    small: String,
    color: [u8; 3],
    ms: u32,
}

static CONTENT: Mutex<Option<Content>> = Mutex::new(None);
static WINDOW: Mutex<usize> = Mutex::new(0);

pub fn start() {
    std::thread::Builder::new().name("aviso".into()).spawn(|| unsafe { thread_main() }).ok();
}

pub fn show(big: String, small: String, color: [u8; 3], ms: u32) {
    *CONTENT.lock().unwrap() = Some(Content { big, small, color, ms });
    let h = *WINDOW.lock().unwrap();
    if h != 0 {
        unsafe { PostMessageW(h as HWND, WM_OSD_SHOW, 0, 0) };
    }
}

fn scale(v: i32, dpi: u32) -> i32 {
    v * dpi as i32 / 96
}

unsafe fn place(h: HWND) {
    let mut pt: POINT = std::mem::zeroed();
    GetCursorPos(&mut pt);
    let mut mi: MONITORINFO = std::mem::zeroed();
    mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
    GetMonitorInfoW(MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST), &mut mi);
    SetWindowPos(h, HWND_TOPMOST, mi.rcWork.left, mi.rcWork.top, 1, 1, SWP_NOACTIVATE);
    let dpi = GetDpiForWindow(h);
    let (w, ht) = (scale(240, dpi), scale(84, dpi));
    SetWindowPos(
        h,
        HWND_TOPMOST,
        mi.rcWork.right - w - scale(24, dpi),
        mi.rcWork.bottom - ht - scale(24, dpi),
        w,
        ht,
        SWP_NOACTIVATE | SWP_SHOWWINDOW,
    );
    SetWindowRgn(h, CreateRoundRectRgn(0, 0, w + 1, ht + 1, scale(16, dpi), scale(16, dpi)), 1);
}

unsafe fn paint(h: HWND) {
    let mut ps: PAINTSTRUCT = std::mem::zeroed();
    let dc = BeginPaint(h, &mut ps);
    let dpi = GetDpiForWindow(h);
    let (big, small, color) = match &*CONTENT.lock().unwrap() {
        Some(c) => (c.big.clone(), c.small.clone(), c.color),
        None => (String::new(), String::new(), [255, 255, 255]),
    };
    let mut rc: RECT = std::mem::zeroed();
    GetClientRect(h, &mut rc);
    let bg = CreateSolidBrush(0x202020);
    FillRect(dc, &rc, bg);
    SetBkMode(dc, TRANSPARENT as i32);

    let face = wide("Segoe UI");
    let fbig = CreateFontW(-scale(30, dpi), 0, 0, 0, FW_SEMIBOLD as i32, 0, 0, 0, DEFAULT_CHARSET as u32, 0, 0, CLEARTYPE_QUALITY as u32, 0, face.as_ptr());
    let fsmall = CreateFontW(-scale(14, dpi), 0, 0, 0, FW_NORMAL as i32, 0, 0, 0, DEFAULT_CHARSET as u32, 0, 0, CLEARTYPE_QUALITY as u32, 0, face.as_ptr());

    let old = SelectObject(dc, fbig);
    SetTextColor(dc, 0xFFFFFF);
    let mut r = RECT { left: rc.left, top: scale(10, dpi), right: rc.right, bottom: scale(50, dpi) };
    // Sempre terminado em zero: com texto vazio, o ponteiro de um Vec vazio é inválido (0x2) e o
    // Windows o lê mesmo com tamanho 0.
    let mut wbig = wide(&big);
    DrawTextW(dc, wbig.as_mut_ptr(), (wbig.len() - 1) as i32, &mut r, DT_CENTER | DT_SINGLELINE | DT_VCENTER);

    // Linha de baixo: bolinha com a cor do LED + texto, centralizados juntos.
    SelectObject(dc, fsmall);
    SetTextColor(dc, 0xBEBEBE);
    let mut wsmall = wide(&small);
    let small_len = (wsmall.len() - 1) as i32;
    let mut sz: SIZE = std::mem::zeroed();
    GetTextExtentPoint32W(dc, wsmall.as_ptr(), small_len, &mut sz);
    let d = scale(10, dpi);
    let left = (rc.right - (d + scale(8, dpi) + sz.cx)) / 2;
    let dot = CreateSolidBrush(color[0] as u32 | (color[1] as u32) << 8 | (color[2] as u32) << 16);
    let pen = CreatePen(PS_SOLID, 1, 0x5A5A5A);
    let ob = SelectObject(dc, dot);
    let op = SelectObject(dc, pen);
    Ellipse(dc, left, scale(62, dpi) - d / 2, left + d, scale(62, dpi) + d / 2);
    let mut r2 = RECT { left: left + d + scale(8, dpi), top: scale(50, dpi), right: rc.right, bottom: scale(74, dpi) };
    DrawTextW(dc, wsmall.as_mut_ptr(), small_len, &mut r2, DT_LEFT | DT_SINGLELINE | DT_VCENTER);

    SelectObject(dc, old);
    SelectObject(dc, ob);
    SelectObject(dc, op);
    DeleteObject(fbig);
    DeleteObject(fsmall);
    DeleteObject(bg);
    DeleteObject(dot);
    DeleteObject(pen);
    EndPaint(h, &ps);
}

unsafe extern "system" fn wndproc(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    match m {
        WM_OSD_SHOW => {
            place(h);
            InvalidateRect(h, null(), 1);
            let ms = CONTENT.lock().unwrap().as_ref().map_or(1500, |c| c.ms);
            SetTimer(h, TIMER_HIDE, ms, None);
            0
        }
        WM_TIMER => {
            KillTimer(h, TIMER_HIDE);
            ShowWindow(h, SW_HIDE);
            0
        }
        WM_ERASEBKGND => 1,
        WM_PAINT => {
            paint(h);
            0
        }
        WM_NCHITTEST => HTTRANSPARENT as LRESULT,
        _ => DefWindowProcW(h, m, w, l),
    }
}

unsafe fn thread_main() {
    SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    let inst = GetModuleHandleW(null());
    let cls = wide("AkkoEchoUltraOSD");
    let mut wc: WNDCLASSW = std::mem::zeroed();
    wc.lpfnWndProc = Some(wndproc);
    wc.hInstance = inst;
    wc.lpszClassName = cls.as_ptr();
    RegisterClassW(&wc);
    let h = CreateWindowExW(
        WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE,
        cls.as_ptr(),
        wide("").as_ptr(),
        WS_POPUP,
        0,
        0,
        1,
        1,
        null_mut(),
        null_mut(),
        inst,
        null(),
    );
    SetLayeredWindowAttributes(h, 0, 235, LWA_ALPHA);
    *WINDOW.lock().unwrap() = h as usize;
    let mut msg: MSG = std::mem::zeroed();
    while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
        TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }
}
