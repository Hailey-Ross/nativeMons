@echo off
rem Builds NativeMonsLauncher.exe with the C# compiler that ships with Windows. Usage: build.bat [output folder]
setlocal
cd /d "%~dp0"
set "OUT=%~1"
if "%OUT%"=="" set "OUT=%~dp0bin"
if not exist "%OUT%" mkdir "%OUT%"
set "FW=%WINDIR%\Microsoft.NET\Framework64\v4.0.30319"
"%FW%\csc.exe" /nologo /target:winexe /optimize+ /r:System.Web.Extensions.dll /r:System.Core.dll /r:"%FW%\System.IO.Compression.FileSystem.dll" /r:"%FW%\System.IO.Compression.dll" /out:"%OUT%\NativeMonsLauncher.exe" NativeMonsLauncher.cs
if errorlevel 1 (echo Build failed. & exit /b 1)
copy /y games.json "%OUT%\games.json" >nul
echo Built %OUT%\NativeMonsLauncher.exe
