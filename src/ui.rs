//! Janela de configuração (processo separado, aberto pela bandeja com --ui). Visual copiado do app
//! da Akko (janela sem moldura, posições absolutas em pontos). Lê o estado publicado pela bandeja
//! (state.json) e manda comandos para ela; nunca fala com o mouse diretamente.

use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use eframe::egui::{self, Align2, Color32, FontFamily, FontId, Pos2, Rect, Rounding, Sense, Shape, Stroke, Vec2};

use crate::i18n::{self, t};
use crate::keys;
use crate::mouse::{BUTTONS, DEBOUNCE_LABELS, DEFAULT_BUTTONS, MACRO_CODE, MACRO_MAX_EVENTS, POLLING_HZ};
use crate::games;
use crate::state::{data_dir, Comando, Estado, EventoMacro, Macro, Prefs, RegraJogo};
use crate::tray;

const W: f32 = 1180.0;
const H: f32 = 750.0;
const BG: Color32 = Color32::from_rgb(0x1c, 0x1c, 0x1c);
const CARD: Color32 = Color32::from_rgb(0x24, 0x24, 0x24);
const CHIP: Color32 = Color32::from_rgb(0x24, 0x24, 0x24);
const PILL: Color32 = Color32::from_rgb(0x2c, 0x2c, 0x2c);
const PILL_HOVER: Color32 = Color32::from_rgb(0x38, 0x38, 0x38);
const TRACK: Color32 = Color32::from_rgb(0x2e, 0x2e, 0x2e);
const TAB: Color32 = Color32::from_rgb(0x26, 0x2d, 0x2f);
const PURPLE: Color32 = Color32::from_rgb(0x67, 0x34, 0xc8);
const PURPLE_LIGHT: Color32 = Color32::from_rgb(0x9a, 0x74, 0xf0);
const TEXT: Color32 = Color32::WHITE;
const SOFT: Color32 = Color32::from_rgb(0xd2, 0xd2, 0xd2);
const MUTED: Color32 = Color32::from_rgb(0x86, 0x86, 0x86);
const WARN: Color32 = Color32::from_rgb(0xf0, 0xb4, 0x4c);
const PANEL: Color32 = Color32::from_rgb(0x18, 0x18, 0x18);
const BORDER: Color32 = Color32::from_rgb(0x4a, 0x4a, 0x4a);

/// Funções do menu de cada botão: (seção, nome, código). Códigos confirmados capturando o app
/// da Akko; os de multimídia não capturados seguem a tabela HID "consumer" (como os capturados).
const BTN_FUNCS: &[(&str, &str, [u8; 3])] = &[
    ("mouse_buttons", "btn_left", [0x10, 0x01, 0]),
    ("mouse_buttons", "btn_middle", [0x10, 0x04, 0]),
    ("mouse_buttons", "btn_right", [0x10, 0x02, 0]),
    ("mouse_buttons", "btn_forward", [0x10, 0x10, 0]),
    ("mouse_buttons", "btn_back", [0x10, 0x08, 0]),
    ("mouse_buttons", "disable_btn", [0x20, 0, 0]),
    ("dpi_keys", "dpi_up", [0x13, 0x01, 0]),
    ("dpi_keys", "dpi_down", [0x13, 0x02, 0]),
    ("dpi_keys", "dpi_cycle", [0x13, 0x03, 0]),
    ("multimedia", "media_player", [0x30, 0x83, 0x01]),
    ("multimedia", "media_play", [0x30, 0xcd, 0]),
    ("multimedia", "media_stop", [0x30, 0xb7, 0]),
    ("multimedia", "media_prev", [0x30, 0xb6, 0]),
    ("multimedia", "media_next", [0x30, 0xb5, 0]),
    ("multimedia", "media_vol_down", [0x30, 0xea, 0]),
    ("multimedia", "media_vol_up", [0x30, 0xe9, 0]),
    ("multimedia", "media_mute", [0x30, 0xe2, 0]),
    ("multimedia", "web_home", [0x30, 0x23, 0x02]),
    ("multimedia", "web_refresh", [0x30, 0x27, 0x02]),
    ("multimedia", "web_stop", [0x30, 0x26, 0x02]),
    ("multimedia", "web_back", [0x30, 0x24, 0x02]),
    ("multimedia", "web_forward", [0x30, 0x25, 0x02]),
    ("multimedia", "web_favorites", [0x30, 0x2a, 0x02]),
    ("multimedia", "web_search", [0x30, 0x21, 0x02]),
    ("multimedia", "my_computer", [0x30, 0x94, 0x01]),
    ("multimedia", "calculator", [0x30, 0x92, 0x01]),
    ("multimedia", "email", [0x30, 0x8a, 0x01]),
    // Office: atalhos com Ctrl (como o Ctrl+C capturado: 20 01 06)
    ("office", "office_open", [0x20, 0x01, 0x12]),
    ("office", "office_new", [0x20, 0x01, 0x11]),
    ("office", "office_undo", [0x20, 0x01, 0x1d]),
    ("office", "office_save", [0x20, 0x01, 0x16]),
    ("office", "office_copy", [0x20, 0x01, 0x06]),
    ("office", "office_cut", [0x20, 0x01, 0x1b]),
    ("office", "office_paste", [0x20, 0x01, 0x19]),
    ("office", "office_find", [0x20, 0x01, 0x09]),
    ("office", "office_select_all", [0x20, 0x01, 0x04]),
    ("office", "office_zoom_out", [0x20, 0x01, 0x2d]),
    ("office", "office_zoom_in", [0x20, 0x01, 0x2e]),
];

/// Posição de cada botão na tela (como no app da Akko): índice, caixa, bolinha.
const BTN_POS: [(usize, [f32; 4], [f32; 2]); BUTTONS] = [
    (0, [471.0, 216.0, 621.0, 247.0], [646.0, 231.0]),
    (1, [801.0, 216.0, 951.0, 247.0], [777.0, 231.0]),
    (2, [857.0, 305.0, 1007.0, 336.0], [833.0, 320.0]),
    (4, [422.0, 370.0, 572.0, 401.0], [597.0, 385.0]),
    (3, [422.0, 430.0, 572.0, 461.0], [597.0, 445.0]),
];

/// Seções do painel de funções dos botões (na ordem do app da Akko).
const PANEL_SECTIONS: [&str; 6] = ["mouse_buttons", "dpi_keys", "keyboard_keys", "multimedia", "macro_buttons", "office"];

/// Modificadores do painel de teclado (como no Akko): nome e bit no byte de modificadores HID.
const MODS: [(&str, u8); 8] = [
    ("Shift_L", 0x02),
    ("Ctrl_L", 0x01),
    ("Alt_L", 0x04),
    ("Win_L", 0x08),
    ("Shift_R", 0x20),
    ("Ctrl_R", 0x10),
    ("Alt_R", 0x40),
    ("Win_R", 0x80),
];

/// Seção que contém a função atual de um botão (para abrir já nela).
fn section_of(code: [u8; 3]) -> usize {
    if BTN_FUNCS.iter().any(|f| f.0 == "office" && f.2 == code) {
        return 5;
    }
    match code[0] {
        0x13 => 1,
        0x20 if code[1] != 0 || code[2] != 0 => 2,
        0x30 => 3,
        0x70 => 4,
        _ => 0,
    }
}

/// Gravação de macro em andamento.
struct Recording {
    prev: [bool; 256],
    last: Instant,
    events: Vec<EventoMacro>,
}

const SLEEP_MIN: [u16; 7] = [1, 5, 10, 15, 20, 25, 30];
const AVISO_MS: [u32; 4] = [1000, 1500, 2000, 3000];

// Barras com marcações (polling, debounce, suspensão): mesma geometria do Akko.
const SL_X0: f32 = 300.0;
const SL_X1: f32 = 897.0;
const SL_FIRST: f32 = 316.0;
const SL_STEP: f32 = 94.0;

#[derive(PartialEq, Clone, Copy)]
enum Page {
    Buttons,
    Performance,
    Macro,
    Games,
    Settings,
    About,
}

struct App {
    est: Option<Estado>,
    est_mtime: Option<SystemTime>,
    last_poll: Instant,
    last_alive: Instant,
    /// Quando a janela iniciou a bandeja sozinha (ela não estava rodando).
    tray_started: Option<Instant>,
    tray_missing_since: Option<Instant>,
    page: Page,
    /// DPI em edição (estágio, valor, último toque): enviado depois de 400 ms sem mudanças.
    dpi_edit: Option<(usize, u16, Instant)>,
    dpi_text: String,
    dpi_text_stage: usize,
    color_edit: Option<(usize, [u8; 3], Instant)>,
    /// Seletor de cor aberto (estágio) e se acabou de abrir (ignora o clique que o abriu).
    color_popup: Option<usize>,
    lang_open: bool,
    popup_just_opened: bool,
    /// Barra com marcações sendo arrastada: (nome, posição).
    drag: Option<(&'static str, usize)>,
    photo: Option<egui::TextureHandle>,
    mouse_top: Option<egui::TextureHandle>,
    /// Imagens do menu lateral do app da Akko (normal, selecionado), na ordem do menu.
    tab_imgs: Vec<Option<(egui::TextureHandle, egui::TextureHandle)>>,
    /// Menu de funções aberto (botão) e captura de tecla em andamento (botão, teclas anteriores).
    btn_menu: Option<usize>,
    /// Seção aberta no painel de funções (usize::MAX = nenhuma).
    btn_section: usize,
    panel_scroll: f32,
    panel_height: f32,
    macro_dropdown: bool,
    /// Lista de programas abertos para escolher um jogo (aberta pelo "Adicionar jogo").
    game_picker: Option<Vec<(String, String)>>,
    game_exe_text: String,
    key_capture: Option<(usize, [bool; 256])>,
    macro_sel: Option<usize>,
    macro_name: String,
    recording: Option<Recording>,
}

fn semibold() -> FontFamily {
    FontFamily::Name("semibold".into())
}

fn font(size: f32, bold: bool) -> FontId {
    if bold {
        FontId::new(size, semibold())
    } else {
        FontId::proportional(size)
    }
}

/// Pasta com as imagens e fontes do app da Akko: a cópia instalada junto com este programa
/// ("akko" ao lado do .exe, feita pelo instalador) ou, se não houver, o app da Akko instalado.
fn akko_dir() -> std::path::PathBuf {
    let own = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join("akko")));
    match own {
        Some(d) if d.join("Skin").is_dir() => d,
        _ => std::path::PathBuf::from(r"C:\Program Files (x86)\Akko"),
    }
}

fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let dir = akko_dir().join("Font");
    if let Ok(b) = std::fs::read(dir.join("MiSans-Normal.ttf")) {
        fonts.font_data.insert("misans".into(), egui::FontData::from_owned(b));
        fonts.families.get_mut(&FontFamily::Proportional).unwrap().insert(0, "misans".into());
    }
    let mut sb = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    if let Ok(b) = std::fs::read(dir.join("MiSans-Semibold.ttf")) {
        fonts.font_data.insert("misans_sb".into(), egui::FontData::from_owned(b));
        sb.insert(0, "misans_sb".into());
    }
    fonts.families.insert(semibold(), sb);
    ctx.set_fonts(fonts);

    let mut v = egui::Visuals::dark();
    v.panel_fill = BG;
    v.window_fill = Color32::from_rgb(0x2a, 0x2a, 0x2a);
    v.extreme_bg_color = BG;
    v.selection.bg_fill = PURPLE;
    v.selection.stroke = Stroke::new(1.0_f32, TEXT);
    v.widgets.inactive.bg_fill = PILL;
    v.widgets.inactive.weak_bg_fill = PILL;
    v.widgets.hovered.weak_bg_fill = PILL_HOVER;
    v.window_stroke = Stroke::new(1.0_f32, Color32::from_rgb(0x44, 0x44, 0x44));
    ctx.set_visuals(v);
}

fn color32(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

fn r(x0: f32, y0: f32, x1: f32, y1: f32) -> Rect {
    Rect::from_min_max(Pos2::new(x0, y0), Pos2::new(x1, y1))
}

fn p(x: f32, y: f32) -> Pos2 {
    Pos2::new(x, y)
}

/// Texto com quebra de linha automática a partir de (x, y) (canto superior esquerdo).
fn wrapped(ui: &egui::Ui, x: f32, y: f32, width: f32, text: &str, size: f32, color: Color32) -> f32 {
    let galley = ui.painter().layout(text.to_string(), font(size, false), color, width);
    let h = galley.size().y;
    ui.painter().galley(p(x, y), galley, color);
    h
}

fn label(ui: &egui::Ui, x: f32, y: f32, text: &str, size: f32, bold: bool, color: Color32) -> Rect {
    ui.painter().text(p(x, y), Align2::LEFT_CENTER, text, font(size, bold), color)
}

/// Botão em pílula (roxo quando selecionado), como os do Akko.
fn pill(ui: &mut egui::Ui, rect: Rect, text: &str, selected: bool) -> egui::Response {
    let resp = ui.interact(rect, ui.id().with(("pill", rect.min.x as i32, rect.min.y as i32)), Sense::click());
    let fill = if selected {
        PURPLE
    } else if resp.hovered() {
        PILL_HOVER
    } else {
        PILL
    };
    ui.painter().rect_filled(rect, Rounding::same(rect.height() / 2.0), fill);
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, text, font(14.0, true), TEXT);
    resp
}

/// Par de pílulas ON/OFF; devolve o novo valor se mudou.
fn on_off(ui: &mut egui::Ui, x: f32, y: f32, on: bool) -> Option<bool> {
    let mut out = None;
    if pill(ui, r(x, y, x + 83.0, y + 30.0), "ON", on).clicked() && !on {
        out = Some(true);
    }
    if pill(ui, r(x + 97.0, y, x + 180.0, y + 30.0), "OFF", !on).clicked() && on {
        out = Some(false);
    }
    out
}

/* ---------------- ícones desenhados ---------------- */

fn icon_pulse(ui: &egui::Ui, c: Pos2) {
    let pts = [(-9.0, 1.0), (-5.0, 1.0), (-3.0, -6.0), (0.0, 7.0), (3.0, -3.0), (5.0, 1.0), (9.0, 1.0)];
    let pts: Vec<Pos2> = pts.iter().map(|&(x, y)| c + Vec2::new(x, y)).collect();
    ui.painter().add(Shape::line(pts, Stroke::new(1.4_f32, TEXT)));
}

fn icon_mouse(ui: &egui::Ui, c: Pos2) {
    let pa = ui.painter();
    let body = Rect::from_center_size(c, Vec2::new(11.0, 17.0));
    pa.rect_stroke(body, Rounding::same(5.5), Stroke::new(1.4_f32, TEXT));
    pa.line_segment([c + Vec2::new(0.0, -8.5), c + Vec2::new(0.0, -3.5)], Stroke::new(1.4_f32, TEXT));
}

fn icon_macro(ui: &egui::Ui, c: Pos2) {
    // mão apontando para cima (como o ícone de Macro do Akko), da fonte de símbolos do egui
    ui.painter().text(c + Vec2::new(0.0, 1.0), Align2::CENTER_CENTER, "\u{261d}", FontId::proportional(18.0), TEXT);
}

fn icon_gamepad(ui: &egui::Ui, c: Pos2) {
    let pa = ui.painter();
    let st = Stroke::new(1.4_f32, TEXT);
    pa.rect_stroke(Rect::from_center_size(c, Vec2::new(20.0, 12.0)), Rounding::same(6.0), st);
    pa.line_segment([c + Vec2::new(-7.5, 0.0), c + Vec2::new(-2.5, 0.0)], st);
    pa.line_segment([c + Vec2::new(-5.0, -2.5), c + Vec2::new(-5.0, 2.5)], st);
    pa.circle_filled(c + Vec2::new(4.0, -1.5), 1.2, TEXT);
    pa.circle_filled(c + Vec2::new(6.5, 1.5), 1.2, TEXT);
}

fn icon_gear(ui: &egui::Ui, c: Pos2) {
    let pa = ui.painter();
    pa.circle_stroke(c, 6.0, Stroke::new(1.4_f32, TEXT));
    pa.circle_stroke(c, 2.3, Stroke::new(1.4_f32, TEXT));
    for k in 0..8 {
        let a = k as f32 * std::f32::consts::FRAC_PI_4;
        let d = Vec2::new(a.cos(), a.sin());
        pa.line_segment([c + d * 6.0, c + d * 8.6], Stroke::new(2.0_f32, TEXT));
    }
}

fn icon_info(ui: &egui::Ui, c: Pos2) {
    let pa = ui.painter();
    pa.circle_stroke(c, 8.5, Stroke::new(1.4_f32, TEXT));
    pa.circle_filled(c + Vec2::new(0.0, -4.0), 1.2, TEXT);
    pa.line_segment([c + Vec2::new(0.0, -1.0), c + Vec2::new(0.0, 5.0)], Stroke::new(1.5_f32, TEXT));
}

/// Mouse roxo inclinado da aba do dispositivo.
fn icon_device(ui: &egui::Ui, c: Pos2) {
    let (a, b) = (9.5_f32, 14.0_f32);
    let ang = -0.75_f32;
    let pts: Vec<Pos2> = (0..40)
        .map(|k| {
            let th = k as f32 / 40.0 * std::f32::consts::TAU;
            let (x, y) = (a * th.cos(), b * th.sin());
            c + Vec2::new(x * ang.cos() - y * ang.sin(), x * ang.sin() + y * ang.cos())
        })
        .collect();
    ui.painter().add(Shape::convex_polygon(pts, PURPLE_LIGHT, Stroke::NONE));
    let d = Vec2::new(ang.sin(), -ang.cos());
    ui.painter().line_segment([c + d * -9.0, c + d * -4.0], Stroke::new(2.0_f32, Color32::from_rgb(0x3a, 0x22, 0x80)));
}

/* ---------------- foto do mouse (do app da Akko, se instalado) ---------------- */

