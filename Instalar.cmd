@echo off
rem Instala o Akko Echo Ultra em C:\Program Files\Akko Echo Ultra (pede administrador).
powershell -NoProfile -ExecutionPolicy Bypass -Command "Start-Process powershell -Verb RunAs -ArgumentList '-NoProfile -ExecutionPolicy Bypass -File \"%~dp0instalador\instalar.ps1\"'"
