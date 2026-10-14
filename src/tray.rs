//! Processo da bandeja: ícone com a bateria, menu, comandos da janela de configuração (via
//! WM_COPYDATA) e a thread que fala com o mouse (a única que fala com ele).

use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::DataExchange::COPYDATASTRUCT;
use windows_sys::Win32::System::Diagnostics::ToolHelp::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::HiDpi::*;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
use windows_sys::Win32::UI::Shell::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::mouse::{macro_bytes, polling_code, wide_str as wide, Buttons, Device, Mem, BUTTONS, DEFAULT_BUTTONS, MACRO_CODE, STAGES};
use crate::i18n::t;
use crate::state::{Comando, Estado, Estagio, Prefs};
use crate::{autostart, games, i18n, osd};

pub const CLASS: &str = "AkkoEchoUltraBandeja";
const WM_TRAY: u32 = WM_APP + 1;
const WM_STATE: u32 = WM_APP + 2;
pub const WM_OPEN_UI: u32 = WM_APP + 3;
const ID_OPEN: usize = 1;
const ID_AUTOSTART: usize = 2;
const ID_EXIT: usize = 3;

static HWND_TRAY: AtomicUsize = AtomicUsize::new(0);
static PREFS: Mutex<Option<Prefs>> = Mutex::new(None);
static ESTADO: Mutex<Option<Estado>> = Mutex::new(None);
/// Tabela dos botões lida do mouse (publicada junto com o estado).
static BOTOES: Mutex<Option<Buttons>> = Mutex::new(None);
/// O mouse mandou um aviso (col03): lê a bateria já (ex.: o cabo entrou ou saiu).
static BATTERY_NOW: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// Mouse carregando (no cabo): ícone verde com raio e bateria lida a cada 15 s.
static CHARGING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static DEV_TX: OnceLock<Sender<Comando>> = OnceLock::new();
static TASKBAR_CREATED: AtomicUsize = AtomicUsize::new(0);

fn prefs() -> Prefs {
    PREFS.lock().unwrap().clone().unwrap_or_default()
}

pub fn find_existing() -> Option<HWND> {
    let h = unsafe { FindWindowW(wide(CLASS).as_ptr(), null()) };
    (!h.is_null()).then_some(h)
}

pub fn post_open_ui(h: HWND) {
    unsafe { PostMessageW(h, WM_OPEN_UI, 0, 0) };
}

fn spawn_ui() {
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::process::Command::new(exe).arg("--ui").spawn();
    }
}

/* ---------------- estado publicado ---------------- */

fn publish(mem: Option<&Mem>, status: &str, battery: i32) {
    let p = prefs();
    let mut e = ESTADO.lock().unwrap();
    let versao = e.as_ref().map_or(0, |e| e.versao) + 1;
    let mut est = Estado {
        versao,
        status: status.into(),
        bateria: battery,
        carregando: CHARGING.load(Ordering::Relaxed),
        prefs: p,
        autostart: autostart::is_enabled(),
        akko_autostart: autostart::akko_enabled(),
        ..Default::default()
    };
    if let Some(m) = mem {
        est.carregado = true;
        est.ativo = m.active();
        est.estagios = (0..STAGES).map(|i| Estagio { dpi: m.dpi(i), cor: m.color(i) }).collect();
        est.polling_hz = m.polling_hz();
        est.lod = m.lod();
        est.debounce = m.debounce();
        est.suspensao_s = m.sleep_s();
        est.angle_snap = m.angle_snap();
        est.ripple = m.ripple();
        est.motion_sync = m.motion_sync();
        if let Some(b) = BOTOES.lock().unwrap().as_ref() {
            est.botoes = (0..BUTTONS).map(|i| b.get(i)).collect();
        }
    }
    est.save();
    *e = Some(est);
    drop(e);
    let h = HWND_TRAY.load(Ordering::Relaxed);
    if h != 0 {
        unsafe { PostMessageW(h as HWND, WM_STATE, 0, 0) };
    }
}

/* ---------------- thread do mouse ---------------- */

fn akko_running() -> bool {
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap == INVALID_HANDLE_VALUE {
            return false;
        }
        let mut pe: PROCESSENTRY32W = std::mem::zeroed();
        pe.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut found = false;
        let mut ok = Process32FirstW(snap, &mut pe);
        while ok != 0 {
            let len = pe.szExeFile.iter().position(|&c| c == 0).unwrap_or(0);
            if String::from_utf16_lossy(&pe.szExeFile[..len]).eq_ignore_ascii_case("Akko.exe") {
                found = true;
                break;
            }
            ok = Process32NextW(snap, &mut pe);
        }
        CloseHandle(snap);
        found
    }
}

fn chord_down() -> bool {
    unsafe {
        (GetAsyncKeyState(VK_LBUTTON as i32) as u16 & 0x8000) != 0
            && (GetAsyncKeyState(VK_MBUTTON as i32) as u16 & 0x8000) != 0
            && (GetAsyncKeyState(VK_RBUTTON as i32) as u16 & 0x8000) != 0
    }
}

struct Worker {
    dev: Option<Device>,
    mem: Option<Mem>,
    /// Última memória conhecida (sobrevive a desconexões; serve para registrar o que mudou).
    last_known: Option<Mem>,
    battery: i32,
    status: &'static str,
}

fn update_prefs(f: impl FnOnce(&mut Prefs)) {
    let mut g = PREFS.lock().unwrap();
    let p = g.get_or_insert_with(Prefs::default);
    f(p);
    p.save();
}