fn load_png(path: &std::path::Path) -> Option<egui::ColorImage> {
    let file = std::fs::File::open(path).ok()?;
    let mut dec = png::Decoder::new(std::io::BufReader::new(file));
    dec.set_transformations(png::Transformations::EXPAND);
    let mut reader = dec.read_info().ok()?;
    let mut buf = vec![0u8; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    let (w, h) = (info.width as usize, info.height as usize);
    let rgba: Vec<u8> = match info.color_type {
        png::ColorType::Rgba => buf[..w * h * 4].to_vec(),
        png::ColorType::Rgb => buf[..w * h * 3].chunks(3).flat_map(|c| [c[0], c[1], c[2], 255]).collect(),
        _ => return None,
    };
    Some(egui::ColorImage::from_rgba_unmultiplied([w, h], &rgba))
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        setup_fonts(&cc.egui_ctx);
        let est = Estado::load();
        if let Some(e) = &est {
            i18n::set_lang(&e.prefs.idioma);
        }
        let skin = akko_dir().join("Skin");
        let photo = [skin.join(r"About\M0019_mouse@150.png"), skin.join(r"ms\M0019\mouse@150.png")]
            .iter()
            .find_map(|f| load_png(f))
            .map(|img| cc.egui_ctx.load_texture("mouse", img, egui::TextureOptions::LINEAR));
        App {
            est,
            est_mtime: None,
            last_poll: Instant::now(),
            last_alive: Instant::now() - Duration::from_secs(60),
            tray_started: None,
            tray_missing_since: None,
            page: Page::Performance,
            dpi_edit: None,
            dpi_text: String::new(),
            dpi_text_stage: usize::MAX,
            color_edit: None,
            color_popup: None,
            lang_open: false,
            popup_just_opened: false,
            drag: None,
            photo,
            mouse_top: load_png(&skin.join(r"ms\M0019\mousea@150.png"))
                .map(|img| cc.egui_ctx.load_texture("mouse_top", img, egui::TextureOptions::LINEAR)),
            tab_imgs: ["key", "dpi", "marco", "", "", "about"]
                .iter()
                .map(|name| {
                    if name.is_empty() {
                        return None; // "Games" e "Settings" não existem no Akko: ícone desenhado
                    }
                    let img = |kind: &str| {
                        load_png(&skin.join(format!(r"control\{name}_{kind}@150.png")))
                            .map(|i| cc.egui_ctx.load_texture(format!("tab_{name}_{kind}"), i, egui::TextureOptions::LINEAR))
                    };
                    Some((img("normal")?, img("hot")?))
                })
                .collect(),
            btn_menu: None,
            btn_section: 0,
            panel_scroll: 0.0,
            panel_height: 0.0,
            macro_dropdown: false,
            game_picker: None,
            game_exe_text: String::new(),
            key_capture: None,
            macro_sel: None,
            macro_name: String::new(),
            recording: None,
        }
    }

    fn refresh(&mut self) {
        if self.last_poll.elapsed() < Duration::from_millis(250) {
            return;
        }
        self.last_poll = Instant::now();
        let mtime = std::fs::metadata(data_dir().join("state.json")).and_then(|m| m.modified()).ok();
        if mtime != self.est_mtime {
            self.est_mtime = mtime;
            if let Some(e) = Estado::load() {
                i18n::set_lang(&e.prefs.idioma);
                self.est = Some(e);
            }
        }
        // Sem a bandeja (ex.: depois de "Sair"), a janela inicia ela sozinha.
        let preview = std::env::var_os("AKKO_ECHO_PREVIEW").is_some();
        // (espera 1,5 s sem bandeja: ao "Sair", a bandeja fecha esta janela logo em seguida)
        let missing = !preview && tray::find_existing().is_none();
        if !missing {
            self.tray_missing_since = None;
        } else if self.tray_missing_since.is_none() {
            self.tray_missing_since = Some(Instant::now());
        }
        let long_enough = self.tray_missing_since.map_or(false, |t| t.elapsed() > Duration::from_millis(1500));
        if missing && long_enough && self.tray_started.map_or(true, |t| t.elapsed() > Duration::from_secs(10)) {
            self.tray_started = Some(Instant::now());
            if let Ok(exe) = std::env::current_exe() {
                let _ = std::process::Command::new(exe).arg("--bandeja").spawn();
            }
            crate::log("janela: bandeja não estava rodando; iniciada");
        }
        if self.last_alive.elapsed() > Duration::from_secs(15) {
            self.last_alive = Instant::now();
            tray::send(&Comando::UiViva);
        }
        // Envia edições pendentes (DPI e cor) depois de um tempo parado.
        if let Some((i, dpi, t)) = self.dpi_edit {
            if t.elapsed() > Duration::from_millis(400) {
                tray::send(&Comando::Dpi { i, dpi });
                self.dpi_edit = None;
            }
        }
        if let Some((i, rgb, t)) = self.color_edit {
            if t.elapsed() > Duration::from_millis(400) && self.color_popup.is_none() {
                tray::send(&Comando::Cor { i, rgb });
                self.color_edit = None;
            }
        }
    }

    fn send_prefs(&mut self, f: impl FnOnce(&mut Prefs)) {
        if let Some(e) = &mut self.est {
            f(&mut e.prefs);
            i18n::set_lang(&e.prefs.idioma);
            tray::send(&Comando::Prefs { prefs: e.prefs.clone() });
        }
    }

    /// Barra com marcações (polling, debounce, suspensão). Devolve a posição escolhida ao soltar.
    fn ticked_slider(&mut self, ui: &mut egui::Ui, name: &'static str, y: f32, labels: &[String], idx: usize) -> Option<usize> {
        let n = labels.len();
        let pos = |i: usize| SL_FIRST + SL_STEP * i as f32;
        let hit = r(SL_X0 - 6.0, y - 13.0, SL_X1 + 6.0, y + 13.0);
        let resp = ui.interact(hit, ui.id().with(name), Sense::click_and_drag());
        let nearest = |x: f32| (((x - SL_FIRST) / SL_STEP).round().max(0.0) as usize).min(n - 1);
        let mut out = None;
        if let Some(pt) = resp.interact_pointer_pos() {
            if resp.is_pointer_button_down_on() {
                self.drag = Some((name, nearest(pt.x)));
            }
            if resp.drag_stopped() || resp.clicked() {
                let i = nearest(pt.x);
                self.drag = None;
                if i != idx {
                    out = Some(i);
                }
            }
        }
        let shown = match self.drag {
            Some((nm, i)) if nm == name => i,
            _ => out.unwrap_or(idx),
        };
        let kx = pos(shown.min(n - 1));
        let pa = ui.painter();
        pa.rect_filled(r(SL_X0, y - 5.0, SL_X1, y + 5.0), Rounding::same(5.0), TRACK);
        pa.rect_filled(r(SL_X0, y - 5.0, kx, y + 5.0), Rounding::same(5.0), PURPLE);
        let knob = Rect::from_center_size(p(kx, y), Vec2::new(30.0, 16.0));
        pa.rect_filled(knob.translate(Vec2::new(0.0, 1.0)), Rounding::same(8.0), Color32::from_black_alpha(90));
        pa.rect_filled(knob, Rounding::same(8.0), Color32::from_rgb(0xf2, 0xf2, 0xf2));
        for (i, l) in labels.iter().enumerate() {
            let c = if i == shown { SOFT } else { MUTED };
            pa.text(p(pos(i), y + 25.0), Align2::CENTER_CENTER, l, font(13.0, false), c);
        }
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        out
    }

    /* ---------------- cabeçalho e menu lateral ---------------- */

    fn header(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        // Arrastar pelo cabeçalho move a janela (não há moldura do Windows).
        let drag = ui.interact(r(0.0, 0.0, W, 100.0), ui.id().with("drag"), Sense::click_and_drag());
        if drag.drag_started() {
            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }

        let title = label(ui, 38.0, 43.0, "<  TAN8 Ultra", 30.0, true, TEXT);
        if let Some(e) = &self.est {
            if e.bateria >= 0 {
                let b = r(title.right() + 6.0, 36.0, title.right() + 34.0, 51.0);
                let fill = if e.bateria <= 15 { Color32::from_rgb(235, 60, 60) } else { Color32::WHITE };
                ui.painter().rect_filled(b, Rounding::same(3.0), fill);
                ui.painter().rect_filled(r(b.right(), 40.5, b.right() + 2.5, 46.5), Rounding::same(1.0), Color32::from_rgb(0x9a, 0x9a, 0x9a));
                ui.painter().text(b.center(), Align2::CENTER_CENTER, e.bateria.to_string(), font(11.0, true), Color32::BLACK);
            }
        }

        // Aba do dispositivo
        ui.painter().rect_filled(r(350.0, 34.0, 570.0, 94.0), Rounding::same(4.0), TAB);
        icon_device(ui, p(453.0, 58.0));
        ui.painter().line_segment([p(436.0, 77.0), p(470.0, 77.0)], Stroke::new(2.0_f32, PURPLE_LIGHT));

        // Idioma
        let lang = r(935.0, 33.0, 1085.0, 61.0);
        let lr = ui.interact(lang, ui.id().with("lang"), Sense::click());
        ui.painter().rect_stroke(lang, Rounding::same(7.0), Stroke::new(1.0_f32, if lr.hovered() { SOFT } else { Color32::from_rgb(0x70, 0x70, 0x70) }));
        let name = if i18n::is_pt() { "Português" } else { "English" };
        label(ui, 947.0, 47.0, name, 14.0, false, TEXT);
        let cx = 1068.0;
        ui.painter().add(Shape::line(vec![p(cx - 4.0, 45.0), p(cx, 49.0), p(cx + 4.0, 45.0)], Stroke::new(1.2_f32, SOFT)));
        if lr.clicked() {
            self.lang_open = !self.lang_open;
            self.popup_just_opened = true;
        }

        // Minimizar e fechar (para a bandeja)
        let min = Rect::from_center_size(p(1115.0, 47.0), Vec2::splat(28.0));
        let mr = ui.interact(min, ui.id().with("min"), Sense::click()).on_hover_text(t("minimize"));
        if mr.hovered() {
            ui.painter().rect_filled(min, Rounding::same(4.0), PILL);
        }
        ui.painter().line_segment([p(1107.0, 47.0), p(1123.0, 47.0)], Stroke::new(1.6_f32, TEXT));
        if mr.clicked() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
        }
        let close = Rect::from_center_size(p(1150.0, 47.0), Vec2::splat(28.0));
        let cr = ui.interact(close, ui.id().with("close"), Sense::click()).on_hover_text(t("close"));
        if cr.hovered() {
            ui.painter().rect_filled(close, Rounding::same(4.0), Color32::from_rgb(0xc4, 0x2b, 0x1c));
        }
        let s = Stroke::new(1.6_f32, TEXT);
        ui.painter().line_segment([p(1142.0, 39.0), p(1158.0, 55.0)], s);
        ui.painter().line_segment([p(1158.0, 39.0), p(1142.0, 55.0)], s);
        if cr.clicked() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn lang_popup(&mut self, ctx: &egui::Context) {
        if !self.lang_open {
            return;
        }
        let area = egui::Area::new(egui::Id::new("lang_popup"))
            .order(egui::Order::Foreground)
            .fixed_pos(p(935.0, 65.0))
            .show(ctx, |ui| {
                let base = ui.min_rect().min;
                let (rect, _) = ui.allocate_exact_size(Vec2::new(150.0, 70.0), Sense::hover());
                ui.painter().rect(rect, Rounding::same(7.0), Color32::from_rgb(0x2a, 0x2a, 0x2a), Stroke::new(1.0_f32, Color32::from_rgb(0x44, 0x44, 0x44)));
                for (k, (code, name)) in [("en", "English"), ("pt", "Português")].iter().enumerate() {
                    let item = r(base.x + 4.0, base.y + 4.0 + k as f32 * 31.0, base.x + 146.0, base.y + 35.0 + k as f32 * 31.0);
                    let resp = ui.interact(item, ui.id().with(code), Sense::click());
                    let cur = (*code == "pt") == i18n::is_pt();
                    if resp.hovered() || cur {
                        ui.painter().rect_filled(item, Rounding::same(5.0), if cur { PURPLE } else { PILL_HOVER });
                    }
                    ui.painter().text(item.left_center() + Vec2::new(10.0, 0.0), Align2::LEFT_CENTER, *name, font(14.0, false), TEXT);
                    if resp.clicked() {
                        let code = code.to_string();
                        self.send_prefs(|p| p.idioma = code);
                        self.lang_open = false;
                    }
                }
            });
        self.close_on_click_outside(ctx, area.response.rect, |s| s.lang_open = false);
    }

    fn close_on_click_outside(&mut self, ctx: &egui::Context, rect: Rect, close: impl FnOnce(&mut Self)) {
        if self.popup_just_opened {
            self.popup_just_opened = false;
            return;
        }
        let outside = ctx.input(|i| i.pointer.primary_pressed() && i.pointer.interact_pos().map_or(false, |pt| !rect.contains(pt)));
        if outside {
            close(self);
        }
    }

    fn sidebar(&mut self, ui: &mut egui::Ui) {
        let items = [
            (Page::Buttons, t("button_assignment")),
            (Page::Performance, t("performance")),
            (Page::Macro, t("macro")),
            (Page::Games, t("games")),
            (Page::Settings, t("settings")),
            (Page::About, t("about")),
        ];
        for (k, (page, text)) in items.into_iter().enumerate() {
            let y = 118.0 + k as f32 * 42.0;
            let rect = r(35.0, y, 255.0, y + 42.0);
            let resp = ui.interact(rect, ui.id().with(("side", k)), Sense::click());
            let sel = self.page == page;
            if !sel && resp.hovered() {
                ui.painter().rect_filled(rect, Rounding::same(8.0), Color32::from_rgb(0x28, 0x28, 0x28));
            }
            let uv = Rect::from_min_max(Pos2::ZERO, p(1.0, 1.0));
            if let Some(Some((normal, hot))) = self.tab_imgs.get(k) {
                // imagem do app da Akko (ícone, e no selecionado o fundo roxo com a bolinha)
                let tex = if sel { hot } else { normal };
                ui.painter().image(tex.id(), rect, uv, Color32::WHITE);
            } else {
                if sel {
                    ui.painter().rect_filled(rect, Rounding::same(8.0), PURPLE);
                    ui.painter().circle_stroke(p(228.0, y + 21.0), 6.0, Stroke::new(1.4_f32, TEXT));
                    ui.painter().circle_filled(p(228.0, y + 21.0), 3.0, TEXT);
                }
                let c = p(68.0, y + 21.0);
                match page {
                    Page::Buttons => icon_mouse(ui, c),
                    Page::Macro => icon_macro(ui, c),
                    Page::Games => icon_gamepad(ui, c),
                    Page::Performance => icon_pulse(ui, c),
                    Page::Settings => icon_gear(ui, c),
                    Page::About => icon_info(ui, c),
                }
            }
            label(ui, 83.0, y + 21.0, text, 14.0, false, TEXT);
            if resp.clicked() {
                self.page = page;
                self.color_popup = None;
                self.btn_menu = None;
                self.key_capture = None;
                self.stop_recording();
            }
        }
    }

    /// Mostra o aviso de estado no lugar da página; devolve true se o mouse está pronto.
    fn status(&mut self, ui: &mut egui::Ui) -> bool {
        // AKKO_ECHO_PREVIEW: só para ver a janela com dados de exemplo, sem a bandeja.
        if tray::find_existing().is_none() && std::env::var_os("AKKO_ECHO_PREVIEW").is_none() {
            wrapped(ui, 301.0, 160.0, 560.0, t("tray_starting"), 15.0, MUTED);
            return false;
        }
        let Some(e) = &self.est else {
            wrapped(ui, 301.0, 160.0, 560.0, t("loading"), 15.0, MUTED);
            return false;
        };
        match e.status.as_str() {
            "conectado" if e.carregado => true,
            "pausado" => {
                wrapped(ui, 301.0, 160.0, 600.0, t("paused"), 15.0, WARN);
                false
            }
            _ => {
                wrapped(ui, 301.0, 160.0, 600.0, t("searching"), 15.0, MUTED);
                false
            }
        }
    }

    /* ---------------- páginas ---------------- */

    fn page_performance(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        label(ui, 301.0, 127.0, t("performance"), 19.0, true, TEXT);
        if !self.status(ui) {
            return;
        }
        let Some(e) = self.est.clone() else { return };
        let sel = e.ativo.min(e.estagios.len().saturating_sub(1));

        label(ui, 301.0, 167.0, t("sensitivity"), 17.0, true, TEXT);
        let dv = label(ui, 301.0, 194.0, t("dpi_value"), 12.0, false, SOFT);
        label(ui, dv.right() + 14.0, 194.0, t("dpi_hint"), 12.0, false, MUTED);

        // Estágios
        let cur_dpi = match self.dpi_edit {
            Some((i, v, _)) if i == sel => v,
            _ => e.estagios.get(sel).map_or(800, |s| s.dpi),
        };
        for (i, st) in e.estagios.iter().enumerate() {
            let x = 299.0 + i as f32 * 92.0;
            let rect = r(x, 229.0, x + 75.0, 261.0);
            let sq = Rect::from_center_size(p(x + 16.0, 245.0), Vec2::splat(14.0));
            let resp = ui.interact(rect, ui.id().with(("chip", i)), Sense::click());
            let sq_resp = ui.interact(sq.expand(3.0), ui.id().with(("sq", i)), Sense::click());
            let cor = match self.color_edit {
                Some((j, c, _)) if j == i => c,
                _ => st.cor,
            };
            ui.painter().rect_filled(rect, Rounding::same(6.0), if resp.hovered() { PILL } else { CHIP });
            if i == sel {
                ui.painter().rect_stroke(rect, Rounding::same(6.0), Stroke::new(1.0_f32, color32(cor)));
            }
            ui.painter().rect_filled(sq, Rounding::same(2.0), color32(cor));
            if sq_resp.hovered() {
                ui.painter().rect_stroke(sq.expand(1.5), Rounding::same(3.0), Stroke::new(1.0_f32, TEXT));
            }
            let dpi = match self.dpi_edit {
                Some((j, v, _)) if j == i => v,
                _ => st.dpi,
            };
            label(ui, x + 28.0, 245.0, &dpi.to_string(), 15.0, false, TEXT);
            if sq_resp.clicked() {
                self.color_popup = Some(i);
                self.popup_just_opened = true;
            } else if resp.clicked() && i != sel {
                tray::send(&Comando::Ativar { i });
                if let Some(est) = &mut self.est {
                    est.ativo = i;
                }
            }
        }

        // Caixa com o DPI do estágio ativo
        if self.dpi_text_stage != sel || self.dpi_edit.is_none() && self.dpi_text.parse::<u16>().ok() != Some(cur_dpi) {
            self.dpi_text = cur_dpi.to_string();
            self.dpi_text_stage = sel;
        }
        // Na mesma linha da barra, logo depois dela; número centralizado na caixa.
        let bx = r(921.0, 284.0, 991.0, 314.0);
        ui.painter().rect_stroke(bx, Rounding::same(7.0), Stroke::new(1.0_f32, TEXT));
        let inner = bx.shrink(2.0);
        let te = egui::TextEdit::singleline(&mut self.dpi_text)
            .frame(false)
            .margin(egui::Margin::ZERO)
            .min_size(inner.size())
            .horizontal_align(egui::Align::Center)
            .vertical_align(egui::Align::Center)
            .font(font(15.0, true))
            .text_color(TEXT);
        let tr = ui.put(inner, te);
        if tr.lost_focus() {
            if let Ok(v) = self.dpi_text.trim().parse::<u16>() {
                let v = (v.clamp(100, 40000) / 50) * 50;
                self.dpi_edit = Some((sel, v, Instant::now() - Duration::from_secs(1)));
            }
        }
        label(ui, 1001.0, 299.0, "DPI", 15.0, true, TEXT);

        // Barra do DPI (preenchida em roxo até o valor)
        let (x0, x1, y) = (SL_X0, SL_X1, 299.0);
        let hit = r(x0 - 8.0, y - 11.0, x1 + 8.0, y + 11.0);
        let sr = ui.interact(hit, ui.id().with("dpi_slider"), Sense::click_and_drag());
        if let Some(pt) = sr.interact_pointer_pos() {
            if sr.dragged() || sr.clicked() || sr.is_pointer_button_down_on() {
                let f = ((pt.x - x0 - 15.0) / (x1 - x0 - 30.0)).clamp(0.0, 1.0);
                let v = ((100.0 + f * 39900.0) / 50.0).round() as u16 * 50;
                if v != cur_dpi {
                    self.dpi_edit = Some((sel, v.clamp(100, 40000), Instant::now()));
                }
            }
        }
        let shown = match self.dpi_edit {
            Some((i, v, _)) if i == sel => v,
            _ => cur_dpi,
        };
        // Mesmo estilo das outras barras: trilho grosso, roxo até o valor e puxador branco.
        let kx = x0 + 15.0 + (shown as f32 - 100.0) / 39900.0 * (x1 - x0 - 30.0);
        let pa = ui.painter();
        pa.rect_filled(r(x0, y - 5.0, x1, y + 5.0), Rounding::same(5.0), TRACK);
        pa.rect_filled(r(x0, y - 5.0, kx, y + 5.0), Rounding::same(5.0), PURPLE);
        let knob = Rect::from_center_size(p(kx, y), Vec2::new(30.0, 16.0));
        pa.rect_filled(knob.translate(Vec2::new(0.0, 1.0)), Rounding::same(8.0), Color32::from_black_alpha(90));
        pa.rect_filled(knob, Rounding::same(8.0), Color32::from_rgb(0xf2, 0xf2, 0xf2));
        label(ui, 301.0, 324.0, "100 DPI", 13.0, false, SOFT);
        ui.painter().text(p(SL_X1, 324.0), Align2::RIGHT_CENTER, "40000 DPI", font(13.0, false), SOFT);
        if sr.hovered() {
            ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        // Polling (o mesmo para todos os estágios, como no app da Akko)
        label(ui, 301.0, 371.0, t("polling_rate"), 17.0, true, TEXT);
        let cur_hz = e.polling_hz;
        let labels: Vec<String> = POLLING_HZ.iter().map(|h| format!("{h} HZ")).collect();
        let idx = POLLING_HZ.iter().position(|&h| h == cur_hz).unwrap_or(3);
        if let Some(i) = self.ticked_slider(ui, "polling", 394.0, &labels, idx) {
            let hz = POLLING_HZ[i];
            tray::send(&Comando::Polling { hz });
            if let Some(est) = &mut self.est {
                est.polling_hz = hz;
            }
        }
        if cur_hz == 8000 {
            label(ui, 301.0, 440.0, t("hz8k_warn"), 12.0, false, WARN);
        }

        // Debounce
        label(ui, 301.0, 466.0, t("debounce"), 17.0, true, TEXT);
        label(ui, 301.0, 488.0, t("debounce_hint"), 12.0, false, SOFT);
        let labels: Vec<String> = DEBOUNCE_LABELS.iter().map(|s| s.to_string()).collect();
        if let Some(i) = self.ticked_slider(ui, "debounce", 506.0, &labels, (e.debounce as usize).min(6)) {
            tray::send(&Comando::Debounce { v: i as u8 });
        }

        // Suspensão
        label(ui, 301.0, 576.0, t("sleep"), 17.0, true, TEXT);
        let labels: Vec<String> = SLEEP_MIN.iter().map(|m| format!("{m}min")).collect();
        let cur_min = (e.suspensao_s + 30) / 60;
        let idx = SLEEP_MIN.iter().enumerate().min_by_key(|(_, &m)| (m as i32 - cur_min as i32).abs()).map_or(0, |(i, _)| i);
        if let Some(i) = self.ticked_slider(ui, "sleep", 600.0, &labels, idx) {
            tray::send(&Comando::Suspensao { s: SLEEP_MIN[i] * 60 });
        }

        // Troca rápida (coluna da direita)
        label(ui, 920.0, 536.0, t("quick_dpi"), 17.0, true, TEXT);
        wrapped(ui, 920.0, 556.0, 245.0, t("quick_dpi_hint"), 12.0, SOFT);
        if let Some(on) = on_off(ui, 920.0, 611.0, e.prefs.troca_dpi) {
            self.send_prefs(|p| p.troca_dpi = on);
        }

        // Linha de baixo
        label(ui, 299.0, 674.0, t("lod"), 17.0, true, TEXT);
        for (v, text) in ["0.7mm", "1mm", "2mm"].iter().enumerate() {
            let x = 299.0 + v as f32 * 64.0;
            if pill(ui, r(x, 703.0, x + 58.0, 733.0), text, e.lod as usize == v).clicked() && e.lod as usize != v {
                tray::send(&Comando::Lod { v: v as u8 });
            }
        }
        label(ui, 502.0, 674.0, t("angle_snap"), 17.0, true, TEXT);
        if let Some(on) = on_off(ui, 502.0, 703.0, e.angle_snap) {
            tray::send(&Comando::AngleSnap { on });
        }
        label(ui, 705.0, 674.0, t("motion_sync"), 17.0, true, TEXT);
        if let Some(on) = on_off(ui, 705.0, 703.0, e.motion_sync) {
            tray::send(&Comando::MotionSync { on });
        }
        label(ui, 908.0, 674.0, t("ripple"), 17.0, true, TEXT);
        if let Some(on) = on_off(ui, 908.0, 703.0, e.ripple) {
            tray::send(&Comando::Ripple { on });
        }

        // Aviso de DPI na tela (coluna da direita, acima da troca rápida)
        label(ui, 920.0, 431.0, t("osd"), 17.0, true, TEXT);
        wrapped(ui, 920.0, 446.0, 245.0, t("osd_hint"), 12.0, SOFT);
        if let Some(on) = on_off(ui, 920.0, 486.0, e.prefs.aviso) {
            self.send_prefs(|p| p.aviso = on);
        }

        self.color_popup_ui(ctx, &e);
    }

    fn color_popup_ui(&mut self, ctx: &egui::Context, e: &Estado) {
        let Some(i) = self.color_popup else { return };
        let Some(st) = e.estagios.get(i) else { return };
        let mut c = match self.color_edit {
            Some((j, c, _)) if j == i => color32(c),
            _ => color32(st.cor),
        };
        let x = 299.0 + i as f32 * 92.0;
        let area = egui::Area::new(egui::Id::new("color_popup"))
            .order(egui::Order::Foreground)
            .fixed_pos(p(x, 268.0))
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).fill(Color32::from_rgb(0x2a, 0x2a, 0x2a)).show(ui, |ui| {
                    ui.spacing_mut().slider_width = 220.0;
                    if egui::color_picker::color_picker_color32(ui, &mut c, egui::color_picker::Alpha::Opaque) {
                        self.color_edit = Some((i, [c.r(), c.g(), c.b()], Instant::now()));
                    }
                });
            });
        self.close_on_click_outside(ctx, area.response.rect, |s| s.color_popup = None);
    }

    fn page_settings(&mut self, ui: &mut egui::Ui) {
        label(ui, 301.0, 127.0, t("settings"), 19.0, true, TEXT);
        let Some(e) = self.est.clone() else {
            self.status(ui);
            return;
        };
        label(ui, 301.0, 167.0, t("launch_startup"), 17.0, true, TEXT);
        label(ui, 301.0, 189.0, t("launch_startup_hint"), 12.0, false, SOFT);
        if let Some(on) = on_off(ui, 301.0, 207.0, e.autostart) {
            tray::send(&Comando::Autostart { on });
        }

        label(ui, 301.0, 272.0, t("akko_startup"), 17.0, true, TEXT);
        if e.akko_autostart {
            label(ui, 301.0, 294.0, t("akko_startup_on"), 12.0, false, SOFT);
            if pill(ui, r(301.0, 312.0, 411.0, 342.0), t("disable"), false).clicked() {
                tray::send(&Comando::DesativarAkko);
            }
        } else {
            label(ui, 301.0, 294.0, t("akko_startup_off"), 12.0, false, SOFT);
        }

        label(ui, 301.0, 377.0, t("osd_duration"), 17.0, true, TEXT);
        for (k, ms) in AVISO_MS.into_iter().enumerate() {
            let x = 301.0 + k as f32 * 90.0;
            let text = format!("{}s", ms as f32 / 1000.0);
            let text = if i18n::is_pt() { text.replace('.', ",") } else { text };
            if pill(ui, r(x, 397.0, x + 80.0, 427.0), &text, e.prefs.aviso_ms == ms).clicked() {
                self.send_prefs(|p| p.aviso_ms = ms);
            }
        }

        label(ui, 301.0, 482.0, t("language"), 17.0, true, TEXT);
        for (k, (code, name)) in [("en", "English"), ("pt", "Português")].into_iter().enumerate() {
            let x = 301.0 + k as f32 * 125.0;
            let cur = (code == "pt") == i18n::is_pt();
            if pill(ui, r(x, 502.0, x + 115.0, 532.0), name, cur).clicked() && !cur {
                self.send_prefs(|p| p.idioma = code.to_string());
            }
        }
    }

    fn page_about(&mut self, ui: &mut egui::Ui) {
        label(ui, 305.0, 125.0, t("about"), 19.0, true, TEXT);
        let card = r(305.0, 155.0, 1005.0, 575.0);
        ui.painter().rect_filled(card, Rounding::same(16.0), CARD);
        label(ui, 344.0, 209.0, t("specs"), 18.0, true, TEXT);
        label(ui, 344.0, 270.0, t("device_name"), 15.0, true, TEXT);
        label(ui, 344.0, 321.0, &format!("{} {}", t("software"), env!("CARGO_PKG_VERSION")), 14.0, false, SOFT);
        let e = self.est.clone().unwrap_or_default();
        let bat = if e.bateria >= 0 { format!("{}%", e.bateria) } else { "--".into() };
        label(ui, 344.0, 372.0, &format!("{}: {bat}", t("battery")), 14.0, false, SOFT);
        let hz = if e.carregado { format!("{} Hz", e.polling_hz) } else { "--".into() };
        label(ui, 344.0, 410.0, &format!("{}: {hz}", t("polling_now")), 14.0, false, SOFT);
        wrapped(ui, 344.0, 455.0, 400.0, t("about_note"), 12.0, MUTED);

        if let Some(tex) = &self.photo {
            let size = tex.size_vec2();
            let h = 256.0;
            let w = size.x / size.y * h;
            let rect = Rect::from_min_size(p(855.0 - w / 2.0, 222.0), Vec2::new(w, h));
            ui.painter().image(tex.id(), rect, Rect::from_min_max(Pos2::ZERO, p(1.0, 1.0)), Color32::WHITE);
        }
        let row = r(792.0, 490.0, 960.0, 510.0);
        let resp = ui.interact(row, ui.id().with("about_startup"), Sense::click());
        if e.autostart {
            ui.painter().circle_filled(p(800.0, 500.0), 6.0, PURPLE_LIGHT);
            ui.painter().circle_filled(p(800.0, 500.0), 2.5, TEXT);
        } else {
            ui.painter().circle_filled(p(800.0, 500.0), 6.0, Color32::from_rgb(0x55, 0x55, 0x55));
        }
        label(ui, 814.0, 500.0, t("launch_startup"), 14.0, false, if resp.hovered() { TEXT } else { SOFT });
        if resp.clicked() && tray::find_existing().is_some() {
            tray::send(&Comando::Autostart { on: !e.autostart });
        }
    }
}

