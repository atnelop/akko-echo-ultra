# Instala o Akko Echo Ultra em C:\Program Files\Akko Echo Ultra (precisa de administrador) e
# devolve o app da Akko ao original (remove as modificações feitas nele antes deste programa).
$ErrorActionPreference = 'Stop'
$src = Split-Path $PSScriptRoot -Parent          # pasta do projeto
$dir = 'C:\Program Files\Akko Echo Ultra'
$akko = 'C:\Program Files (x86)\Akko'
$rel = New-Object System.Collections.Generic.List[string]

Get-Process AkkoEchoUltra -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep 1
New-Item -ItemType Directory -Force $dir, "$dir\codigo-fonte" | Out-Null

# O .exe vem da compilação (target\release) ou, no zip da página de Releases do GitHub, fica
# ao lado da pasta "instalador".
$exe = @("$src\target\release\AkkoEchoUltra.exe", "$src\AkkoEchoUltra.exe") | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $exe) { throw 'AkkoEchoUltra.exe nao encontrado (compile com: cargo build --release)' }
Copy-Item $exe "$dir\AkkoEchoUltra.exe" -Force
$ico = @("$src\assets\app.ico", "$src\AkkoEchoUltra.ico") | Where-Object { Test-Path $_ } | Select-Object -First 1
if ($ico) { Copy-Item $ico "$dir\AkkoEchoUltra.ico" -Force }
Copy-Item "$PSScriptRoot\desinstalar.ps1" "$dir\desinstalar.ps1" -Force
foreach ($item in 'src', 'assets', 'tools', 'instalador', '.cargo', 'mingw-libs', 'Cargo.toml', 'Cargo.lock', 'build.rs', 'README.md') {
    if (Test-Path "$src\$item") { Copy-Item "$src\$item" "$dir\codigo-fonte\" -Recurse -Force }
}
$rel.Add("programa copiado para $dir")

# Imagens e fontes do app da Akko que a janela usa, copiadas para "akko" ao lado do programa
# (assim a janela não depende do app da Akko instalado). Vêm do app da Akko, se instalado, ou do
# backup da instalação original. Não fazem parte do repositório.
$assets = @('Font\MiSans-Normal.ttf', 'Font\MiSans-Semibold.ttf', 'Skin\About\M0019_mouse@150.png', 'Skin\ms\M0019\mousea@150.png')
foreach ($k in 'key', 'dpi', 'marco', 'about') { foreach ($t in 'normal', 'hot') { $assets += "Skin\Control\${k}_${t}@150.png" } }
$zipPath = "$dir\backup-akko\instalacao_ProgramFiles_Akko.zip"
$zip = $null
$copied = 0
foreach ($a in $assets) {
    $dst = "$dir\akko\$a"
    if (Test-Path $dst) { continue }
    New-Item -ItemType Directory -Force (Split-Path $dst) | Out-Null
    if (Test-Path "$akko\$a") {
        Copy-Item "$akko\$a" $dst
        $copied++
    } elseif (Test-Path $zipPath) {
        if (-not $zip) {
            Add-Type -AssemblyName System.IO.Compression.FileSystem
            $zip = [IO.Compression.ZipFile]::OpenRead($zipPath)
        }
        $entry = $zip.Entries | Where-Object { $_.FullName -ieq $a } | Select-Object -First 1
        if ($entry) {
            [IO.Compression.ZipFileExtensions]::ExtractToFile($entry, $dst, $true)
            $copied++
        }
    }
}
if ($zip) { $zip.Dispose() }
if ($copied) { $rel.Add("imagens e fontes da Akko copiadas para $dir\akko ($copied arquivos)") }

# Atalho no menu Iniciar (para todos os usuários)
$sh = New-Object -ComObject WScript.Shell
$lnk = $sh.CreateShortcut('C:\ProgramData\Microsoft\Windows\Start Menu\Programs\Akko Echo Ultra.lnk')
$lnk.TargetPath = "$dir\AkkoEchoUltra.exe"
$lnk.WorkingDirectory = $dir
$lnk.IconLocation = "$dir\AkkoEchoUltra.exe,0"
$lnk.Description = 'Configuração do mouse Akko TAN8 Ultra'
$lnk.Save()
$rel.Add('atalho criado no menu Iniciar')

# Entrada em "Aplicativos instalados"
$key = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\AkkoEchoUltra'
New-Item $key -Force | Out-Null
$size = [int]((Get-ChildItem $dir -Recurse -File | Measure-Object Length -Sum).Sum / 1KB)
$values = @{
    DisplayName = 'Akko Echo Ultra'; DisplayVersion = '1.3.3'; Publisher = 'Akko Echo Ultra (projeto pessoal)'
    DisplayIcon = "$dir\AkkoEchoUltra.exe,0"; InstallLocation = $dir
    UninstallString = "powershell.exe -NoProfile -ExecutionPolicy Bypass -File `"$dir\desinstalar.ps1`""
}
foreach ($k in $values.Keys) { Set-ItemProperty $key -Name $k -Value $values[$k] }
Set-ItemProperty $key -Name NoModify -Value 1 -Type DWord
Set-ItemProperty $key -Name NoRepair -Value 1 -Type DWord
Set-ItemProperty $key -Name EstimatedSize -Value $size -Type DWord
$rel.Add('registrado em Aplicativos instalados')

# App da Akko de volta ao original
if (Test-Path "$akko\HookDLL_orig.dll") {
    Get-Process Akko -ErrorAction SilentlyContinue | Stop-Process -Force
    Start-Sleep 1
    Move-Item "$akko\HookDLL_orig.dll" "$akko\HookDLL.dll" -Force
    $rel.Add('Akko: HookDLL.dll original restaurado')
}
$xml = "$akko\Skin\XML\DeviceXml\M0019_3950_8762.xml"
if (Test-Path "$xml.orig") { Move-Item "$xml.orig" $xml -Force; $rel.Add('Akko: layout original do TAN8 Ultra restaurado') }
Get-ChildItem "$akko\Skin\power" -Filter 'akkomod_bat_*' -ErrorAction SilentlyContinue | Remove-Item -Force
if (Test-Path "$akko\AkkoMod") {
    if (Test-Path "$akko\AkkoMod\backup") {
        New-Item -ItemType Directory -Force "$dir\backup-akko" | Out-Null
        Copy-Item "$akko\AkkoMod\backup\*" "$dir\backup-akko\" -Recurse -Force
        $rel.Add("backups do Akko movidos para $dir\backup-akko")
    }
    Remove-Item "$akko\AkkoMod" -Recurse -Force
    $rel.Add('Akko: pasta AkkoMod removida')
}

$rel | Set-Content "$dir\instalacao.txt" -Encoding UTF8
Write-Host ($rel -join "`n")
