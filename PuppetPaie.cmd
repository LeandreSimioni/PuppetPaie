@echo off
rem ---------------------------------------------------------------
rem  PuppetPaie (Windows)
rem  Ouvre la fenetre PuppetPaie : logiciel\puppetpaie.exe, avec
rem  ses deux dossiers a cote (donnees\ et interface\).
rem ---------------------------------------------------------------
set "PROG=%~dp0logiciel\puppetpaie.exe"

if exist "%PROG%" (
    start "" "%PROG%"
    exit /b 0
)

echo.
echo   Le programme PuppetPaie est absent (%PROG%).
echo   Reprendre la derniere version du logiciel, puis relancer.
echo.
pause