/// Nome da função de um botão.
fn func_name(code: [u8; 3], e: &Estado) -> String {
    if code == MACRO_CODE {
        return format!("{}: {}", t("macro"), e.prefs.macro_no_mouse.clone().unwrap_or_else(|| "?".into()));
    }
    if let Some(f) = BTN_FUNCS.iter().find(|f| f.2 == code) {
        return t(f.1).to_string();
    }
    if code[0] == 0x20 {
        let mods = keys::modifiers_name(code[1]);
        if code[2] == 0 {
            return mods;
        }
        let key = keys::hid_name(code[2]);
        return if mods.is_empty() { key } else { format!("{mods}+{key}") };
    }
    format!("{:02x} {:02x} {:02x}", code[0], code[1], code[2])
}


impl App {
    /* ---------------- Button Assignment ---------------- */

    fn page_buttons(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        // Com um botão selecionado, o mouse vai para a esquerda e o painel de funções aparece à
        // direita (como no app da Akko).
        let dx = if self.btn_menu.is_some() { -275.0 } else { 0.0 };
        let sh = |x0: f32, y0: f32, x1: f32, y1: f32| r(x0 + dx, y0, x1 + dx, y1);
        if let Some(tex) = &self.mouse_top {
            let size = tex.size_vec2();
            // um pouco menor que a foto original, para não encostar no texto de baixo
            let h = 320.0;
            let w = size.x / size.y * h;
            let rect = Rect::from_center_size(p(715.0 + dx, 398.0), Vec2::new(w, h));
            ui.painter().image(tex.id(), rect, Rect::from_min_max(Pos2::ZERO, p(1.0, 1.0)), Color32::WHITE);
        }
        if !self.status(ui) {
            return;
        }
        let Some(e) = self.est.clone() else { return };
        if e.botoes.len() < BUTTONS {
            wrapped(ui, 301.0, 160.0, 600.0, t("buttons_unread"), 15.0, WARN);
            return;
        }
        let mut hit_something = false;
        for (i, b, d) in BTN_POS {
            let rect = sh(b[0], b[1], b[2], b[3]);
            let code = e.botoes[i];
            let capturing = matches!(self.key_capture, Some((j, _)) if j == i);
            let open = self.btn_menu == Some(i);
            let resp = ui.interact(rect, ui.id().with(("btn", i)), Sense::click());
            let fill = if open || capturing { PURPLE } else if resp.hovered() { PILL_HOVER } else { PILL };
            ui.painter().rect(rect, Rounding::same(7.0), fill, Stroke::new(1.0_f32, BORDER));
            let text = if capturing { t("press_key").to_string() } else { func_name(code, &e) };
            let size = if text.chars().count() > 15 { 12.5 } else { 15.0 };
            ui.painter().text(rect.center(), Align2::CENTER_CENTER, &text, font(size, true), TEXT);
            // bolinha como no Akko (roxa no botão selecionado ou com função mudada)
            let dot = p(d[0] + dx, d[1]);
            if open {
                ui.painter().circle_stroke(dot, 6.0, Stroke::new(1.5_f32, PURPLE_LIGHT));
                ui.painter().circle_filled(dot, 3.0, PURPLE_LIGHT);
            } else {
                ui.painter().circle_stroke(dot, 6.0, Stroke::new(1.5_f32, MUTED));
                if code != DEFAULT_BUTTONS[i] {
                    ui.painter().circle_filled(dot, 3.0, MUTED);
                }
            }
            if resp.clicked() {
                hit_something = true;
                self.key_capture = None;
                if open {
                    self.btn_menu = None;
                } else {
                    self.btn_menu = Some(i);
                    self.btn_section = section_of(code);
                    self.panel_scroll = 0.0;
                    self.macro_dropdown = false;
                }
            }
        }

        // Trocar esquerdo e direito
        let swapped = e.botoes[0] == DEFAULT_BUTTONS[1] && e.botoes[1] == DEFAULT_BUTTONS[0];
        let row = sh(659.0, 586.0, 960.0, 608.0);
        let rr = ui.interact(row, ui.id().with("swap"), Sense::click());
        let sd = p(667.0 + dx, 597.0);
        if swapped {
            ui.painter().circle_filled(sd, 6.0, PURPLE_LIGHT);
            ui.painter().circle_filled(sd, 2.5, TEXT);
        } else {
            ui.painter().circle_filled(sd, 6.0, Color32::from_rgb(0x55, 0x55, 0x55));
        }
        label(ui, 686.0 + dx, 597.0, t("swap_lr"), 16.0, true, if rr.hovered() { TEXT } else { SOFT });
        if rr.clicked() {
            hit_something = true;
            tray::send(&Comando::TrocarEsqDir);
        }

        // Restaurar padrão
        let rb = sh(661.0, 626.0, 790.0, 657.0);
        let rbr = ui.interact(rb, ui.id().with("btn_restore"), Sense::click());
        ui.painter().rect(rb, Rounding::same(6.0), if rbr.hovered() { PILL_HOVER } else { PILL }, Stroke::new(1.0_f32, BORDER));
        ui.painter().text(rb.center(), Align2::CENTER_CENTER, t("restore_defaults"), font(14.0, false), TEXT);
        if rbr.clicked() {
            hit_something = true;
            tray::send(&Comando::BotoesPadrao);
        }

        self.key_capture_poll(ctx);
        let panel = r(795.0, 68.0, 1166.0, 736.0);
        if self.btn_menu.is_some() {
            self.btn_panel(ui, ctx, &e, panel);
            // clique fora do painel e dos botões fecha a seleção
            let outside = ctx.input(|inp| {
                inp.pointer.primary_clicked() && inp.pointer.interact_pos().map_or(false, |pt| !panel.contains(pt) && pt.y > 100.0)
            });
            if outside && !hit_something && self.key_capture.is_none() {
                self.btn_menu = None;
            }
        }
    }

