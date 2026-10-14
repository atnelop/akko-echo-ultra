//! Comunicação com o mouse Akko TAN8 Ultra (receptor 2.4G, VID 0x38EE, PID 0x0024).
//!
//! Protocolo (interface de configuração "col04", relatórios de 64 bytes):
//!   pedido:   04 [checksum lo] [checksum hi] [cmd] [tamanho] [endereço] 00 00 [dados...]
//!   checksum: soma dos bytes a partir do índice 3 (16 bits)
//!   cmd 01 = inicia gravação, 06 = grava bloco, 02 = confirma; 05 = lê bloco; 1a = bateria;
//!   07 = lê a tabela dos botões, 09 = grava a tabela dos botões, 15 = grava o macro
//!   (descobertos em 2026-10-10 capturando o app da Akko)
//!   resposta: mesmo cabeçalho, dados a partir do índice 8
//!
//! Cuidados descobertos na prática (2026-10-10):
//! - cada pedido faz o receptor gerar ~1 px de movimento falso no cursor: nada de consultas
//!   periódicas frequentes;
//! - dois programas falando com o mouse ao mesmo tempo corrompem a memória: com o Akko aberto,
//!   este programa não fala com o mouse;
//! - gravações parciais podem ser perdidas (um bloco inteiro zerado) ao desligar/ligar o mouse:
//!   a memória é sempre gravada inteira;
//! - ao entrar em 8000 Hz pelo software o receptor pode parar de responder até o mouse ser
//!   desligado e ligado, e o estágio 2 pode ser zerado: ver `Device::apply` e a restauração dos
//!   estágios na bandeja.

use std::mem::{size_of, zeroed};
use std::ptr::{null, null_mut};
use std::time::{Duration, Instant};

use windows_sys::core::GUID;
use windows_sys::Win32::Devices::DeviceAndDriverInstallation::*;
use windows_sys::Win32::Devices::HumanInterfaceDevice::*;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Storage::FileSystem::*;
use windows_sys::Win32::System::Threading::*;
use windows_sys::Win32::System::IO::*;

pub const MEM_SIZE: usize = 0x80;
pub const STAGES: usize = 4; // estágios efetivos; 5..8 são cópias de 1..4
const STAGES_FW: usize = 8;
const STAGE0: usize = 0x0e;
const STAGE_SIZE: usize = 9;
const ACTIVE: usize = 0x0c;
const POLL_A: usize = 0x0a;
const POLL_B: usize = 0x0b;
const POLL_C: usize = 0x60;
const LOD_A: usize = 0x0d;
const LOD_B: usize = 0x5b;
const SLEEP: usize = 0x58; // segundos, little-endian
const DEBOUNCE: usize = 0x61;
const FW_QUICK_SWITCH: usize = 0x66;
const ANGLE_SNAP: usize = 0x5c;
const RIPPLE: usize = 0x5d;
const MOTION_SYNC: usize = 0x62;
// Luz (mesma numeração de modos do app da Akko, DefaultData\default_light.json)
const LIGHT_MODE: usize = 0x01;
const LIGHT_MULTI: usize = 0x05; // 1 = usa a cor de cada estágio de DPI
const LIGHT_RGB: usize = 0x06;
const BLOCKS: [u8; 6] = [0x00, 0x18, 0x30, 0x48, 0x60, 0x78];

/// Polling (código 0..6). 8000 Hz funciona, mas entrar nele pelo software pode travar a
/// comunicação até o mouse ser desligado e ligado (limitação do receptor).
pub const POLLING_HZ: [u32; 7] = [125, 250, 500, 1000, 2000, 4000, 8000];
pub const DEBOUNCE_LABELS: [&str; 7] = ["0.125ms", "1ms", "2ms", "4ms", "8ms", "16ms", "24ms"];

pub fn polling_code(hz: u32) -> Option<u8> {
    POLLING_HZ.iter().position(|&h| h == hz).map(|c| c as u8)
}

pub fn polling_hz_of(code: u8) -> u32 {
    POLLING_HZ.get(code as usize).copied().unwrap_or(0)
}

