@echo off
cd /d C:\KI\StudioApp
set "OUT=%USERPROFILE%\Downloads\Fertige Programme"
if not exist "%OUT%" mkdir "%OUT%"
cargo tauri build
if errorlevel 1 (
  echo.
  echo [FEHLER] Build fehlgeschlagen!
  pause
  exit /b 1
)
copy /Y "src-tauri\target\release\ki-bild-studio.exe" "%OUT%\KI Bildstudio Lokal.exe"
copy /Y "src-tauri\target\release\bundle\nsis\*.exe" "%OUT%\"
copy /Y "src-tauri\target\release\bundle\msi\*.msi" "%OUT%\"
echo.
echo ===================================================
echo FERTIG! Auslieferungsdateien liegen jetzt in:
echo %OUT%
echo ===================================================
dir /b "%OUT%"
explorer "%OUT%"
pause