    /// Captura de tecla para um botão: a primeira tecla (com Ctrl/Shift/Alt/Win segurados).
    /// Um clique do mouse cancela.
    fn key_capture_poll(&mut self, ctx: &egui::Context) {
        let Some((i, mut prev)) = self.key_capture.take() else { return };
        ctx.request_repaint();
        if ctx.input(|inp| inp.pointer.any_pressed()) {
            return;
        }
        for (hid, down) in keys::changes(&mut prev) {
            if down && !keys::is_modifier(hid) {
                let cur = self.est.as_ref().and_then(|e| e.botoes.get(i).copied()).unwrap_or([0; 3]);
                let marked = if cur[0] == 0x20 { cur[1] } else { 0 };
                let mods = keys::held_modifiers(&prev) | marked;
                tray::send(&Comando::Botao { i, codigo: [0x20, mods, hid] });
                return;
            }
        }
        self.key_capture = Some((i, prev));
    }

    /// Painel "Preset Buttons" (como no app da Akko): seções que abrem e fecham, uma por vez.
    /// Desenhado em posições absolutas, com rolagem própria pela roda do mouse.
    fn btn_panel(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, e: &Estado, panel: Rect) {
        let Some(i) = self.btn_menu else { return };
        let cur = e.botoes[i];
        ui.painter().rect_filled(panel, Rounding::same(16.0), PANEL);
        label(ui, 822.0, 99.0, t("preset_buttons"), 17.0, true, TEXT);

        // rolagem
        let view = r(panel.left(), 124.0, panel.right(), panel.bottom() - 8.0);
        if ctx.input(|inp| inp.pointer.hover_pos().map_or(false, |pt| panel.contains(pt))) {
            self.panel_scroll -= ctx.input(|inp| inp.smooth_scroll_delta.y);
        }
        self.panel_scroll = self.panel_scroll.clamp(0.0, (self.panel_height - view.height()).max(0.0));
        let pa = ui.painter().with_clip_rect(view);
        let top = view.top() + 28.0 - self.panel_scroll;
        let mut y = top;
        let visible = |y: f32| y > view.top() + 6.0 && y < view.bottom() - 6.0;
        let text = |y: f32, x: f32, s: &str, size: f32, bold: bool, c: Color32| {
            pa.text(p(x, y), Align2::LEFT_CENTER, s, font(size, bold), c);
        };
        let dot = |y: f32, x: f32, sel: bool| {
            if sel {
                pa.circle_stroke(p(x, y), 5.5, Stroke::new(1.5_f32, PURPLE_LIGHT));
                pa.circle_filled(p(x, y), 2.8, PURPLE_LIGHT);
            } else {
                pa.circle_filled(p(x, y), 6.0, Color32::from_rgb(0x4a, 0x4a, 0x4a));
            }
        };
        let mut send: Option<Comando> = None;

        for (k, &sec) in PANEL_SECTIONS.iter().enumerate() {
            let open = self.btn_section == k;
            let head = r(812.0, y - 14.0, 1150.0, y + 14.0);
            let hr = visible(y).then(|| ui.interact(head, ui.id().with(("sec", k)), Sense::click()));
            let tri = if open {
                vec![p(822.0, y - 4.0), p(831.0, y - 4.0), p(826.5, y + 3.0)]
            } else {
                vec![p(823.0, y - 5.0), p(830.0, y), p(823.0, y + 5.0)]
            };
            pa.add(Shape::convex_polygon(tri, TEXT, Stroke::NONE));
            let hov = hr.as_ref().map_or(false, |r| r.hovered());
            text(y, 846.0, t(sec), 15.0, false, if hov { TEXT } else { SOFT });
            if hr.map_or(false, |r| r.clicked()) {
                self.btn_section = if open { usize::MAX } else { k };
                self.macro_dropdown = false;
            }
            if !open {
                y += 56.0;
                continue;
            }
            y += 38.0;
            match sec {
                "keyboard_keys" => {
                    let is_key = cur[0] == 0x20 && (cur[1] != 0 || cur[2] != 0);
                    let mods = if is_key { cur[1] } else { 0 };
                    let key = if is_key { cur[2] } else { 0 };
                    // modificadores em duas linhas de quatro
                    for (n, (name, bit)) in MODS.iter().enumerate() {
                        let (cx, cy) = (818.0 + (n % 4) as f32 * 86.0, y + (n / 4) as f32 * 30.0);
                        let on = mods & bit != 0;
                        let rr = visible(cy).then(|| ui.interact(r(cx - 9.0, cy - 12.0, cx + 76.0, cy + 12.0), ui.id().with(("mod", n)), Sense::click()));
                        dot(cy, cx, on);
                        text(cy, cx + 12.0, name, 13.0, true, TEXT);
                        if rr.map_or(false, |r| r.clicked()) {
                            send = Some(Comando::Botao { i, codigo: [0x20, mods ^ bit, key] });
                        }
                    }
                    let py = y + 72.0;
                    pa.line_segment([p(822.0, py), p(834.0, py)], Stroke::new(2.0_f32, TEXT));
                    pa.line_segment([p(828.0, py - 6.0), p(828.0, py + 6.0)], Stroke::new(2.0_f32, TEXT));
                    text(y + 108.0, 812.0, t("press_a_key"), 14.0, true, TEXT);
                    let bx = r(812.0, y + 123.0, 1062.0, y + 153.0);
                    let capturing = matches!(self.key_capture, Some((j, _)) if j == i);
                    pa.rect(bx, Rounding::same(2.0), Color32::BLACK, Stroke::new(1.0_f32, if capturing { PURPLE_LIGHT } else { Color32::BLACK }));
                    let shown = if capturing { "...".to_string() } else if key != 0 { keys::hid_name(key) } else { String::new() };
                    text(bx.center().y, bx.left() + 10.0, &shown, 14.0, true, TEXT);
                    let br = visible(bx.center().y).then(|| ui.interact(bx, ui.id().with("keybox"), Sense::click()));
                    if br.map_or(false, |r| r.clicked()) {
                        self.key_capture = Some((i, keys::snapshot()));
                    }
                    y += 153.0;
                }
                "macro_buttons" => {
                    let gal = pa.layout(t("macro_description").to_string(), font(13.0, false), SOFT, 300.0);
                    let gh = gal.size().y;
                    pa.galley(p(812.0, y - 10.0), gal, SOFT);
                    y += gh + 4.0;
                    let dd = r(812.0, y, 1135.0, y + 36.0);
                    let ddr = visible(dd.center().y).then(|| ui.interact(dd, ui.id().with("macro_dd"), Sense::click()));
                    pa.rect_stroke(dd, Rounding::same(18.0), Stroke::new(1.0_f32, BORDER));
                    let sel_name = (cur == MACRO_CODE).then(|| e.prefs.macro_no_mouse.clone()).flatten();
                    let dd_text = sel_name.clone().unwrap_or_else(|| t("select_macro").to_string());
                    text(dd.center().y, 830.0, &dd_text, 15.0, true, TEXT);
                    let cx = dd.right() - 24.0;
                    let cy = dd.center().y;
                    pa.add(Shape::line(vec![p(cx - 5.0, cy - 2.5), p(cx, cy + 2.5), p(cx + 5.0, cy - 2.5)], Stroke::new(1.4_f32, TEXT)));
                    if ddr.map_or(false, |r| r.clicked()) {
                        self.macro_dropdown = !self.macro_dropdown;
                    }
                    y += 36.0;
                    if self.macro_dropdown {
                        y += 14.0;
                        if e.prefs.macros.is_empty() {
                            text(y, 830.0, t("no_macros"), 12.0, false, MUTED);
                            y += 25.0;
                        }
                        for (n, m) in e.prefs.macros.iter().enumerate() {
                            let sel = sel_name.as_deref() == Some(m.nome.as_str());
                            let rr = visible(y).then(|| ui.interact(r(830.0, y - 12.0, 1150.0, y + 12.0), ui.id().with(("mac", n)), Sense::click()));
                            dot(y, 840.0, sel);
                            let hov = rr.as_ref().map_or(false, |r| r.hovered());
                            text(y, 859.0, &m.nome, 14.0, false, if hov || sel { TEXT } else { SOFT });
                            if rr.map_or(false, |r| r.clicked()) && !m.eventos.is_empty() {
                                send = Some(Comando::BotaoMacro { i, nome: m.nome.clone() });
                                self.macro_dropdown = false;
                            }
                            y += 25.0;
                        }
                        y -= 25.0;
                    }
                }
                _ => {
                    let mut n = 0;
                    for &(fsec, name, code) in BTN_FUNCS {
                        if fsec != sec {
                            continue;
                        }
                        let sel = cur == code;
                        let rr = visible(y).then(|| ui.interact(r(830.0, y - 12.0, 1150.0, y + 12.0), ui.id().with(("item", k, n)), Sense::click()));
                        dot(y, 840.0, sel);
                        let hov = rr.as_ref().map_or(false, |r| r.hovered());
                        text(y, 859.0, t(name), 14.0, false, if hov || sel { TEXT } else { SOFT });
                        if rr.map_or(false, |r| r.clicked()) {
                            send = Some(Comando::Botao { i, codigo: code });
                        }
                        y += 25.0;
                        n += 1;
                    }
                    y -= 25.0;
                }
            }
            y += 56.0;
        }
        self.panel_height = y - top + 10.0;
        if let Some(c) = send {
            tray::send(&c);
        }
    }