/// Registra a memória (memory.txt) e, se mudou desde a última vez, o que mudou (no log).
fn record_mem(prev: Option<&Mem>, m: &Mem, origem: &str) {
    crate::state::write_atomic("memory.txt", &format!("{origem}\n{}", m.hex()));
    if let Some(p) = prev {
        let diffs: Vec<String> = (0..m.0.len())
            .filter(|&i| p.0[i] != m.0[i])
            .map(|i| format!("0x{i:02x}: {:02x}->{:02x}", p.0[i], m.0[i]))
            .collect();
        if !diffs.is_empty() {
            crate::log(&format!("memoria mudou ({origem}): {}", diffs.join(", ")));
        }
    }
}

/// Estágios válidos (o firmware às vezes zera um, ao entrar em 8000 Hz).
fn stages_ok(m: &Mem) -> bool {
    (0..STAGES).all(|i| m.dpi(i) >= 50)
}

impl Worker {
    /// Aplica uma mudança: grava a memória inteira (normalizada) e publica.
    fn change(&mut self, f: impl FnOnce(&mut Mem, &Prefs)) -> bool {
        let (Some(dev), Some(old)) = (&self.dev, &self.mem) else { return false };
        let p = prefs();
        let mut new = old.clone();
        f(&mut new, &p);
        new.normalize(p.troca_dpi);
        if &new == old {
            return true;
        }
        match dev.apply(old, &new) {
            Ok(()) => {
                record_mem(self.last_known.as_ref(), &new, "gravada");
                if stages_ok(&new) {
                    let st: Vec<Estagio> = (0..STAGES).map(|i| Estagio { dpi: new.dpi(i), cor: new.color(i) }).collect();
                    if prefs().estagios_salvos != st {
                        update_prefs(|p| p.estagios_salvos = st);
                    }
                }
                self.last_known = Some(new.clone());
                self.mem = Some(new);
                publish(self.mem.as_ref(), self.status, self.battery);
                true
            }
            Err(e) => {
                crate::log(&format!("falha ao gravar no mouse: {e:?}; relendo"));
                self.mem = None;
                false
            }
        }
    }

    /// Ativa um estágio (o polling é o mesmo para todos, como no app da Akko). Mostra o aviso.
    /// Ativa um estágio e o polling dele (se tiver). Mostra o aviso.
    fn activate(&mut self, i: usize) {
        let Some(old) = self.mem.clone() else { return };
        let p = prefs();
        let own = p.polling_por_estagio.get(i).copied().filter(|&hz| polling_code(hz).is_some());
        let hz = own.unwrap_or(old.polling_hz());
        let ok = self.change(|m, _| {
            m.set_active(i);
            if let Some(code) = own.and_then(polling_code) {
                m.set_polling(code);
            }
        });
        // Entrando em 8000 Hz o receptor pode parar de responder no meio da gravação, mas o
        // estágio já foi gravado antes (ver Device::apply): o aviso aparece do mesmo jeito.
        if (ok || hz == 8000) && p.aviso {
            let txt = t("stage_of").replacen("{}", &(i + 1).to_string(), 1).replacen("{}", &STAGES.to_string(), 1);
            osd::show(format!("{} DPI", old.dpi(i)), format!("{txt} \u{b7} {hz} Hz"), old.color(i), p.aviso_ms);
        }
    }

    fn handle(&mut self, c: Comando) {
        match c {
            Comando::Ativar { i } if i < STAGES => self.activate(i),
            Comando::Dpi { i, dpi } if i < STAGES => {
                self.change(|m, _| m.set_dpi(i, dpi.clamp(100, 40000)));
            }
            Comando::Cor { i, rgb } if i < STAGES => {
                self.change(|m, _| m.set_color(i, rgb));
            }
            Comando::Polling { hz } => {
                if let Some(code) = polling_code(hz) {
                    self.change(|m, _| m.set_polling(code));
                }
            }
            Comando::PollingEstagio { i, hz } if i < STAGES => {
                if let Some(code) = polling_code(hz) {
                    update_prefs(|p| p.polling_por_estagio[i] = hz);
                    if self.mem.as_ref().map_or(false, |m| m.active() == i) {
                        self.change(|m, _| m.set_polling(code));
                    }
                    publish(self.mem.as_ref(), self.status, self.battery);
                }
            }
            Comando::Lod { v } if v <= 2 => {
                self.change(|m, _| m.set_lod(v));
            }
            Comando::Debounce { v } if v <= 6 => {
                self.change(|m, _| m.set_debounce(v));
            }
            Comando::Suspensao { s } => {
                self.change(|m, _| m.set_sleep_s(s.clamp(60, 1800)));
            }
            Comando::AngleSnap { on } => {
                self.change(|m, _| m.set_angle_snap(on));
            }
            Comando::Ripple { on } => {
                self.change(|m, _| m.set_ripple(on));
            }
            Comando::MotionSync { on } => {
                self.change(|m, _| m.set_motion_sync(on));
            }
            Comando::Botao { i, codigo } if i < BUTTONS && codigo != MACRO_CODE => {
                self.buttons(|b| b.set(i, codigo));
            }
            Comando::BotaoMacro { i, nome } if i < BUTTONS => self.assign_macro(i, &nome),
            Comando::BotoesPadrao => {
                self.buttons(|b| (0..BUTTONS).for_each(|i| b.set(i, DEFAULT_BUTTONS[i])));
            }
            Comando::TrocarEsqDir => {
                self.buttons(|b| {
                    let (l, r) = (b.get(0), b.get(1));
                    b.set(0, r);
                    b.set(1, l);
                });
            }
            Comando::RegravarMacro => {
                if let Some(nome) = prefs().macro_no_mouse {
                    self.write_macro(&nome);
                }
            }
            _ => {}
        }
    }