/// Cópia da memória de configuração do mouse (0x00..0x7f).
#[derive(Clone, PartialEq, Eq)]
pub struct Mem(pub [u8; MEM_SIZE]);

impl Mem {
    fn stage(&self, i: usize) -> &[u8] {
        &self.0[STAGE0 + i * STAGE_SIZE..STAGE0 + (i + 1) * STAGE_SIZE]
    }
    pub fn dpi(&self, i: usize) -> u16 {
        let s = self.stage(i);
        u16::from_le_bytes([s[2], s[3]])
    }
    pub fn color(&self, i: usize) -> [u8; 3] {
        let s = self.stage(i);
        [s[6], s[7], s[8]]
    }
    pub fn set_dpi(&mut self, i: usize, dpi: u16) {
        let o = STAGE0 + i * STAGE_SIZE;
        let b = dpi.to_le_bytes();
        self.0[o] = 1;
        self.0[o + 1] = 1; // X e Y iguais
        self.0[o + 2..o + 4].copy_from_slice(&b);
        self.0[o + 4..o + 6].copy_from_slice(&b);
    }
    pub fn set_color(&mut self, i: usize, rgb: [u8; 3]) {
        let o = STAGE0 + i * STAGE_SIZE;
        self.0[o + 6..o + 9].copy_from_slice(&rgb);
    }
    /// Estágio ativo entre os efetivos (0..STAGES).
    pub fn active(&self) -> usize {
        self.0[ACTIVE] as usize % STAGES
    }
    pub fn set_active(&mut self, i: usize) {
        self.0[ACTIVE] = i as u8;
    }
    pub fn polling(&self) -> u8 {
        self.0[POLL_C]
    }
    pub fn polling_hz(&self) -> u32 {
        polling_hz_of(self.polling())
    }
    pub fn set_polling(&mut self, code: u8) {
        self.0[POLL_A] = code;
        self.0[POLL_B] = code;
        self.0[POLL_C] = code;
    }
    pub fn lod(&self) -> u8 {
        self.0[LOD_A]
    }
    pub fn set_lod(&mut self, v: u8) {
        self.0[LOD_A] = v;
        self.0[LOD_B] = v;
    }
    pub fn debounce(&self) -> u8 {
        self.0[DEBOUNCE]
    }
    pub fn set_debounce(&mut self, v: u8) {
        self.0[DEBOUNCE] = v;
    }
    pub fn sleep_s(&self) -> u16 {
        u16::from_le_bytes([self.0[SLEEP], self.0[SLEEP + 1]])
    }
    pub fn angle_snap(&self) -> bool {
        self.0[ANGLE_SNAP] != 0
    }
    pub fn set_angle_snap(&mut self, on: bool) {
        self.0[ANGLE_SNAP] = on as u8;
    }
    pub fn ripple(&self) -> bool {
        self.0[RIPPLE] != 0
    }
    pub fn set_ripple(&mut self, on: bool) {
        self.0[RIPPLE] = on as u8;
    }
    pub fn motion_sync(&self) -> bool {
        self.0[MOTION_SYNC] != 0
    }
    pub fn set_motion_sync(&mut self, on: bool) {
        self.0[MOTION_SYNC] = on as u8;
    }
    pub fn set_sleep_s(&mut self, s: u16) {
        self.0[SLEEP..SLEEP + 2].copy_from_slice(&s.to_le_bytes());
    }
    pub fn set_light_mode(&mut self, v: u8) {
        self.0[LIGHT_MODE] = v;
    }
    pub fn set_light_multi(&mut self, on: bool) {
        self.0[LIGHT_MULTI] = on as u8;
    }
    pub fn set_light_rgb(&mut self, rgb: [u8; 3]) {
        self.0[LIGHT_RGB..LIGHT_RGB + 3].copy_from_slice(&rgb);
    }
    /// Grava um byte qualquer (bytes da luz sem nome conhecido).
    pub fn set_byte(&mut self, addr: usize, v: u8) {
        self.0[addr] = v;
    }
    /// Memória em hexadecimal, 16 bytes por linha.
    pub fn hex(&self) -> String {
        self.0
            .chunks(16)
            .enumerate()
            .map(|(i, c)| format!("{:02x}: {}
", i * 16, c.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ")))
            .collect()
    }
    /// Normaliza antes de gravar: estágios 5..8 = cópias dos 1..4 (o firmware cicla pelos 8) e
    /// troca rápida do firmware desligada (a troca é feita por este programa).
    pub fn normalize(&mut self, own_switch: bool) {
        for i in STAGES..STAGES_FW {
            let src = STAGE0 + (i % STAGES) * STAGE_SIZE;
            let dst = STAGE0 + i * STAGE_SIZE;
            let copy: [u8; STAGE_SIZE] = self.0[src..src + STAGE_SIZE].try_into().unwrap();
            self.0[dst..dst + STAGE_SIZE].copy_from_slice(&copy);
            self.0[dst] = 1;
        }
        if own_switch {
            self.0[FW_QUICK_SWITCH] = 0;
        }
        // O estágio ativo sempre aponta para um dos efetivos.
        self.0[ACTIVE] %= STAGES as u8;
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Caminho da interface de configuração (col04) do mouse, se conectado.
fn find_config_path() -> Option<Vec<u16>> {
    find_hid_path("&col04", true)
}

/// Caminho de uma coleção HID do mouse (ex.: "&col03"), preferindo o modo cabo (PID 0023).
/// `config` exige relatórios de 64 bytes (a interface de configuração).
fn find_hid_path(col: &str, config: bool) -> Option<Vec<u16>> {
    unsafe {
        let mut guid: GUID = zeroed();
        HidD_GetHidGuid(&mut guid);
        let info = SetupDiGetClassDevsW(&guid, null(), null_mut(), DIGCF_PRESENT | DIGCF_DEVICEINTERFACE);
        if info as isize == -1 {
            return None;
        }
        // O modo cabo (PID 0023) tem preferência: com o cabo ligado, o receptor pode continuar
        // presente mas sem o mouse do outro lado.
        let mut found: Option<Vec<u16>> = None;
        let mut i = 0;
        loop {
            let mut did: SP_DEVICE_INTERFACE_DATA = zeroed();
            did.cbSize = size_of::<SP_DEVICE_INTERFACE_DATA>() as u32;
            if SetupDiEnumDeviceInterfaces(info, null(), &guid, i, &mut did) == 0 {
                break;
            }
            i += 1;
            let mut need = 0u32;
            SetupDiGetDeviceInterfaceDetailW(info, &did, null_mut(), 0, &mut need, null_mut());
            if need == 0 {
                continue;
            }
            let mut buf = vec![0u64; need as usize / 8 + 2];
            let detail = buf.as_mut_ptr() as *mut SP_DEVICE_INTERFACE_DETAIL_DATA_W;
            (*detail).cbSize = size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32;
            if SetupDiGetDeviceInterfaceDetailW(info, &did, detail, need, null_mut(), null_mut()) == 0 {
                continue;
            }
            let p = std::ptr::addr_of!((*detail).DevicePath) as *const u16;
            let mut len = 0;
            while *p.add(len) != 0 {
                len += 1;
            }
            let path = std::slice::from_raw_parts(p, len);
            let lower = String::from_utf16_lossy(path).to_lowercase();
            // PID 0024 = pelo receptor 2.4G; PID 0023 = mouse ligado no cabo (carregando).
            let ours = lower.contains("vid_38ee&pid_0024") || lower.contains("vid_38ee&pid_0023");
            if ours && lower.contains(col) && (!config || has_config_caps(path)) {
                let mut v = path.to_vec();
                v.push(0);
                let wired = lower.contains("pid_0023");
                if wired || found.is_none() {
                    found = Some(v);
                }
                if wired {
                    break;
                }
            }
        }
        SetupDiDestroyDeviceInfoList(info);
        found
    }
}

unsafe fn has_config_caps(path: &[u16]) -> bool {
    let mut z = path.to_vec();
    z.push(0);
    let h = CreateFileW(z.as_ptr(), 0, FILE_SHARE_READ | FILE_SHARE_WRITE, null(), OPEN_EXISTING, 0, null_mut());
    if h == INVALID_HANDLE_VALUE {
        return false;
    }
    let mut pp = 0;
    let mut ok = false;
    if HidD_GetPreparsedData(h, &mut pp) != 0 {
        let mut caps: HIDP_CAPS = zeroed();
        HidP_GetCaps(pp, &mut caps);
        ok = caps.InputReportByteLength == 64 && caps.OutputReportByteLength == 64;
        HidD_FreePreparsedData(pp);
    }
    CloseHandle(h);
    ok
}

#[derive(Debug)]
#[allow(dead_code)] // o código do erro aparece no log via Debug
pub enum Error {
    NoReply,
    Io(u32),
    Unknown,
}

pub struct Device {
    h: HANDLE,
    ev: HANDLE,
    /// Conectado pelo cabo (PID 0023), não pelo receptor 2.4G.
    pub wired: bool,
}

unsafe impl Send for Device {}

impl Drop for Device {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.h);
            CloseHandle(self.ev);
        }
    }
}