    /* ---------------- Games (DPI por jogo) ---------------- */

    fn page_games(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        label(ui, 301.0, 127.0, t("games"), 19.0, true, TEXT);
        let Some(e) = self.est.clone() else {
            self.status(ui);
            return;
        };
        wrapped(ui, 301.0, 145.0, 820.0, t("games_hint"), 12.0, SOFT);

        label(ui, 301.0, 197.0, t("game_return"), 15.0, true, TEXT);
        if let Some(on) = on_off(ui, 301.0, 214.0, e.prefs.jogo_voltar) {
            self.send_prefs(|p| p.jogo_voltar = on);
        }

        // Lista de jogos: executável, título e o estágio de DPI (chips como na Performance)
        let mut y = 272.0;
        if e.prefs.jogos.is_empty() {
            label(ui, 301.0, y + 18.0, t("no_games"), 13.0, false, MUTED);
            y += 44.0;
        }
        let mut remove: Option<usize> = None;
        let mut set_stage: Option<(usize, usize)> = None;
        for (k, j) in e.prefs.jogos.iter().enumerate() {
            if y > 640.0 {
                break;
            }
            let row = r(301.0, y, 1150.0, y + 48.0);
            ui.painter().rect_filled(row, Rounding::same(10.0), CARD);
            label(ui, 318.0, y + 17.0, &j.exe, 14.0, true, TEXT);
            let title: String = j.nome.chars().take(38).collect();
            label(ui, 318.0, y + 34.0, &title, 11.0, false, MUTED);
            for (i, st) in e.estagios.iter().enumerate() {
                let x = 640.0 + i as f32 * 92.0;
                let chip = r(x, y + 9.0, x + 80.0, y + 39.0);
                let resp = ui.interact(chip, ui.id().with(("game_chip", k, i)), Sense::click());
                let sel = j.estagio == i;
                ui.painter().rect_filled(chip, Rounding::same(6.0), if resp.hovered() { PILL } else { BG });
                if sel {
                    ui.painter().rect_stroke(chip, Rounding::same(6.0), Stroke::new(1.0_f32, color32(st.cor)));
                }
                ui.painter().rect_filled(Rect::from_center_size(p(x + 15.0, y + 24.0), Vec2::splat(12.0)), Rounding::same(2.0), color32(st.cor));
                label(ui, x + 27.0, y + 24.0, &st.dpi.to_string(), 14.0, false, if sel { TEXT } else { SOFT });
                if resp.clicked() && !sel {
                    set_stage = Some((k, i));
                }
            }
            let del = Rect::from_center_size(p(1126.0, y + 24.0), Vec2::splat(26.0));
            let dr = ui.interact(del, ui.id().with(("game_del", k)), Sense::click()).on_hover_text(t("delete"));
            if dr.hovered() {
                ui.painter().rect_filled(del, Rounding::same(5.0), PILL_HOVER);
            }
            let s = Stroke::new(1.5_f32, SOFT);
            ui.painter().line_segment([del.center() + Vec2::new(-5.0, -5.0), del.center() + Vec2::new(5.0, 5.0)], s);
            ui.painter().line_segment([del.center() + Vec2::new(5.0, -5.0), del.center() + Vec2::new(-5.0, 5.0)], s);
            if dr.clicked() {
                remove = Some(k);
            }
            y += 56.0;
        }
        if let Some((k, i)) = set_stage {
            self.send_prefs(|p| p.jogos[k].estagio = i);
        }
        if let Some(k) = remove {
            self.send_prefs(|p| {
                p.jogos.remove(k);
            });
        }

        // Adicionar jogo: escolher entre os programas abertos ou digitar o executável
        let add = r(301.0, y + 6.0, 461.0, y + 36.0);
        if pill(ui, add, t("add_game"), self.game_picker.is_some()).clicked() {
            self.game_picker = if self.game_picker.is_some() { None } else { Some(games::window_apps()) };
            self.popup_just_opened = true;
        }
        self.game_picker_ui(ctx, &e, p(301.0, (y + 42.0).min(H - 330.0)));
    }