    /// Muda a tabela dos botões e grava no mouse.
    fn buttons(&mut self, f: impl FnOnce(&mut Buttons)) -> bool {
        let Some(dev) = &self.dev else { return false };
        let Some(old) = BOTOES.lock().unwrap().clone() else { return false };
        let mut new = old.clone();
        f(&mut new);
        if new == old {
            return true;
        }
        match dev.write_buttons(&new) {
            Ok(()) => {
                crate::log(&format!("botoes gravados: {}", (0..BUTTONS).map(|i| hex3(new.get(i))).collect::<Vec<_>>().join(" ")));
                *BOTOES.lock().unwrap() = Some(new);
                publish(self.mem.as_ref(), self.status, self.battery);
                true
            }
            Err(e) => {
                crate::log(&format!("falha ao gravar os botoes: {e:?}"));
                self.mem = None; // relê tudo
                false
            }
        }
    }

    fn write_macro(&mut self, nome: &str) -> bool {
        let Some(dev) = &self.dev else { return false };
        let Some(m) = prefs().macros.into_iter().find(|m| m.nome == nome) else { return false };
        if m.eventos.is_empty() {
            return false;
        }
        match dev.write_macro(&macro_bytes(&m.keys())) {
            Ok(()) => {
                crate::log(&format!("macro '{nome}' gravado ({} eventos)", m.eventos.len()));
                update_prefs(|p| p.macro_no_mouse = Some(nome.to_string()));
                republish();
                true
            }
            Err(e) => {
                crate::log(&format!("falha ao gravar o macro: {e:?}"));
                false
            }
        }
    }

    /// O mouse guarda um macro só: grava o escolhido e o coloca no botão `i`; outros botões que
    /// tinham macro voltam ao padrão.
    fn assign_macro(&mut self, i: usize, nome: &str) {
        if !self.write_macro(nome) {
            return;
        }
        self.buttons(|b| {
            for j in 0..BUTTONS {
                if j != i && b.get(j) == MACRO_CODE {
                    b.set(j, DEFAULT_BUTTONS[j]);
                }
            }
            b.set(i, MACRO_CODE);
        });
    }
}

