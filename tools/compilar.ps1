# Compila a versão final. Os caminhos da pasta do usuário (registro do cargo, rustup, projeto) são
# trocados por "~" dentro do .exe, para o nome de usuário do Windows não ir junto no programa.
$ErrorActionPreference = 'Stop'
Set-Location (Split-Path $PSScriptRoot -Parent)
$env:RUSTFLAGS = "-L native=mingw-libs --remap-path-prefix=$env:USERPROFILE=~"
cargo build --release