impl Device {
    pub fn open() -> Option<Device> {
        let path = find_config_path()?;
        let wired = String::from_utf16_lossy(&path).to_lowercase().contains("pid_0023");
        unsafe {
            let h = CreateFileW(
                path.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                null(),
                OPEN_EXISTING,
                FILE_FLAG_OVERLAPPED,
                null_mut(),
            );
            if h == INVALID_HANDLE_VALUE {
                return None;
            }
            let ev = CreateEventW(null(), 1, 0, null());
            Some(Device { h, ev, wired })
        }
    }

    unsafe fn io(&self, write: bool, buf: &mut [u8; 64], ms: u32) -> Result<u32, Error> {
        let mut ov: OVERLAPPED = zeroed();
        ov.hEvent = self.ev;
        ResetEvent(self.ev);
        let mut n = 0u32;
        let ok = if write {
            WriteFile(self.h, buf.as_ptr(), 64, null_mut(), &mut ov)
        } else {
            ReadFile(self.h, buf.as_mut_ptr(), 64, null_mut(), &mut ov)
        };
        if ok == 0 {
            let e = GetLastError();
            if e != ERROR_IO_PENDING {
                return Err(Error::Io(e));
            }
        }
        if WaitForSingleObject(self.ev, ms) != WAIT_OBJECT_0 {
            CancelIo(self.h);
            GetOverlappedResult(self.h, &ov, &mut n, 1);
            return Err(Error::NoReply);
        }
        if GetOverlappedResult(self.h, &ov, &mut n, 0) == 0 {
            return Err(Error::Io(GetLastError()));
        }
        Ok(n)
    }

