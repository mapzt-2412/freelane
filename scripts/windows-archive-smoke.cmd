@echo off
setlocal
set "TOOLS=%~dp0"
rem Run from a tools CD with verified official 7zip-extra and serial/vioser.inf.
rem The output directory must be new; preserve previous diagnostic results.
if "%~1"=="" (
  echo Usage: windows-archive-smoke.cmd new-output-directory
  exit /b 2
)
set "WORK=%~1"
if exist "%WORK%" (
  echo Output directory already exists. Choose a new directory.
  exit /b 2
)
mkdir "%WORK%"
if errorlevel 1 exit /b 2
call :checks > "%WORK%\result.txt" 2>&1
set "RESULT=%ERRORLEVEL%"
type "%WORK%\result.txt"
echo AcceptanceExitCode=%RESULT%
exit /b %RESULT%

:checks
ver
echo Windows archive acceptance: source on readonly tools CD
call :run x86 "%TOOLS%7zip-extra\7za.exe"
if errorlevel 1 exit /b 1
call :run x64 "%TOOLS%7zip-extra\x64\7za.exe"
if errorlevel 1 exit /b 1
call :run arm64 "%TOOLS%7zip-extra\arm64\7za.exe"
if errorlevel 1 exit /b 1
echo All three architectures created, tested, extracted and byte-compared successfully.
exit /b 0

:run
echo Architecture=%~1
"%~2" a -y "%WORK%\%~1.7z" "%TOOLS%serial\vioser.inf"
if errorlevel 1 exit /b 1
"%~2" t "%WORK%\%~1.7z"
if errorlevel 1 exit /b 1
"%~2" x -y "%WORK%\%~1.7z" "-o%WORK%\%~1"
if errorlevel 1 exit /b 1
fc /b "%TOOLS%serial\vioser.inf" "%WORK%\%~1\vioser.inf"
if errorlevel 1 exit /b 1
echo Architecture=%~1 PASS
exit /b 0
