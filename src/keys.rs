//! Teclas: código virtual do Windows (VK) → uso HID do teclado (o que o mouse grava), nomes para
//! mostrar, e a leitura do teclado inteiro (GetAsyncKeyState) para gravar macros e atalhos.

use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, MapVirtualKeyW};

/// Teclas de símbolo (dependem do layout, ex.: ABNT2): scan code → uso HID, pela posição física.
const OEM_SCAN: &[(u32, u8)] = &[
    (0x29, 0x35), (0x0c, 0x2d), (0x0d, 0x2e), (0x1a, 0x2f), (0x1b, 0x30), (0x2b, 0x31), (0x27, 0x33),
    (0x28, 0x34), (0x33, 0x36), (0x34, 0x37), (0x35, 0x38), (0x56, 0x64), (0x73, 0x87), (0x7e, 0x85),
];

fn is_oem(vk: u8) -> bool {
    matches!(vk, 0xba..=0xc1 | 0xdb..=0xdf | 0xe2)
}

/// (VK, uso HID, nome)
const KEYS: &[(u8, u8, &str)] = &[
    (0x08, 0x2a, "Backspace"),
    (0x09, 0x2b, "Tab"),
    (0x0d, 0x28, "Enter"),
    (0x13, 0x48, "Pause"),
    (0x14, 0x39, "Caps Lock"),
    (0x1b, 0x29, "Esc"),
    (0x20, 0x2c, "Space"),
    (0x21, 0x4b, "Page Up"),
    (0x22, 0x4e, "Page Down"),
    (0x23, 0x4d, "End"),
    (0x24, 0x4a, "Home"),
    (0x25, 0x50, "Left"),
    (0x26, 0x52, "Up"),
    (0x27, 0x4f, "Right"),
    (0x28, 0x51, "Down"),
    (0x2c, 0x46, "Print Screen"),
    (0x2d, 0x49, "Insert"),
    (0x2e, 0x4c, "Delete"),
    (0x5d, 0x65, "Menu"),
    (0x60, 0x62, "Num 0"),
    (0x61, 0x59, "Num 1"),
    (0x62, 0x5a, "Num 2"),
    (0x63, 0x5b, "Num 3"),
    (0x64, 0x5c, "Num 4"),
    (0x65, 0x5d, "Num 5"),
    (0x66, 0x5e, "Num 6"),
    (0x67, 0x5f, "Num 7"),
    (0x68, 0x60, "Num 8"),
    (0x69, 0x61, "Num 9"),
    (0x6a, 0x55, "Num *"),
    (0x6b, 0x57, "Num +"),
    (0x6d, 0x56, "Num -"),
    (0x6e, 0x63, "Num ."),
    (0x6f, 0x54, "Num /"),
    (0x90, 0x53, "Num Lock"),
    (0x91, 0x47, "Scroll Lock"),
    // modificadores
    (0xa0, 0xe1, "Shift"),
    (0xa1, 0xe5, "Right Shift"),
    (0xa2, 0xe0, "Ctrl"),
    (0xa3, 0xe4, "Right Ctrl"),
    (0xa4, 0xe2, "Alt"),
    (0xa5, 0xe6, "Alt Gr"),
    (0x5b, 0xe3, "Win"),
    (0x5c, 0xe7, "Right Win"),
];

/// Uso HID de uma tecla (letras, números e F1..F12 calculados; o resto pela tabela).
pub fn vk_to_hid(vk: u8) -> Option<u8> {
    match vk {
        b'A'..=b'Z' => Some(vk - b'A' + 0x04),
        b'1'..=b'9' => Some(vk - b'1' + 0x1e),
        b'0' => Some(0x27),
        0x70..=0x7b => Some(vk - 0x70 + 0x3a), // F1..F12
        _ if is_oem(vk) => {
            let sc = unsafe { MapVirtualKeyW(vk as u32, 0) }; // MAPVK_VK_TO_VSC
            OEM_SCAN.iter().find(|k| k.0 == sc).map(|k| k.1)
        }
        _ => KEYS.iter().find(|k| k.0 == vk).map(|k| k.1),
    }
}

pub fn is_modifier(hid: u8) -> bool {
    (0xe0..=0xe7).contains(&hid)
}

pub fn hid_name(hid: u8) -> String {
    match hid {
        0x04..=0x1d => ((b'A' + hid - 0x04) as char).to_string(),
        0x1e..=0x26 => ((b'1' + hid - 0x1e) as char).to_string(),
        0x27 => "0".into(),
        0x3a..=0x45 => format!("F{}", hid - 0x3a + 1),
        _ if OEM_SCAN.iter().any(|k| k.1 == hid) => {
            // o caractere que esta posição tem no layout atual
            let sc = OEM_SCAN.iter().find(|k| k.1 == hid).unwrap().0;
            let ch = unsafe {
                let vk = MapVirtualKeyW(sc, 1); // MAPVK_VSC_TO_VK
                MapVirtualKeyW(vk, 2) & 0x7fff // MAPVK_VK_TO_CHAR
            };
            char::from_u32(ch).filter(|c| !c.is_control()).map_or_else(|| format!("0x{hid:02x}"), |c| c.to_uppercase().to_string())
        }
        _ => KEYS.iter().find(|k| k.1 == hid).map_or_else(|| format!("0x{hid:02x}"), |k| k.2.to_string()),
    }
}

/// Byte de modificadores do HID: Ctrl 0x01, Shift 0x02, Alt 0x04, Win 0x08 (lado direito << 4).
pub fn modifiers_name(m: u8) -> String {
    let mut parts = Vec::new();
    for (bit, name) in [(0x11, "Ctrl"), (0x22, "Shift"), (0x44, "Alt"), (0x88, "Win")] {
        if m & bit != 0 {
            parts.push(name);
        }
    }
    parts.join("+")
}

/// Estado de todas as teclas que o mouse entende (indexado por VK).
pub fn snapshot() -> [bool; 256] {
    let mut s = [false; 256];
    for vk in 0x08..=0xfeu8 {
        if vk_to_hid(vk).is_some() {
            s[vk as usize] = unsafe { (GetAsyncKeyState(vk as i32) as u16 & 0x8000) != 0 };
        }
    }
    s
}

/// Teclas que mudaram desde `prev`: (uso HID, pressionada?). Atualiza `prev`.
pub fn changes(prev: &mut [bool; 256]) -> Vec<(u8, bool)> {
    let now = snapshot();
    let mut out = Vec::new();
    for vk in 0..256 {
        if now[vk] != prev[vk] {
            if let Some(hid) = vk_to_hid(vk as u8) {
                out.push((hid, now[vk]));
            }
        }
    }
    *prev = now;
    out
}

/// Modificadores pressionados agora, no formato do HID.
pub fn held_modifiers(state: &[bool; 256]) -> u8 {
    let mut m = 0u8;
    for (vk, bit) in [(0xa2, 0x01), (0xa0, 0x02), (0xa4, 0x04), (0x5b, 0x08), (0xa3, 0x10), (0xa1, 0x20), (0xa5, 0x40), (0x5c, 0x80)] {
        if state[vk] {
            m |= bit;
        }
    }
    m
}