    /// Envia um pedido e espera a resposta correspondente.
    fn request(&self, cmd: u8, len: u8, addr: u8, data: &[u8]) -> Result<[u8; 64], Error> {
        let mut pkt = [0u8; 64];
        pkt[0] = 4;
        pkt[3] = cmd;
        pkt[4] = len;
        pkt[5] = addr;
        pkt[8..8 + data.len()].copy_from_slice(data);
        let sum: u32 = pkt[3..].iter().map(|&b| b as u32).sum();
        pkt[1] = sum as u8;
        pkt[2] = (sum >> 8) as u8;
        // Estes ecoam tamanho e endereço na resposta; os demais, só o comando.
        let mem_cmd = matches!(cmd, 0x05 | 0x06 | 0x07 | 0x09 | 0x15);
        unsafe {
            HidD_FlushQueue(self.h);
            self.io(true, &mut pkt, 500)?;
            let t0 = Instant::now();
            while t0.elapsed() < Duration::from_millis(600) {
                let mut r = [0u8; 64];
                let n = self.io(false, &mut r, 300)?;
                if n >= 64 && r[0] == 4 && r[3] == cmd && (!mem_cmd || (r[4] == len && r[5] == addr)) {
                    return Ok(r);
                }
            }
        }
        Err(Error::NoReply)
    }