fn hex3(c: [u8; 3]) -> String {
    format!("{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

/// Luz como o app da Akko deixa: modo fixo, cor branca e "multicor" (o firmware acende o LED
/// na cor do estágio ao trocar de DPI).
fn restore_light(w: &mut Worker) {
    w.change(|m, _| default_light(m));
}

fn default_light(m: &mut Mem) {
    // bytes 0x00..0x09 como estavam na memória original (backup de 2026-10-10)
    for (a, v) in [(0x00, 0x00), (0x02, 0x03), (0x03, 0x00), (0x04, 0x00), (0x09, 0x00)] {
        m.set_byte(a, v);
    }
    m.set_light_mode(3);
    m.set_light_multi(true);
    m.set_light_rgb([0xff, 0xff, 0xff]);
}

/// Bytes de função desconhecida que vêm de fábrica com valor e se perderam numa corrupção da
/// memória (0x64 = 0x1e no backup original de 2026-10-10; virou 0x00). Devolve o de fábrica.
fn restore_factory_bytes(w: &mut Worker) {
    const FACTORY: [(usize, u8); 1] = [(0x64, 0x1e)];
    let Some(m) = &w.mem else { return };
    let lost: Vec<(usize, u8)> = FACTORY.iter().copied().filter(|&(a, _)| m.0[a] == 0).collect();
    if lost.is_empty() {
        return;
    }
    crate::log(&format!("restaurando bytes de fabrica: {lost:02x?}"));
    w.change(|m, _| lost.iter().for_each(|&(a, v)| m.set_byte(a, v)));
}

/// Se o firmware zerou algum estágio (acontece ao entrar em 8000 Hz), devolve os últimos bons.
fn restore_stages(w: &mut Worker) {
    let saved = prefs().estagios_salvos;
    let Some(m) = &w.mem else { return };
    if stages_ok(m) || saved.len() != STAGES || saved.iter().any(|s| s.dpi < 50) {
        return;
    }
    let bad: Vec<usize> = (0..STAGES).filter(|&i| m.dpi(i) < 50).collect();
    crate::log(&format!("estagios zerados {bad:?}: restaurando os ultimos bons"));
    w.change(|m, _| {
        for &i in &bad {
            m.set_dpi(i, saved[i].dpi);
            m.set_color(i, saved[i].cor);
        }
    });
}

fn device_thread(rx: Receiver<Comando>) {
    let mut w = Worker { dev: None, mem: None, last_known: None, battery: -1, status: "procurando" };
    let mut fails = 0u32;
    let mut last_connect = Instant::now() - Duration::from_secs(60);
    let mut last_akko_check = Instant::now() - Duration::from_secs(60);
    let mut last_battery: Option<Instant> = None;
    let mut last_battery_raw: Option<[u8; 8]> = None;
    let mut battery_streak = 0i32;
    // Tempo de uso (segundos com mouse/teclado mexendo) desde que saiu do cabo, para o registro
    // da bateria.
    let mut use_secs = 0u64;
    let mut last_use_check = Instant::now();
    let mut akko = false;
    let mut chord = false;
    let mut last_game_check = Instant::now();
    // Jogo em primeiro plano agora: (executável, estágio de antes do jogo).
    let mut game: Option<(String, usize)> = None;
    publish(None, w.status, w.battery);

    loop {
        std::thread::sleep(Duration::from_millis(30));
        let now = Instant::now();

        // Com o Akko aberto, não fala com o mouse (os dois juntos corrompem a memória).
        if now - last_akko_check >= Duration::from_secs(2) {
            last_akko_check = now;
            let running = akko_running();
            if running != akko {
                akko = running;
                if akko {
                    w.dev = None;
                    w.mem = None;
                    w.status = "pausado";
                    publish(None, w.status, w.battery);
                    crate::log("Akko aberto: comunicação pausada");
                } else {
                    w.status = "procurando";
                    last_connect = now - Duration::from_secs(60);
                    publish(None, w.status, w.battery);
                }
            }
        }

        let mut cmds = Vec::new();
        while let Ok(c) = rx.try_recv() {
            cmds.push(c);
        }
        if akko {
            continue;
        }

        // Conecta e lê a memória (o mouse pode estar dormindo ou desligado).
        if w.mem.is_none() {
            // Depois de várias falhas seguidas (ex.: receptor travado em 8 kHz), tenta menos vezes:
            // cada pedido mexe o cursor ~1 px.
            let wait = if fails >= 5 { 15 } else { 3 };
            if now - last_connect < Duration::from_secs(wait) {
                continue;
            }
            last_connect = now;
            dbg("mouse: abrindo");
            if w.dev.is_none() {
                w.dev = Device::open();
            }
            dbg(if w.dev.is_some() { "mouse: aberto, lendo memoria" } else { "mouse: nao encontrado" });
            match w.dev.as_ref().map(|d| d.read_mem()) {
                Some(Ok(m)) => {
                    fails = 0;
                    record_mem(w.last_known.as_ref(), &m, "lida");
                    w.last_known = Some(m.clone());
                    w.mem = Some(m);
                    w.status = "conectado";
                    last_battery = None;
                    publish(w.mem.as_ref(), w.status, w.battery);
                    crate::log("mouse conectado");
                    let b = w.dev.as_ref().and_then(|d| d.read_buttons().ok());
                    if b.is_none() {
                        crate::log("nao consegui ler a tabela dos botoes");
                    }
                    *BOTOES.lock().unwrap() = b;
                    publish(w.mem.as_ref(), w.status, w.battery);
                    restore_stages(&mut w);
                    // Os testes de iluminação (removidos) mexiam na luz: garante a luz padrão
                    // (não grava nada se já estiver assim).
                    restore_light(&mut w);
                    restore_factory_bytes(&mut w);
                    // Estágios ainda sem polling próprio ficam com o polling atual do mouse.
                    if let Some(hz) = w.mem.as_ref().map(|m| m.polling_hz()) {
                        if prefs().polling_por_estagio.iter().any(|&h| polling_code(h).is_none()) {
                            update_prefs(|p| p.polling_por_estagio.iter_mut().filter(|h| polling_code(**h).is_none()).for_each(|h| *h = hz));
                        }
                    }
                }
                _ => {
                    fails += 1;
                    w.dev = None;
                    if w.status != "procurando" {
                        w.status = "procurando";
                        publish(None, w.status, w.battery);
                    }
                    continue;
                }
            }
        }

        for c in cmds {
            w.handle(c);
        }

        // Troca de DPI própria: no instante em que esquerdo + meio + direito ficam pressionados.
        let down = chord_down();
        if down && !chord && prefs().troca_dpi {
            if let Some(m) = &w.mem {
                let next = (m.active() + 1) % STAGES;
                w.activate(next);
            }
        }
        chord = down;

        // DPI por jogo: com um jogo da lista em primeiro plano, ativa o estágio dele; ao sair dele
        // (fechar ou Alt+Tab), volta ao estágio de antes. Só consulta o Windows, não o mouse.
        if now - last_game_check >= Duration::from_secs(1) {
            last_game_check = now;
            let p = prefs();
            let rule = games::foreground_exe().and_then(|exe| p.jogos.iter().find(|j| j.exe == exe).cloned());
            match (rule, game.clone()) {
                (Some(r), cur) if cur.as_ref().map_or(true, |(e, _)| *e != r.exe) => {
                    if let Some(active) = w.mem.as_ref().map(|m| m.active()) {
                        let target = r.estagio.min(STAGES - 1);
                        let prev = cur.map_or(active, |(_, p)| p);
                        crate::log(&format!("jogo em primeiro plano: {} -> estagio {}", r.exe, target + 1));
                        if active != target {
                            w.activate(target);
                        }
                        game = Some((r.exe, prev));
                    }
                }
                (None, Some((exe, prev))) => {
                    crate::log(&format!("saiu do jogo {exe}"));
                    if p.jogo_voltar && w.mem.as_ref().map_or(false, |m| m.active() != prev) {
                        w.activate(prev);
                    }
                    game = None;
                }
                _ => {}
            }
        }

        // Bateria: a cada minuto (2 s no cabo, 5 s carregando). Enquanto
        // não houver leitura (ex.: logo depois de ligar o PC, com o mouse ainda acordando), tenta
        // de novo a cada 10 s.
        let period = if w.battery < 0 {
            10
        } else if w.dev.as_ref().map_or(false, |d| d.wired) {
            // no cabo (sem o receptor no meio, o pedido não mexe o cursor): quase em tempo real
            2
        } else if CHARGING.load(Ordering::Relaxed) {
            5
        } else if input_idle() >= Duration::from_secs(300) {
            // PC sem uso há mais de 5 min: a cada 5 min (o mouse pode estar dormindo)
            300
        } else {
            // a cada minuto (o cabo pode entrar sem nenhum aviso do mouse)
            60
        };
        // aviso do mouse: lê já (no máximo uma vez a cada 2 s)
        if BATTERY_NOW.swap(false, Ordering::Relaxed) && last_battery.map_or(true, |t| now - t >= Duration::from_secs(2)) {
            last_battery = None;
        }
        // A leitura não mexe o cursor (medido com Raw Input em 1000 e 8000 Hz): lê também em jogo.
        let due = last_battery.map_or(true, |t| now - t >= Duration::from_secs(period));
        if now - last_use_check >= Duration::from_secs(1) {
            last_use_check = now;
            if CHARGING.load(Ordering::Relaxed) || w.dev.as_ref().map_or(false, |d| d.wired) {
                use_secs = 0;
            } else if input_idle() < Duration::from_secs(2) {
                use_secs += 1;
            }
        }
        if due {
            last_battery = Some(now);
            if let Some(d) = &w.dev {
                match d.battery_raw() {
                    Ok(raw) => {
                        // Registra a resposta inteira quando muda; sem cabo, registra toda leitura com o
                        // tempo de uso (para ver quando sai de 100% e quanto dura).
                        let hex = raw.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ");
                        if !d.wired {
                            crate::log(&format!("bateria: {}% (resposta {hex}), uso sem cabo: {} min", raw[0], use_secs / 60));
                        } else if last_battery_raw != Some(raw) {
                            crate::log(&format!("bateria (resposta): {hex}"));
                        }
                        last_battery_raw = Some(raw);
                        // byte 0 = porcentagem; byte 1 = 1 carregando, 2 carga completa (o mouse
                        // continua dizendo 2 um tempo depois de sair do cabo: só vale no cabo)
                        let b = raw[0] as i32;
                        let full = raw[1] == 2 && d.wired;
                        let charging = raw[1] == 1 || full;
                        let was = CHARGING.swap(charging, Ordering::Relaxed);
                        if b <= 100 {
                            let shown = smooth_battery(w.battery, b, charging, full, &mut battery_streak);
                            if shown != w.battery || was != charging {
                                w.battery = shown;
                                publish(w.mem.as_ref(), w.status, w.battery);
                            }
                        }
                    }
                    Err(e) => {
                        // Ex.: o cabo saiu e o mouse voltou para o receptor (outro aparelho USB):
                        // reconecta já e lê a bateria de novo.
                        crate::log(&format!("bateria sem resposta ({e:?}): reconectando"));
                        CHARGING.store(false, Ordering::Relaxed);
                        w.dev = None;
                        w.mem = None;
                        last_connect = now - Duration::from_secs(60);
                    }
                }
            }
        }
    }
}

/// Tempo desde a última entrada (mouse ou teclado) no Windows.
fn input_idle() -> Duration {
    unsafe {
        let mut li = LASTINPUTINFO { cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32, dwTime: 0 };
        if GetLastInputInfo(&mut li) == 0 {
            return Duration::ZERO;
        }
        let ms = windows_sys::Win32::System::SystemInformation::GetTickCount().wrapping_sub(li.dwTime);
        Duration::from_millis(ms as u64)
    }
}

/// O mouse estima a carga pela tensão: carregando, ela sobe rápido e para em 99%; ao sair do cabo
/// volta a 100%; em uso, oscila. Para o número não ir e voltar: carregando, só sobe; sem cabo, só
/// desce, e só depois de duas leituras seguidas mais baixas (sobe só com duas leituras seguidas 5
/// pontos acima: carregou sem o programa ver, ex.: com o PC desligado).
fn smooth_battery(shown: i32, read: i32, charging: bool, full: bool, streak: &mut i32) -> i32 {
    if shown < 0 || full {
        *streak = 0;
        return if full { 100 } else { read };
    }
    if charging {
        *streak = 0;
        return shown.max(read);
    }
    let dir = if read < shown { -1 } else if read >= shown + 5 { 1 } else { 0 };
    if dir == 0 {
        *streak = 0;
        shown
    } else if *streak == dir {
        *streak = 0;
        read
    } else {
        *streak = dir;
        shown
    }
}

/* ---------------- ícone da bandeja ---------------- */

/// Só para conferir o desenho: `--icone-png <arquivo> <bateria>` salva o ícone ampliado 8x.
static ICON_DUMP: Mutex<Option<String>> = Mutex::new(None);

pub fn dump_icon(path: &str, battery: i32, charging: bool) {
    *ICON_DUMP.lock().unwrap() = Some(path.to_string());
    CHARGING.store(charging, Ordering::Relaxed);
    unsafe { DestroyIcon(battery_icon(battery)) };
}

fn dump_png(path: &str, bgra: &[u8], s: usize) {
    const Z: usize = 8;
    let n = s * Z;
    let mut rgb = vec![0u8; n * n * 3];
    for y in 0..n {
        for x in 0..n {
            let i = ((y / Z) * s + x / Z) * 4;
            let a = bgra[i + 3] as u32;
            let bg = if (x / 16 + y / 16) % 2 == 0 { 40u32 } else { 60 };
            for (c, src) in [(0, 2), (1, 1), (2, 0)] {
                rgb[(y * n + x) * 3 + c] = ((bgra[i + src] as u32 * a + bg * (255 - a)) / 255) as u8;
            }
        }
    }
    if let Ok(f) = std::fs::File::create(path) {
        let mut enc = png::Encoder::new(std::io::BufWriter::new(f), n as u32, n as u32);
        enc.set_color(png::ColorType::Rgb);
        if let Ok(mut w) = enc.write_header() {
            let _ = w.write_image_data(&rgb);
        }
    }
}

/// Ícone 32x32: mouse branco deitado (frente e rodinha à esquerda) com a porcentagem da bateria.
unsafe fn battery_icon(battery: i32) -> HICON {
    const S: i32 = 32;
    let text = if battery >= 0 { battery.to_string() } else { "--".into() };
    let charging = CHARGING.load(Ordering::Relaxed);
    // Carregando: raio dentro do mouse, ao lado do número (com 100%, números mais estreitos).
    let bolt_on = charging;
    // O texto é desenhado com GDI (máscara branca sobre preto); o mouse é calculado com
    // supersampling para as bordas ficarem suaves.
    let render = |draw: &dyn Fn(HDC)| -> Vec<u8> {
        let mut bi: BITMAPINFO = std::mem::zeroed();
        bi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bi.bmiHeader.biWidth = S;
        bi.bmiHeader.biHeight = -S;
        bi.bmiHeader.biPlanes = 1;
        bi.bmiHeader.biBitCount = 32;
        let mut bits: *mut core::ffi::c_void = null_mut();
        let dc = CreateCompatibleDC(null_mut());
        let bmp = CreateDIBSection(dc, &bi, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
        let old = SelectObject(dc, bmp);
        draw(dc);
        GdiFlush();
        let px = std::slice::from_raw_parts(bits as *const u8, (S * S * 4) as usize).to_vec();
        SelectObject(dc, old);
        DeleteObject(bmp);
        DeleteDC(dc);
        (0..(S * S) as usize).map(|i| px[i * 4 + 1]).collect()
    };
    // Mouse visto de lado (frente à esquerda): base reta, corpo que sobe da frente até o
    // ponto mais alto e desce arredondado atrás; a rodinha aparece em cima, na frente.
    let top = |x: f32| -> f32 {
        let t = x / 32.0;
        // frente baixa subindo até o ponto mais alto e traseira arredondada (cabe número grande)
        let h = if t < 0.55 {
            0.4 + 0.6 * (std::f32::consts::FRAC_PI_2 * t / 0.55).sin()
        } else {
            (1.0 - ((t - 0.55) / 0.45).powi(2)).max(0.0).powf(0.35)
        };
        31.5 - 26.5 * h
    };
    let shell = |x: f32, y: f32| {
        if !(0.0..32.0).contains(&x) || y > 31.5 || y < top(x) {
            return false;
        }
        // nariz (frente) arredondado embaixo
        let r = 5.0;
        let (cx, cy) = (r, 31.5 - r);
        if x < cx && y > cy && (x - cx).powi(2) + (y - cy).powi(2) > r * r {
            return false;
        }
        true
    };
    // rodinha: marca escura em cima, na frente
    let (wx, wy) = (5.0, top(5.0) + 1.6);
    let inside = shell;
    let wheel_mark = move |x: f32, y: f32| ((x - wx) / 2.0).powi(2) + ((y - wy) / 1.1).powi(2) <= 1.0;
    // Raio (carregando): escuro, dentro do corpo do mouse, à esquerda da porcentagem.
    const BOLT: [(f32, f32); 7] = [(9.6, 9.0), (2.6, 20.0), (6.4, 20.0), (4.4, 31.0), (12.4, 17.6), (8.6, 17.6), (12.0, 9.0)];
    let in_bolt = |x: f32, y: f32| {
        let mut inside = false;
        let mut j = BOLT.len() - 1;
        for i in 0..BOLT.len() {
            let (xi, yi) = BOLT[i];
            let (xj, yj) = BOLT[j];
            if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
                inside = !inside;
            }
            j = i;
        }
        inside
    };
    let detail = move |x: f32, y: f32| if bolt_on { in_bolt(x, y) } else { wheel_mark(x, y) };
    let mut body = vec![0u8; (S * S) as usize];
    let mut marks = vec![0u8; (S * S) as usize];
    const N: i32 = 4;
    for py in 0..S {
        for px in 0..S {
            let (mut b, mut m) = (0, 0);
            for sy in 0..N {
                for sx in 0..N {
                    let x = px as f32 + (sx as f32 + 0.5) / N as f32;
                    let y = py as f32 + (sy as f32 + 0.5) / N as f32;
                    if inside(x, y) {
                        b += 1;
                        if detail(x, y) {
                            m += 1;
                        }
                    }
                }
            }
            body[(py * S + px) as usize] = (b * 255 / (N * N)) as u8;
            marks[(py * S + px) as usize] = (m * 255 / (N * N)) as u8;
        }
    }
    let glyphs = render(&|dc| {
        let face = wide("Segoe UI");
        // (altura, largura média; 0 = normal). "100" carregando usa números estreitos.
        let (size, width) = match (bolt_on, text.len() >= 3) {
            (true, false) => (20, 0),
            (true, true) => (18, 6),
            (false, false) => (24, 0),
            (false, true) => (17, 0),
        };
        let f = CreateFontW(-size, width, 0, 0, FW_BOLD as i32, 0, 0, 0, DEFAULT_CHARSET as u32, 0, 0, ANTIALIASED_QUALITY as u32, 0, face.as_ptr());
        let of = SelectObject(dc, f);
        SetBkMode(dc, TRANSPARENT as i32);
        SetTextColor(dc, 0xFFFFFF);
        let mut r = RECT { left: if bolt_on { 12 } else { 3 }, top: 8, right: S - 1, bottom: 31 };
        let mut w = wide(&text);
        DrawTextW(dc, w.as_mut_ptr(), (w.len() - 1) as i32, &mut r, DT_CENTER | DT_SINGLELINE | DT_VCENTER);
        SelectObject(dc, of);
        DeleteObject(f);
    });
    let low = (0..=15).contains(&battery) && !charging;
    let mut bgra = vec![0u8; (S * S * 4) as usize];
    for i in 0..(S * S) as usize {
        let a = body[i] as u32;
        let t = (glyphs[i] as u32).max(marks[i] as u32);
        // corpo verde carregando, vermelho com bateria baixa, senão branco; texto e raio escuros
        let (br, bg, bb) = if charging {
            (90u32, 210u32, 110u32)
        } else if low {
            (235, 60, 60)
        } else {
            (255, 255, 255)
        };
        let mix = |c: u32| (c * (255 - t) + 20 * t) / 255;
        bgra[i * 4] = mix(bb) as u8;
        bgra[i * 4 + 1] = mix(bg) as u8;
        bgra[i * 4 + 2] = mix(br) as u8;
        bgra[i * 4 + 3] = a as u8;
    }
    if let Some(path) = ICON_DUMP.lock().unwrap().take() {
        dump_png(&path, &bgra, S as usize);
    }
    let mut bi: BITMAPINFO = std::mem::zeroed();
    bi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
    bi.bmiHeader.biWidth = S;
    bi.bmiHeader.biHeight = -S;
    bi.bmiHeader.biPlanes = 1;
    bi.bmiHeader.biBitCount = 32;
    let mut bits: *mut core::ffi::c_void = null_mut();
    let color = CreateDIBSection(null_mut(), &bi, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
    std::ptr::copy_nonoverlapping(bgra.as_ptr(), bits as *mut u8, bgra.len());
    let mask = CreateBitmap(S, S, 1, 1, null());
    let ii = ICONINFO { fIcon: 1, xHotspot: 0, yHotspot: 0, hbmMask: mask, hbmColor: color };
    let icon = CreateIconIndirect(&ii);
    DeleteObject(color);
    DeleteObject(mask);
    icon
}

/* ---------------- janela e ícone da bandeja ---------------- */

static ICON: AtomicUsize = AtomicUsize::new(0);

unsafe fn set_tip(nid: &mut NOTIFYICONDATAW, text: &str) {
    let w: Vec<u16> = text.encode_utf16().take(nid.szTip.len() - 1).collect();
    nid.szTip = [0; 128];
    nid.szTip[..w.len()].copy_from_slice(&w);
}

fn dbg(msg: &str) {
    if std::env::var_os("AKKO_ECHO_DEBUG").is_some() {
        crate::log(&format!("DIAG {msg}"));
    }
}

unsafe fn update_icon(h: HWND, add: bool) {
    dbg("update_icon inicio");
    let e = ESTADO.lock().unwrap().clone().unwrap_or_default();
    dbg("update_icon estado ok");
    let icon = battery_icon(e.bateria);
    dbg("update_icon icone ok");
    let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
    nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = h;
    nid.uID = 1;
    nid.uFlags = NIF_ICON | NIF_TIP | NIF_MESSAGE;
    nid.uCallbackMessage = WM_TRAY;
    nid.hIcon = icon;
    let mut tip = String::from("Akko Echo Ultra");
    match e.status.as_str() {
        "conectado" if e.carregado => {
            let st = e.estagios.get(e.ativo).cloned().unwrap_or_default();
            tip += &format!("\n{} DPI \u{b7} {} Hz", st.dpi, e.polling_hz);
        }
        "pausado" => tip += &format!("\n{}", t("tray_paused")),
        _ => tip += &format!("\n{}", t("tray_searching")),
    }
    if e.bateria >= 0 {
        tip += &format!("\n{}: {}%", t("battery"), e.bateria);
        if e.carregando {
            tip += &format!(" ({})", t("charging"));
        }
    }
    set_tip(&mut nid, &tip);
    dbg("update_icon notify...");
    Shell_NotifyIconW(if add { NIM_ADD } else { NIM_MODIFY }, &nid);
    dbg("update_icon notify ok");
    let old = ICON.swap(icon as usize, Ordering::Relaxed);
    if old != 0 {
        DestroyIcon(old as HICON);
    }
    dbg("update_icon fim");
}

unsafe fn show_menu(h: HWND) {
    let menu = CreatePopupMenu();
    AppendMenuW(menu, MF_STRING | MF_DEFAULT, ID_OPEN, wide(t("tray_open")).as_ptr());
    let auto = if autostart::is_enabled() { MF_CHECKED } else { MF_UNCHECKED };
    AppendMenuW(menu, MF_STRING | auto, ID_AUTOSTART, wide(t("launch_startup")).as_ptr());
    AppendMenuW(menu, MF_SEPARATOR, 0, null());
    AppendMenuW(menu, MF_STRING, ID_EXIT, wide(t("tray_exit")).as_ptr());
    let mut pt: POINT = std::mem::zeroed();
    GetCursorPos(&mut pt);
    SetForegroundWindow(h);
    let cmd = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_BOTTOMALIGN, pt.x, pt.y, 0, h, null()) as usize;
    DestroyMenu(menu);
    match cmd {
        ID_OPEN => spawn_ui(),
        ID_AUTOSTART => {
            autostart::set_enabled(!autostart::is_enabled());
            republish();
        }
        ID_EXIT => exit_app(h),
        _ => {}
    }
}

/// Sai de vez: fecha a janela de configuração (se aberta), tira o ícone e encerra o processo.
unsafe fn exit_app(h: HWND) {
    crate::log("saindo (menu da bandeja)");
    let ui = FindWindowW(null(), wide("Akko Echo Ultra").as_ptr());
    if !ui.is_null() {
        PostMessageW(ui, WM_CLOSE, 0, 0);
        // espera a janela fechar (senão ela veria a bandeja sumir e a iniciaria de novo)
        for _ in 0..30 {
            if IsWindow(ui) == 0 {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    DestroyWindow(h);
}

/// Publica de novo o último estado (depois de mudar preferências ou inicialização).
fn republish() {
    let mut g = ESTADO.lock().unwrap();
    if let Some(cur) = g.as_mut() {
        cur.versao += 1;
        cur.prefs = prefs();
        cur.autostart = autostart::is_enabled();
        cur.akko_autostart = autostart::akko_enabled();
        cur.save();
    }
    drop(g);
    let h = HWND_TRAY.load(Ordering::Relaxed);
    if h != 0 {
        unsafe { PostMessageW(h as HWND, WM_STATE, 0, 0) };
    }
}

fn handle_command(c: Comando) {
    match c {
        Comando::Prefs { prefs: mut p } => {
            // os estágios salvos e o macro no mouse são da bandeja, não da janela
            let old = prefs();
            p.estagios_salvos = old.estagios_salvos.clone();
            p.macro_no_mouse = old.macro_no_mouse.clone();
            // O macro que está no mouse foi editado ou renomeado?
            let mut changed = false;
            if let Some(n) = old.macro_no_mouse.clone() {
                let before = old.macros.iter().find(|m| m.nome == n);
                match p.macros.iter().find(|m| m.nome == n) {
                    Some(now) => changed = before.map_or(false, |b| b.eventos != now.eventos),
                    None => {
                        // renomeado: mesma posição, mesma quantidade de macros
                        let idx = old.macros.iter().position(|m| m.nome == n);
                        if let (Some(i), true) = (idx, old.macros.len() == p.macros.len()) {
                            p.macro_no_mouse = Some(p.macros[i].nome.clone());
                            changed = p.macros[i].eventos != old.macros[i].eventos;
                        }
                    }
                }
            }
            i18n::set_lang(&p.idioma);
            p.save();
            *PREFS.lock().unwrap() = Some(p);
            republish();
            if changed {
                if let Some(tx) = DEV_TX.get() {
                    let _ = tx.send(Comando::RegravarMacro);
                }
            }
        }
        Comando::Autostart { on } => {
            autostart::set_enabled(on);
            republish();
        }
        Comando::DesativarAkko => {
            autostart::disable_akko();
            republish();
        }
        other => {
            if let Some(tx) = DEV_TX.get() {
                let _ = tx.send(other);
            }
        }
    }
}

unsafe extern "system" fn wndproc(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    match m {
        WM_TRAY => {
            match l as u32 {
                WM_LBUTTONUP | WM_LBUTTONDBLCLK => spawn_ui(),
                WM_RBUTTONUP | WM_CONTEXTMENU => show_menu(h),
                _ => {}
            }
            0
        }
        WM_STATE => {
            update_icon(h, false);
            0
        }
        WM_OPEN_UI => {
            spawn_ui();
            0
        }
        WM_COPYDATA => {
            let cds = &*(l as *const COPYDATASTRUCT);
            let bytes = std::slice::from_raw_parts(cds.lpData as *const u8, cds.cbData as usize);
            if let Ok(c) = serde_json::from_slice::<Comando>(bytes) {
                handle_command(c);
            }
            1
        }
        WM_DESTROY => {
            let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
            nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
            nid.hWnd = h;
            nid.uID = 1;
            Shell_NotifyIconW(NIM_DELETE, &nid);
            PostQuitMessage(0);
            0
        }
        _ if m == TASKBAR_CREATED.load(Ordering::Relaxed) as u32 && m != 0 => {
            update_icon(h, true);
            0
        }
        _ => DefWindowProcW(h, m, w, l),
    }
}

pub fn run(open_ui: bool) {
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let p = Prefs::load();
        i18n::set_lang(&p.idioma);
        *PREFS.lock().unwrap() = Some(p);
        osd::start();

        let inst = GetModuleHandleW(null());
        let cls = wide(CLASS);
        let mut wc: WNDCLASSW = std::mem::zeroed();
        wc.lpfnWndProc = Some(wndproc);
        wc.hInstance = inst;
        wc.lpszClassName = cls.as_ptr();
        RegisterClassW(&wc);
        let h = CreateWindowExW(0, cls.as_ptr(), wide("Akko Echo Ultra (bandeja)").as_ptr(), WS_POPUP, 0, 0, 0, 0, null_mut(), null_mut(), inst, null());
        HWND_TRAY.store(h as usize, Ordering::Relaxed);
        TASKBAR_CREATED.store(RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()) as usize, Ordering::Relaxed);
        // Sem ChangeWindowMessageFilterEx: só programas com o mesmo nível de integridade (a janela
        // de configuração) mandam comandos; processos em sandbox (integridade baixa) não.

        let (tx, rx) = channel();
        let _ = DEV_TX.set(tx);
        std::thread::Builder::new().name("mouse".into()).spawn(move || device_thread(rx)).ok();
        std::thread::Builder::new()
            .name("avisos".into())
            .spawn(|| {
                crate::mouse::listen_notifications(|r| {
                    crate::log(&format!("aviso do mouse: {}", r.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ")));
                    BATTERY_NOW.store(true, Ordering::Relaxed);
                })
            })
            .ok();
        update_icon(h, true);
        if open_ui {
            spawn_ui();
        }

        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    // Encerra o processo inteiro (as threads do mouse e do aviso não seguram o programa aberto).
    std::process::exit(0);
}

/// Envia um comando para a bandeja (usado pela janela de configuração).
pub fn send(c: &Comando) -> bool {
    // A prévia (AKKO_ECHO_PREVIEW, dados de exemplo) nunca manda nada para a bandeja de verdade.
    if std::env::var_os("AKKO_ECHO_PREVIEW").is_some() {
        return false;
    }
    let Some(h) = find_existing() else { return false };
    let json = serde_json::to_vec(c).unwrap_or_default();
    let cds = COPYDATASTRUCT { dwData: 1, cbData: json.len() as u32, lpData: json.as_ptr() as *mut _ };
    unsafe { SendMessageW(h, WM_COPYDATA, 0, &cds as *const _ as LPARAM) != 0 }
}
