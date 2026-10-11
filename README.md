# Akko Echo Ultra

Programa próprio (Rust, nativo do Windows) para configurar o mouse **Akko TAN8 Ultra** (receptor 2.4G, VID `38EE` PID `0024`) sem o app da Akko. Fica na bandeja do Windows mostrando a bateria do mouse.

## Funções

- **Button Assignment:** função de cada botão. Opções: botões do mouse, DPI+/DPI-/ciclo, teclas e atalhos (Ctrl/Shift/Alt/Win), multimídia, Office, macro e desativado. Também inverte esquerdo e direito e restaura o padrão.
- **Performance:**
  - 4 estágios de DPI com cor.
  - Polling rate (125 Hz a 8k Hz), o mesmo para todos os estágios.
  - Debounce, suspensão e LOD (0,7/1/2 mm).
  - Angle Snapping, Motion Sync e Ripple Control.
- **Troca rápida de DPI:** esquerdo + meio + direito juntos trocam o estágio na hora, com um aviso no canto da tela mostrando o DPI.
- **Games:** DPI por jogo. Com um jogo da lista em primeiro plano, o estágio de DPI dele é ativado sozinho; ao sair do jogo, volta ao DPI anterior.
- **Macro:** grava sequências de teclas, com os tempos, e coloca o macro num botão. O mouse guarda um macro por vez.
- **Bandeja:** ícone com a porcentagem da bateria. Iniciar com o Windows, minimizado.
- **Idioma:** inglês e português.

## Instalar (sem compilar)

1. Baixe o `.zip` mais recente em **Releases** e extraia.
2. Dê dois cliques em `Instalar.cmd` e aceite o pedido de administrador.

O programa vai para `C:\Program Files\Akko Echo Ultra`, com atalho no menu Iniciar. Para desinstalar, use **Configurações → Aplicativos instalados**.

> Não use junto com o app da Akko: os dois falando com o mouse ao mesmo tempo corrompem a memória dele. O Echo Ultra pausa sozinho enquanto o `Akko.exe` estiver aberto.
>
> Se o app da Akko estiver instalado na hora da instalação, o Echo Ultra copia dele as fotos do mouse, os ícones e as fontes. Sem ele, a janela usa um desenho próprio do mouse, ícones próprios e a fonte Segoe UI do Windows. Os arquivos da Akko não fazem parte deste repositório.

## Compilar

Requisitos: Rust com o toolchain `stable-x86_64-pc-windows-gnu`.

```bash
powershell -ExecutionPolicy Bypass -File tools\compilar.ps1
```

O script roda `cargo build --release` trocando os caminhos da pasta do usuário por `~` dentro do `.exe`.

- `mingw-libs/libshlwapi.a` foi gerado com `zig dlltool` (ver `.cargo/config.toml`).
- O ícone e as informações de versão (`assets/app.res.o`) são gerados com `zig rc /:output-format coff /:target x64 /fo app.res.o app.rc`.
- Algumas dependências estão fixadas no `Cargo.lock` (parking_lot 0.12.3, parking_lot_core 0.9.10, libloading 0.8.6, arboard 3.4.1) para evitar `raw-dylib`, que exige o `dlltool` do MinGW.

Depois de compilar, `instalador\instalar.ps1` (como administrador) instala a versão compilada.

## Protocolo do mouse (resumo)

Interface HID `col04`, relatórios de 64 bytes: `04 [checksum lo] [checksum hi] [cmd] [tamanho] [endereço] 00 00 [dados]`. O checksum é a soma dos bytes a partir do índice 3.

| cmd | função |
|---|---|
| `05` / `06` | ler / gravar a memória de configuração (0x00–0x7f), gravação entre `01` e `02` |
| `07` / `09` | ler / gravar a tabela dos botões (3 bytes por botão) |
| `15` | gravar o macro |
| `1a` | bateria (%) |

Os detalhes estão nos comentários de `src/mouse.rs`.

Cuidados descobertos na prática:
- Cada pedido pelo receptor 2.4G move o cursor ~1 px, então o programa não faz consultas contínuas.
- Gravações parciais podem se perder: a memória é sempre gravada inteira.
- Entrar em 8 kHz pelo software pode travar o receptor até desligar e ligar o mouse.

Projeto pessoal, sem relação com a Akko.