    pub fn read_mem(&self) -> Result<Mem, Error> {
        let mut m = [0u8; MEM_SIZE];
        for &a in BLOCKS.iter() {
            let len = (MEM_SIZE - a as usize).min(0x18) as u8;
            let r = self.request(0x05, len, a, &[])?;
            m[a as usize..a as usize + len as usize].copy_from_slice(&r[8..8 + len as usize]);
        }
        Ok(Mem(m))
    }

    fn write_blocks(&self, mem: &Mem, blocks: &[u8]) -> Result<(), Error> {
        self.request(0x01, 0, 0, &[])?;
        let mut result = Ok(());
        for &a in blocks {
            let len = (MEM_SIZE - a as usize).min(0x18);
            let data = &mem.0[a as usize..a as usize + len];
            match self.request(0x06, len as u8, a, data) {
                Ok(r) if &r[8..8 + len] == data => {}
                Ok(_) => {
                    result = Err(Error::Unknown);
                    break;
                }
                Err(e) => {
                    result = Err(e);
                    break;
                }
            }
        }
        let _ = self.request(0x02, 0, 0, &[]);
        result
    }

    /// Grava `new` (sempre a memória inteira). Ao entrar em 8000 Hz, primeiro grava tudo com o
    /// polling antigo (fica salvo mesmo que o receptor pare de responder) e só depois o 8 kHz.
    pub fn apply(&self, old: &Mem, new: &Mem) -> Result<(), Error> {
        if new.polling() == 6 && old.polling() != 6 {
            let mut safe = new.clone();
            safe.set_polling(old.polling());
            self.write_blocks(&safe, &BLOCKS)?;
        }
        self.write_blocks(new, &BLOCKS)
    }

    /// Tabela dos botões (0x24 bytes: 12 entradas de 3 bytes).
    pub fn read_buttons(&self) -> Result<Buttons, Error> {
        let mut b = [0u8; BTN_LEN];
        let r = self.request(0x07, 0x18, 0x00, &[])?;
        b[..0x18].copy_from_slice(&r[8..8 + 0x18]);
        let r = self.request(0x07, 0x0c, 0x18, &[])?;
        b[0x18..].copy_from_slice(&r[8..8 + 0x0c]);
        Ok(Buttons(b))
    }

    /// Grava a tabela dos botões inteira, como o app da Akko (01, 09 x2, 02).
    pub fn write_buttons(&self, b: &Buttons) -> Result<(), Error> {
        self.write_seq(0x09, &b.0)
    }

    /// Grava o macro (o mouse guarda um só; os botões o chamam com 70 00 00).
    pub fn write_macro(&self, data: &[u8]) -> Result<(), Error> {
        if data.len() > 0xff {
            return Err(Error::Unknown);
        }
        self.write_seq(0x15, data)
    }

    /// 01, blocos de até 0x18 bytes com o comando `cmd` (conferindo o eco), 02.
    fn write_seq(&self, cmd: u8, data: &[u8]) -> Result<(), Error> {
        self.request(0x01, 0, 0, &[])?;
        let mut result = Ok(());
        for (k, chunk) in data.chunks(0x18).enumerate() {
            match self.request(cmd, chunk.len() as u8, (k * 0x18) as u8, chunk) {
                Ok(r) if &r[8..8 + chunk.len()] == chunk => {}
                Ok(_) => {
                    result = Err(Error::Unknown);
                    break;
                }
                Err(e) => {
                    result = Err(e);
                    break;
                }
            }
        }
        let _ = self.request(0x02, 0, 0, &[]);
        result
    }

    /// Resposta da bateria (bytes 8..16): o byte 0 é a porcentagem.
    pub fn battery_raw(&self) -> Result<[u8; 8], Error> {
        let r = self.request(0x1a, 6, 0, &[])?;
        let mut b = [0u8; 8];
        b.copy_from_slice(&r[8..16]);
        Ok(b)
    }
}

