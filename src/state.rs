//! Preferências do programa e estado publicado pela bandeja para a janela de configuração.
//! Ficam em %LOCALAPPDATA%\Akko Echo Ultra\ (prefs.json e state.json).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub fn data_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir());
    let dir = base.join("Akko Echo Ultra");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Prefs {
    /// Polling de cada estágio, em Hz (0 = não muda ao trocar para ele).
    pub polling_por_estagio: [u32; 4],
    /// Troca de DPI ao pressionar esquerdo + meio + direito juntos.
    pub troca_dpi: bool,
    /// Aviso na tela ao trocar de DPI.
    pub aviso: bool,
    pub aviso_ms: u32,
    /// "en" ou "pt"
    pub idioma: String,
    /// Últimos estágios bons (DPI e cor), para restaurar se o firmware zerar algum.
    pub estagios_salvos: Vec<Estagio>,
    /// Macros gravados neste PC (o mouse guarda um por vez: o que está num botão).
    pub macros: Vec<Macro>,
    /// Nome do macro que está gravado no mouse agora.
    pub macro_no_mouse: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
pub struct Macro {
    pub nome: String,
    pub eventos: Vec<EventoMacro>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Default)]
pub struct EventoMacro {
    /// true = pressionar, false = soltar
    pub down: bool,
    /// tecla (uso HID do teclado)
    pub tecla: u8,
    /// tempo até o próximo evento, em ms
    pub atraso_ms: u16,
}

impl Macro {
    pub fn keys(&self) -> Vec<crate::mouse::MacroKey> {
        self.eventos.iter().map(|e| crate::mouse::MacroKey { down: e.down, key: e.tecla, delay_ms: e.atraso_ms }).collect()
    }
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs {
            polling_por_estagio: [4000, 1000, 1000, 1000],
            troca_dpi: true,
            aviso: true,
            aviso_ms: 1500,
            idioma: "en".into(),
            estagios_salvos: Vec::new(),
            macros: Vec::new(),
            macro_no_mouse: None,
        }
    }
}

impl Prefs {
    pub fn load() -> Prefs {
        let mut p: Prefs = std::fs::read_to_string(data_dir().join("prefs.json"))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        for hz in p.polling_por_estagio.iter_mut() {
            if *hz != 0 && crate::mouse::polling_code(*hz).is_none() {
                *hz = 1000;
            }
        }
        if p.idioma != "pt" {
            p.idioma = "en".into();
        }
        p
    }
    pub fn save(&self) {
        write_atomic("prefs.json", &serde_json::to_string_pretty(self).unwrap_or_default());
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Estagio {
    pub dpi: u16,
    pub cor: [u8; 3],
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct Estado {
    pub versao: u64,
    /// "conectado", "procurando" ou "pausado" (Akko aberto)
    pub status: String,
    pub bateria: i32,
    pub carregado: bool,
    pub ativo: usize,
    pub estagios: Vec<Estagio>,
    pub polling_hz: u32,
    pub lod: u8,
    pub debounce: u8,
    pub suspensao_s: u16,
    pub angle_snap: bool,
    pub ripple: bool,
    pub motion_sync: bool,
    /// Códigos dos 5 botões (vazio se a tabela não foi lida).
    pub botoes: Vec<[u8; 3]>,
    pub prefs: Prefs,
    pub autostart: bool,
    pub akko_autostart: bool,
}

impl Estado {
    pub fn load() -> Option<Estado> {
        std::fs::read_to_string(data_dir().join("state.json"))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
    }
    pub fn save(&self) {
        write_atomic("state.json", &serde_json::to_string(self).unwrap_or_default());
    }
}

/// Comandos da janela de configuração para a bandeja (via WM_COPYDATA, em JSON).
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Comando {
    Ativar { i: usize },
    Dpi { i: usize, dpi: u16 },
    Cor { i: usize, rgb: [u8; 3] },
    PollingEstagio { i: usize, hz: u32 },
    Lod { v: u8 },
    Debounce { v: u8 },
    Suspensao { s: u16 },
    AngleSnap { on: bool },
    Ripple { on: bool },
    MotionSync { on: bool },
    /// Função de um botão (código de 3 bytes, ver mouse::DEFAULT_BUTTONS).
    Botao { i: usize, codigo: [u8; 3] },
    /// Grava o macro `nome` no mouse e coloca no botão `i`.
    BotaoMacro { i: usize, nome: String },
    BotoesPadrao,
    TrocarEsqDir,
    /// O macro que está no mouse foi editado: grava de novo.
    RegravarMacro,
    Prefs { prefs: Prefs },
    Autostart { on: bool },
    DesativarAkko,
    UiViva,
}

pub fn write_atomic(name: &str, text: &str) {
    let dir = data_dir();
    let tmp = dir.join(format!("{name}.tmp"));
    if std::fs::write(&tmp, text).is_ok() {
        let _ = std::fs::rename(&tmp, dir.join(name));
    }
}
