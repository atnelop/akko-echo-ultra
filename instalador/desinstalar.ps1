# Desinstala o Akko Echo Ultra (chamado por "Aplicativos instalados" do Windows).
$ErrorActionPreference = 'SilentlyContinue'
$principal = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Start-Process powershell.exe -Verb RunAs -ArgumentList "-NoProfile -ExecutionPolicy Bypass -File `"$PSCommandPath`""
    exit
}
$dir = 'C:\Program Files\Akko Echo Ultra'
Get-Process AkkoEchoUltra | Stop-Process -Force
Start-Sleep 1
Remove-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run' -Name 'Akko Echo Ultra'
Remove-Item 'C:\ProgramData\Microsoft\Windows\Start Menu\Programs\Akko Echo Ultra.lnk' -Force
Remove-Item 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\AkkoEchoUltra' -Recurse -Force
# A pasta (com os backups do Akko) é apagada depois que este script terminar.
Start-Process cmd.exe -WindowStyle Hidden -ArgumentList "/c timeout /t 2 >nul & rmdir /s /q `"$dir`""