/* ---------------- botões e macro ---------------- */

pub const BTN_LEN: usize = 0x24;
/// Botões configuráveis: esquerdo, direito, meio, voltar, avançar (ordem da tabela).
pub const BUTTONS: usize = 5;
/// Códigos de 3 bytes: [tipo] [valor] [valor]. 10 = botão do mouse (máscara), 13 = DPI
/// (01 +, 02 -, 03 ciclo), 20 = teclado (modificadores, tecla HID), 30 = multimídia (uso
/// "consumer" de 16 bits), 70 = macro.
pub const DEFAULT_BUTTONS: [[u8; 3]; BUTTONS] = [[0x10, 0x01, 0], [0x10, 0x02, 0], [0x10, 0x04, 0], [0x10, 0x08, 0], [0x10, 0x10, 0]];
pub const MACRO_CODE: [u8; 3] = [0x70, 0, 0];

#[derive(Clone, PartialEq, Eq)]
pub struct Buttons(pub [u8; BTN_LEN]);

impl Buttons {
    pub fn get(&self, i: usize) -> [u8; 3] {
        [self.0[i * 3], self.0[i * 3 + 1], self.0[i * 3 + 2]]
    }
    pub fn set(&mut self, i: usize, c: [u8; 3]) {
        self.0[i * 3..i * 3 + 3].copy_from_slice(&c);
    }
}

/// Um evento do macro: tecla (uso HID) pressionada/solta e o tempo até o próximo evento.
#[derive(Clone, Copy, Debug)]
pub struct MacroKey {
    pub down: bool,
    pub key: u8,
    pub delay_ms: u16,
}

pub const MACRO_MAX_EVENTS: usize = 50;

/// Formato gravado pelo app da Akko (dois macros capturados):
///   aa 55 [tamanho total u16] 01 00 00 00 [8 bytes sem uso] 12 00 [n eventos u16] [4 bytes sem uso]
///   e os eventos: [tipo|0x80 se pressionada] [tecla] e o tempo até o próximo (u16), sem tempo
///   depois do último. Tipo 0x0a = teclado.
pub fn macro_bytes(events: &[MacroKey]) -> Vec<u8> {
    let events = &events[..events.len().min(MACRO_MAX_EVENTS)];
    let n = events.len();
    let mut out = vec![0xaa, 0x55, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x12, 0];
    out.extend_from_slice(&(n as u16).to_le_bytes());
    out.extend_from_slice(&[0, 0, 0, 0]);
    for (i, e) in events.iter().enumerate() {
        out.push(0x0a | if e.down { 0x80 } else { 0 });
        out.push(e.key);
        if i + 1 < n {
            out.extend_from_slice(&e.delay_ms.to_le_bytes());
        }
    }
    let total = out.len() as u16;
    out[2..4].copy_from_slice(&total.to_le_bytes());
    out
}

/// Escuta os avisos que o mouse manda sozinho (interface "col03": troca de DPI, etc.) e chama
/// `on_report` para cada um. Só lê: não manda nada ao mouse (não mexe o cursor). Reabre sozinho
/// quando o mouse troca entre o cabo e o receptor. Não retorna.
pub fn listen_notifications(on_report: impl Fn(&[u8])) -> ! {
    loop {
        if let Some(path) = find_hid_path("&col03", false) {
            unsafe {
                let h = CreateFileW(path.as_ptr(), GENERIC_READ, FILE_SHARE_READ | FILE_SHARE_WRITE, null(), OPEN_EXISTING, 0, null_mut());
                if h != INVALID_HANDLE_VALUE {
                    let mut buf = [0u8; 64];
                    loop {
                        let mut n = 0u32;
                        if ReadFile(h, buf.as_mut_ptr(), buf.len() as u32, &mut n, null_mut()) == 0 {
                            break;
                        }
                        on_report(&buf[..n as usize]);
                    }
                    CloseHandle(h);
                }
            }
        }
        std::thread::sleep(Duration::from_secs(3));
    }
}

pub fn wide_str(s: &str) -> Vec<u16> {
    wide(s)
}
