//! Textos da interface em inglês e português. O idioma fica em Prefs::idioma ("en" ou "pt").

use std::sync::atomic::{AtomicBool, Ordering};

static PT: AtomicBool = AtomicBool::new(false);

pub fn set_lang(lang: &str) {
    PT.store(lang == "pt", Ordering::Relaxed);
}

pub fn is_pt() -> bool {
    PT.load(Ordering::Relaxed)
}

/// Texto da chave no idioma atual (a própria chave se não existir).
pub fn t(key: &'static str) -> &'static str {
    let pt = is_pt();
    for &(k, en, p) in TEXTS {
        if k == key {
            return if pt { p } else { en };
        }
    }
    key
}

const TEXTS: &[(&str, &str, &str)] = &[
    // menu lateral e cabeçalho
    ("performance", "Performance", "Desempenho"),
    ("button_assignment", "Button Assignment", "Botões"),
    ("macro", "Macro", "Macro"),
    ("games", "Games", "Jogos"),
    ("games_hint", "When a game from this list is in focus, its DPI stage is activated automatically. Checked once per second, without talking to the mouse until a switch is needed.", "Quando um jogo desta lista está em primeiro plano, o estágio de DPI dele é ativado sozinho. Verificado uma vez por segundo, sem falar com o mouse até precisar trocar."),
    ("game_return", "Return to the previous DPI when leaving the game", "Voltar ao DPI anterior ao sair do jogo"),
    ("no_games", "No games yet. Open the game, then click Add game and pick it from the list.", "Nenhum jogo. Abra o jogo, clique em Adicionar jogo e escolha-o na lista."),
    ("add_game", "Add game", "Adicionar jogo"),
    ("pick_game", "Programs open now (open the game first):", "Programas abertos agora (abra o jogo antes):"),
    ("type_exe", "Or type the .exe:", "Ou digite o .exe:"),
    ("add", "Add", "Adicionar"),
    // botões
    ("buttons_unread", "Could not read the button table from the mouse. Turn it off and on.", "Não consegui ler a tabela dos botões do mouse. Desligue e ligue o mouse."),
    ("press_key", "Press a key...", "Aperte uma tecla..."),
    ("press_key_item", "Press a key...", "Apertar uma tecla..."),
    ("no_macros", "No macros yet: create one on the Macro page.", "Nenhum macro: crie um na página Macro."),
    ("default_fn", "Default", "Padrão"),
    ("swap_lr", "Swap Left and Right Clicks", "Inverter clique esquerdo e direito"),
    ("restore_defaults", "Restore Defaults", "Restaurar padrão"),
    ("mouse_buttons", "Mouse Buttons", "Botões do mouse"),
    ("dpi_keys", "DPI Keys", "Teclas de DPI"),
    ("multimedia", "Multimedia", "Multimídia"),
    ("keyboard_keys", "Keyboard Keys", "Teclas do teclado"),
    ("other", "Other", "Outros"),
    ("preset_buttons", "Preset Buttons", "Funções dos botões"),
    ("office", "Office", "Office"),
    ("press_a_key", "Press a Key", "Aperte uma tecla"),
    ("macro_description", "A Macro is a sequence of pre-defined commands that simplifies complex actions in games and other applications.", "Um macro é uma sequência de comandos que simplifica ações complexas em jogos e outros programas."),
    ("select_macro", "Select the recorded macro here", "Escolha o macro gravado aqui"),
    ("media_player", "Open Player", "Abrir player"),
    ("web_home", "Home", "Página inicial"),
    ("web_refresh", "Refresh", "Atualizar"),
    ("web_stop", "Stop", "Parar"),
    ("web_back", "Back", "Voltar"),
    ("web_forward", "Forward", "Avançar"),
    ("web_favorites", "Favorites", "Favoritos"),
    ("web_search", "Search", "Pesquisar"),
    ("my_computer", "My Computer", "Meu computador"),
    ("calculator", "Calculator", "Calculadora"),
    ("email", "Email", "E-mail"),
    ("office_open", "Open", "Abrir"),
    ("office_new", "New", "Novo"),
    ("office_undo", "Undo", "Desfazer"),
    ("office_save", "Save", "Salvar"),
    ("office_copy", "Copy", "Copiar"),
    ("office_cut", "Cut", "Recortar"),
    ("office_paste", "Paste", "Colar"),
    ("office_find", "Find", "Localizar"),
    ("office_select_all", "Select All", "Selecionar tudo"),
    ("office_zoom_out", "Zoom Out", "Diminuir zoom"),
    ("office_zoom_in", "Zoom In", "Aumentar zoom"),
    ("macro_buttons", "Macro Buttons", "Macros"),
    ("btn_left", "Left Click", "Clique esquerdo"),
    ("btn_right", "Right Click", "Clique direito"),
    ("btn_middle", "Scroll Click", "Clique do meio"),
    ("btn_back", "Back", "Voltar"),
    ("btn_forward", "Forward", "Avançar"),
    ("dpi_up", "DPI+", "DPI+"),
    ("dpi_down", "DPI-", "DPI-"),
    ("dpi_cycle", "DPI Cycle", "Ciclo de DPI"),
    ("media_play", "Play/Pause", "Tocar/Pausar"),
    ("media_vol_up", "Volume Up", "Aumentar volume"),
    ("media_vol_down", "Volume Down", "Diminuir volume"),
    ("media_mute", "Volume Mute", "Mudo"),
    ("media_next", "Next Track", "Próxima faixa"),
    ("media_prev", "Previous Track", "Faixa anterior"),
    ("media_stop", "Stop", "Parar"),
    ("disable_btn", "Disabled", "Desativado"),
    // macro
    ("macro_manager", "Macro Manager", "Gerenciador de macros"),
    ("macro_list", "Macro List", "Lista de macros"),
    ("new_macro", "New macro", "Novo macro"),
    ("macro_details", "Macro Details or Actions", "Detalhes e ações do macro"),
    ("macro_pick", "Create a macro with + or pick one from the list.", "Crie um macro no + ou escolha um na lista."),
    ("macro_empty", "Empty. Click Record and type the keys.", "Vazio. Clique em Gravar e digite as teclas."),
    ("macro_hint", "Only keyboard keys are recorded. The mouse stores one macro at a time: assign it to a button on the Button Assignment page.", "Só teclas do teclado são gravadas. O mouse guarda um macro por vez: coloque-o num botão na página Botões."),
    ("record", "Record", "Gravar"),
    ("stop", "Stop", "Parar"),
    ("name", "Name", "Nome"),
    ("delete", "Delete", "Apagar"),
    ("clear", "Clear", "Limpar"),
    ("press", "Press", "Pressiona"),
    ("release", "Release", "Solta"),
    // desempenho (recursos do sensor)
    ("angle_snap", "Angle Snapping", "Angle Snapping"),
    ("motion_sync", "Motion Sync", "Motion Sync"),
    ("ripple", "Ripple Control", "Ripple Control"),
    ("settings", "Settings", "Configurações"),
    ("about", "About", "Sobre"),
    ("minimize", "Minimize", "Minimizar"),
    ("close", "Close to tray", "Fechar para a bandeja"),
    // avisos de estado
    ("tray_starting", "Starting Akko Echo Ultra in the tray...", "Iniciando o Akko Echo Ultra na bandeja..."),
    ("loading", "Loading...", "Carregando..."),
    ("paused", "Paused: the Akko app is open. Close it to configure here (both at once corrupt the mouse memory).", "Pausado: o app da Akko está aberto. Feche-o para configurar por aqui (os dois juntos corrompem a memória do mouse)."),
    ("searching", "Looking for the mouse... turn it on or move it to wake it up.", "Procurando o mouse... ligue-o ou mexa nele para acordar."),
    // desempenho
    ("sensitivity", "Sensitivity", "Sensibilidade"),
    ("dpi_value", "DPI Value", "Valor de DPI"),
    ("dpi_hint", "Click a stage to activate it. Click its color square to change the LED color.", "Clique num estágio para ativá-lo. Clique no quadrado de cor para mudar a cor do LED."),
    ("polling_rate", "Polling Rate", "Taxa de Polling"),
    ("debounce", "Debounce Delay", "Atraso de Debounce"),
    ("debounce_hint", "125µs debounce works only at 8000Hz polling. Using a low debounce may cause accidental double-clicks.", "Debounce de 125µs só funciona com polling de 8000Hz. Debounce baixo pode causar cliques duplos acidentais."),
    ("sleep", "Sleep Setting", "Suspensão"),
    ("quick_dpi", "Quick DPI switch", "Troca rápida de DPI"),
    ("quick_dpi_hint", "Press Left + Middle + Right together to switch to the next DPI stage instantly.", "Pressione Esquerdo + Meio + Direito juntos para trocar para o próximo estágio na hora."),
    ("lod", "LOD Height", "Altura LOD"),
    ("osd", "DPI Notification", "Aviso de DPI"),
    ("osd_hint", "Shows DPI and polling rate in the corner of the screen when switching.", "Mostra o DPI e o polling no canto da tela ao trocar."),
    ("stage_of", "Stage {} of {}", "Estágio {} de {}"),
    ("hz8k_warn", "8000 Hz via software may stop the receiver from responding until the mouse is turned off and on.", "8000 Hz pelo software pode fazer o receptor parar de responder até o mouse ser desligado e ligado."),
    // configurações
    ("launch_startup", "Launch on Startup", "Iniciar com o Windows"),
    ("launch_startup_hint", "Starts minimized in the tray, next to the clock.", "Abre minimizado na bandeja, perto do relógio."),
    ("akko_startup", "Akko app startup", "Inicialização do app da Akko"),
    ("akko_startup_on", "The Akko app opens with Windows. While it is open, Akko Echo Ultra is paused.", "O app da Akko abre com o Windows. Com ele aberto, o Akko Echo Ultra fica pausado."),
    ("akko_startup_off", "Disabled.", "Desativada."),
    ("disable", "Disable", "Desativar"),
    ("osd_duration", "Notification duration", "Duração do aviso"),
    ("language", "Language", "Idioma"),
    // sobre
    ("specs", "Specifications", "Especificações"),
    ("device_name", "Device name: TAN8 Ultra", "Dispositivo: TAN8 Ultra"),
    ("software", "Software: Akko Echo Ultra", "Programa: Akko Echo Ultra"),
    ("battery", "Battery", "Bateria"),
    ("charging", "charging", "carregando"),
    ("polling_now", "Current polling rate", "Polling atual"),
    ("about_note", "Do not use together with the Akko app. Button remapping and macros: use the Akko app (this program pauses itself while it is open).", "Não use junto com o app da Akko. Remapear botões e macros: use o app da Akko (este programa pausa sozinho enquanto ele estiver aberto)."),
    // bandeja
    ("tray_open", "Open Akko Echo Ultra", "Abrir Akko Echo Ultra"),
    ("tray_exit", "Exit", "Sair"),
    ("tray_paused", "Paused (Akko app open)", "Pausado (app da Akko aberto)"),
    ("tray_searching", "Looking for the mouse...", "Procurando o mouse..."),
];
