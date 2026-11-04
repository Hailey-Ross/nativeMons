@echo off
rem Open with no arguments to build everything, ready to run (see build-all.ps1), at low priority so the PC stays usable.
rem build.bat <output folder> builds only NativeMonsLauncher.exe, with the C# compiler that ships with Windows.
setlocal
cd /d "%~dp0"
set "OUT=%~1"
if not "%OUT%"=="" goto launcher
start "" /b /wait /low powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0build-all.ps1"
set "RESULT=%errorlevel%"
if not "%RESULT%"=="0" echo. & echo Build failed.
echo.
pause
exit /b %RESULT%
:launcher
if not exist "%OUT%" mkdir "%OUT%"
set "FW=%WINDIR%\Microsoft.NET\Framework64\v4.0.30319"
"%FW%\csc.exe" /nologo /target:winexe /optimize+ /r:System.Web.Extensions.dll /r:System.Core.dll /r:"%FW%\System.IO.Compression.FileSystem.dll" /r:"%FW%\System.IO.Compression.dll" /win32icon:nativemons.ico /out:"%OUT%\NativeMonsLauncher.exe" NativeMonsLauncher.cs
if errorlevel 1 (echo Build failed. & exit /b 1)
copy /y games.json "%OUT%\games.json" >nul
rem A local build: version.txt's local=<commit> stops the updater offering a release over it (a release package writes its own).
set "REV=unknown"
for /f %%r in ('git -C "%~dp0.." rev-parse HEAD 2^>nul') do set "REV=%%r"
powershell -NoProfile -Command "$f = '%OUT%\version.txt'; $l = @(if (Test-Path -LiteralPath $f) { Get-Content -LiteralPath $f | Where-Object { $_ -notlike 'local=*' } }) + 'local=%REV%'; Set-Content -LiteralPath $f -Value $l -Encoding ascii"
echo Built %OUT%\NativeMonsLauncher.exe