    fn game_picker_ui(&mut self, ctx: &egui::Context, e: &Estado, pos: Pos2) {
        let Some(apps) = self.game_picker.clone() else { return };
        let mut add: Option<(String, String)> = None;
        let area = egui::Area::new(egui::Id::new("game_picker")).order(egui::Order::Foreground).fixed_pos(pos).show(ctx, |ui| {
            egui::Frame::popup(ui.style()).fill(Color32::from_rgb(0x2a, 0x2a, 0x2a)).show(ui, |ui| {
                ui.set_width(460.0);
                ui.label(egui::RichText::new(t("pick_game")).size(12.0).color(SOFT));
                ui.add_space(4.0);
                egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                    for (exe, title) in &apps {
                        let known = e.prefs.jogos.iter().any(|j| &j.exe == exe);
                        let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 34.0), Sense::click());
                        if resp.hovered() && !known {
                            ui.painter().rect_filled(rect, Rounding::same(5.0), PILL_HOVER);
                        }
                        let c = if known { MUTED } else { TEXT };
                        ui.painter().text(rect.left_center() + Vec2::new(10.0, -7.0), Align2::LEFT_CENTER, exe, font(13.0, true), c);
                        let short: String = title.chars().take(60).collect();
                        ui.painter().text(rect.left_center() + Vec2::new(10.0, 8.0), Align2::LEFT_CENTER, short, font(11.0, false), MUTED);
                        if resp.clicked() && !known {
                            add = Some((exe.clone(), title.clone()));
                        }
                    }
                });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(t("type_exe")).size(12.0).color(SOFT));
                    let te = ui.add(egui::TextEdit::singleline(&mut self.game_exe_text).hint_text("game.exe").desired_width(200.0));
                    let go = ui.button(t("add")).clicked() || te.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    let exe = self.game_exe_text.trim().to_lowercase();
                    if go && !exe.is_empty() {
                        let exe = if exe.ends_with(".exe") { exe } else { format!("{exe}.exe") };
                        add = Some((exe, String::new()));
                    }
                });
            });
        });
        if let Some((exe, nome)) = add {
            if !e.prefs.jogos.iter().any(|j| j.exe == exe) {
                // começa no estágio ativo agora (dá para trocar nos chips)
                let estagio = e.ativo;
                self.send_prefs(|p| p.jogos.push(RegraJogo { exe, nome, estagio }));
            }
            self.game_picker = None;
            self.game_exe_text.clear();
            return;
        }
        self.close_on_click_outside(ctx, area.response.rect, |s| s.game_picker = None);
    }

    /* ---------------- Macro ---------------- */

    fn page_macro(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        label(ui, 299.0, 127.0, t("macro_manager"), 19.0, true, TEXT);
        let Some(e) = self.est.clone() else {
            self.status(ui);
            return;
        };
        let macros = e.prefs.macros.clone();
        if self.macro_sel.map_or(false, |k| k >= macros.len()) {
            self.macro_sel = None;
        }

        // Lista
        let left = r(298.0, 158.0, 538.0, 636.0);
        ui.painter().rect_filled(left, Rounding::same(14.0), PANEL);
        label(ui, 318.0, 192.0, t("macro_list"), 17.0, true, TEXT);
        let plus = Rect::from_center_size(p(510.0, 192.0), Vec2::splat(26.0));
        let pr = ui.interact(plus, ui.id().with("macro_new"), Sense::click()).on_hover_text(t("new_macro"));
        if pr.hovered() {
            ui.painter().rect_filled(plus, Rounding::same(5.0), PILL_HOVER);
        }
        ui.painter().line_segment([plus.center() - Vec2::new(7.0, 0.0), plus.center() + Vec2::new(7.0, 0.0)], Stroke::new(1.8_f32, TEXT));
        ui.painter().line_segment([plus.center() - Vec2::new(0.0, 7.0), plus.center() + Vec2::new(0.0, 7.0)], Stroke::new(1.8_f32, TEXT));
        if pr.clicked() {
            self.stop_recording();
            let mut n = macros.len() + 1;
            while macros.iter().any(|m| m.nome == format!("Macro {n}")) {
                n += 1;
            }
            let nome = format!("Macro {n}");
            let mut list = macros.clone();
            list.push(Macro { nome: nome.clone(), eventos: Vec::new() });
            self.macro_sel = Some(list.len() - 1);
            self.macro_name = nome;
            self.send_prefs(|p| p.macros = list);
        }
        for (k, m) in macros.iter().enumerate() {
            let y = 218.0 + k as f32 * 38.0;
            if y > 590.0 {
                break;
            }
            let item = r(308.0, y, 528.0, y + 34.0);
            let resp = ui.interact(item, ui.id().with(("macro_item", k)), Sense::click());
            let sel = self.macro_sel == Some(k);
            if sel {
                ui.painter().rect_filled(item, Rounding::same(7.0), PURPLE);
            } else if resp.hovered() {
                ui.painter().rect_filled(item, Rounding::same(7.0), PILL_HOVER);
            }
            label(ui, item.left() + 12.0, item.center().y, &m.nome, 14.0, false, TEXT);
            if e.prefs.macro_no_mouse.as_deref() == Some(m.nome.as_str()) {
                ui.painter().circle_filled(p(item.right() - 14.0, item.center().y), 4.0, if sel { TEXT } else { PURPLE_LIGHT });
            }
            if resp.clicked() && !sel {
                self.stop_recording();
                self.macro_sel = Some(k);
                self.macro_name = m.nome.clone();
            }
        }

        // Detalhes
        let right = r(565.0, 158.0, 1152.0, 563.0);
        ui.painter().rect_filled(right, Rounding::same(14.0), PANEL);
        label(ui, 595.0, 192.0, t("macro_details"), 17.0, true, TEXT);
        // A lista pode ter mudado neste quadro (+ cria um macro novo): relê.
        let macros = self.est.as_ref().map(|e| e.prefs.macros.clone()).unwrap_or_default();
        let Some((k, m)) = self.macro_sel.and_then(|k| macros.get(k).cloned().map(|m| (k, m))) else {
            wrapped(ui, 595.0, 230.0, 520.0, t("macro_pick"), 13.0, MUTED);
            wrapped(ui, 299.0, 650.0, 850.0, t("macro_hint"), 12.0, MUTED);
            return;
        };
        if pill(ui, r(1040.0, 178.0, 1132.0, 206.0), t("clear"), false).clicked() {
            self.stop_recording();
            let mut list = macros.clone();
            list[k].eventos.clear();
            self.send_prefs(|p| p.macros = list);
        }
        let events: Vec<EventoMacro> = match &self.recording {
            Some(rec) => rec.events.clone(),
            None => m.eventos.clone(),
        };
        let list_rect = r(585.0, 216.0, 1135.0, 552.0);
        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(list_rect), |ui| {
            egui::ScrollArea::vertical().id_salt("macro_events").stick_to_bottom(true).show(ui, |ui| {
                ui.set_width(list_rect.width());
                for (n, ev) in events.iter().enumerate() {
                    let (row, _) = ui.allocate_exact_size(Vec2::new(list_rect.width() - 12.0, 28.0), Sense::hover());
                    if n % 2 == 0 {
                        ui.painter().rect_filled(row, Rounding::same(5.0), Color32::from_rgb(0x20, 0x20, 0x20));
                    }
                    let y = row.center().y;
                    let (txt, col) = if ev.down { (t("press"), PURPLE_LIGHT) } else { (t("release"), SOFT) };
                    ui.painter().text(p(row.left() + 12.0, y), Align2::LEFT_CENTER, format!("{}", n + 1), font(12.0, false), MUTED);
                    ui.painter().text(p(row.left() + 48.0, y), Align2::LEFT_CENTER, txt, font(13.0, false), col);
                    ui.painter().text(p(row.left() + 170.0, y), Align2::LEFT_CENTER, keys::hid_name(ev.tecla), font(14.0, true), TEXT);
                    if n + 1 < events.len() {
                        ui.painter().text(p(row.right() - 12.0, y), Align2::RIGHT_CENTER, format!("{} ms", ev.atraso_ms), font(13.0, false), SOFT);
                    }
                }
                if events.is_empty() && self.recording.is_none() {
                    ui.label(egui::RichText::new(t("macro_empty")).size(13.0).color(MUTED));
                }
            });
        });

        // Barra de baixo: gravar, nome, apagar
        let bar = r(565.0, 580.0, 1152.0, 636.0);
        ui.painter().rect_filled(bar, Rounding::same(14.0), PANEL);
        let rec_rect = r(590.0, 594.0, 720.0, 622.0);
        let rr = ui.interact(rec_rect, ui.id().with("macro_rec"), Sense::click());
        if rr.hovered() {
            ui.painter().rect_filled(rec_rect, Rounding::same(6.0), PILL_HOVER);
        }
        let recording = self.recording.is_some();
        if recording {
            ui.painter().circle_filled(p(606.0, 608.0), 6.0, Color32::from_rgb(0xe0, 0x40, 0x40));
        } else {
            ui.painter().circle_stroke(p(606.0, 608.0), 6.5, Stroke::new(1.4_f32, PURPLE_LIGHT));
            ui.painter().add(Shape::convex_polygon(
                vec![p(604.0, 605.0), p(609.0, 608.0), p(604.0, 611.0)],
                PURPLE_LIGHT,
                Stroke::NONE,
            ));
        }
        let rec_text = if recording { format!("{} ({}/{})", t("stop"), events.len(), MACRO_MAX_EVENTS) } else { t("record").to_string() };
        label(ui, 621.0, 608.0, &rec_text, 15.0, true, TEXT);
        if rr.clicked() {
            if recording {
                self.stop_recording();
            } else {
                self.recording = Some(Recording { prev: keys::snapshot(), last: Instant::now(), events: Vec::new() });
            }
        }
        label(ui, 745.0, 608.0, t("name"), 14.0, false, SOFT);
        let name_rect = r(800.0, 595.0, 990.0, 621.0);
        ui.painter().rect_stroke(name_rect, Rounding::same(6.0), Stroke::new(1.0_f32, BORDER));
        let te = egui::TextEdit::singleline(&mut self.macro_name)
            .frame(false)
            .margin(egui::Margin::symmetric(8.0, 0.0))
            .min_size(name_rect.size())
            .vertical_align(egui::Align::Center)
            .font(font(14.0, false))
            .text_color(TEXT);
        let nr = ui.put(name_rect, te);
        if nr.lost_focus() {
            let nome = self.macro_name.trim().to_string();
            if !nome.is_empty() && nome != m.nome && !macros.iter().any(|x| x.nome == nome) {
                let mut list = macros.clone();
                list[k].nome = nome;
                self.send_prefs(|p| p.macros = list);
            } else {
                self.macro_name = m.nome.clone();
            }
        }
        if pill(ui, r(1040.0, 593.0, 1132.0, 623.0), t("delete"), false).clicked() {
            self.stop_recording();
            let mut list = macros.clone();
            list.remove(k);
            self.macro_sel = None;
            self.send_prefs(|p| p.macros = list);
        }
        wrapped(ui, 299.0, 650.0, 850.0, t("macro_hint"), 12.0, MUTED);

        self.record_poll(ctx);
    }

    /// Lê o teclado durante a gravação do macro.
    fn record_poll(&mut self, ctx: &egui::Context) {
        let Some(rec) = &mut self.recording else { return };
        ctx.request_repaint();
        for (hid, down) in keys::changes(&mut rec.prev) {
            if rec.events.len() >= MACRO_MAX_EVENTS {
                break;
            }
            // começa sempre por uma tecla pressionada
            if rec.events.is_empty() && !down {
                continue;
            }
            let now = Instant::now();
            if let Some(last) = rec.events.last_mut() {
                last.atraso_ms = now.duration_since(rec.last).as_millis().min(65_535) as u16;
            }
            rec.last = now;
            rec.events.push(EventoMacro { down, tecla: hid, atraso_ms: 0 });
        }
    }

    /// Termina a gravação: solta as teclas que ficaram pressionadas e salva no macro.
    fn stop_recording(&mut self) {
        let Some(mut rec) = self.recording.take() else { return };
        let Some(k) = self.macro_sel else { return };
        let mut held: Vec<u8> = Vec::new();
        for ev in &rec.events {
            if ev.down {
                held.push(ev.tecla);
            } else {
                held.retain(|&h| h != ev.tecla);
            }
        }
        for h in held {
            if rec.events.len() >= MACRO_MAX_EVENTS {
                rec.events.pop();
            }
            rec.events.push(EventoMacro { down: false, tecla: h, atraso_ms: 0 });
        }
        if let Some(last) = rec.events.last_mut() {
            last.atraso_ms = 0;
        }
        let Some(e) = &self.est else { return };
        let mut list = e.prefs.macros.clone();
        if let Some(m) = list.get_mut(k) {
            m.eventos = rec.events;
            self.send_prefs(|p| p.macros = list);
        }
    }
}

impl eframe::App for App {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        Color32::from(BG).to_normalized_gamma_f32()
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.refresh();
        ctx.request_repaint_after(Duration::from_millis(300));

        egui::CentralPanel::default().frame(egui::Frame::none().fill(BG)).show(ctx, |ui| {
            self.header(ui, ctx);
            if !(self.page == Page::Buttons && self.btn_menu.is_some()) {
                self.sidebar(ui);
            }
            match self.page {
                Page::Buttons => self.page_buttons(ui, ctx),
                Page::Macro => self.page_macro(ui, ctx),
                Page::Games => self.page_games(ui, ctx),
                Page::Performance => self.page_performance(ui, ctx),
                Page::Settings => self.page_settings(ui),
                Page::About => self.page_about(ui),
            }
            // Borda fina (a janela não tem moldura do Windows)
            ui.painter().rect_stroke(r(0.5, 0.5, W - 0.5, H - 0.5), Rounding::ZERO, Stroke::new(1.0_f32, Color32::from_rgb(0x33, 0x33, 0x33)));
        });
        self.lang_popup(ctx);
    }
}

/// Ícone do programa: quadrado roxo arredondado com um mouse branco.
pub fn app_icon_rgba(size: u32) -> Vec<u8> {
    let s = size as f32;
    let mut px = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            let r = s * 0.22;
            let dx = (fx - fx.clamp(r, s - r)).abs();
            let dy = (fy - fy.clamp(r, s - r)).abs();
            let inside = (dx * dx + dy * dy).sqrt() <= r;
            if !inside {
                continue;
            }
            let i = ((y * size + x) * 4) as usize;
            // mouse: elipse branca com a divisão dos botões
            let (cx, cy, ex, ey) = (s * 0.5, s * 0.52, s * 0.23, s * 0.32);
            let e = ((fx - cx) / ex).powi(2) + ((fy - cy) / ey).powi(2);
            let line = (fx - cx).abs() < s * 0.025 && fy < cy - ey * 0.15 || (fy - (cy - ey * 0.15)).abs() < s * 0.025 && e < 1.0;
            let white = e <= 1.0 && !line;
            let c = if white { [255, 255, 255] } else { [0x61, 0x33, 0xc9] };
            px[i..i + 4].copy_from_slice(&[c[0], c[1], c[2], 255]);
        }
    }
    px
}

pub fn run() {
    use windows_sys::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
    use windows_sys::Win32::System::Threading::CreateMutexW;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    // A prévia (AKKO_ECHO_PREVIEW) pode abrir junto com a janela de verdade.
    let preview = std::env::var_os("AKKO_ECHO_PREVIEW").is_some();
    unsafe {
        CreateMutexW(std::ptr::null(), 0, crate::mouse::wide_str("Local\\AkkoEchoUltra.Janela").as_ptr());
        if GetLastError() == ERROR_ALREADY_EXISTS && !preview {
            let h = FindWindowW(std::ptr::null(), crate::mouse::wide_str("Akko Echo Ultra").as_ptr());
            if !h.is_null() {
                ShowWindow(h, SW_RESTORE);
                SetForegroundWindow(h);
            }
            return;
        }
    }
    let icon = egui::IconData { rgba: app_icon_rgba(64), width: 64, height: 64 };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Akko Echo Ultra")
            .with_inner_size([W, H])
            .with_resizable(false)
            .with_decorations(false)
            .with_icon(Arc::new(icon)),
        centered: true,
        ..Default::default()
    };
    if let Err(e) = eframe::run_native("Akko Echo Ultra", options, Box::new(|cc| Ok(Box::new(App::new(cc))))) {
        crate::log(&format!("janela: erro ao abrir: {e}"));
    }
}
