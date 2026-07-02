@echo off
rem Launch the pblang native IDE (pbide) on Windows.
rem
rem   editors\ide\run.bat              :: open the IDE
rem   editors\ide\run.bat main.sapp    :: open the IDE on a file
rem   set PBIDE_SCALE=2 && editors\ide\run.bat   :: force a UI scale (else auto-fits display)
rem
rem The IDE auto-detects your display resolution and scales itself accordingly. It runs
rem with --allow-all because the editor reads/writes files, runs the formatter/checker,
rem and (for the terminal panel) executes shell commands.
setlocal enabledelayedexpansion

set "SCRIPT_DIR=%~dp0"
pushd "%SCRIPT_DIR%..\.."
set "ROOT=%CD%"

rem Prefer the workspace build. PBIDE and the graphics/input runtime must match
rem during development; an older installed pblang on PATH can make clicks/scaling
rem look broken while loading newer IDE sources.
if not "%PBLANG_BIN%"=="" (
    set "PBLANG=%PBLANG_BIN%"
) else if exist "%ROOT%\target\debug\pblang.exe" (
    set "PBLANG=%ROOT%\target\debug\pblang.exe"
) else if exist "%ROOT%\target\release\pblang.exe" (
    set "PBLANG=%ROOT%\target\release\pblang.exe"
) else (
    where pblang >nul 2>nul
    if !ERRORLEVEL!==0 (
        set "PBLANG=pblang"
    ) else (
        echo pblang.exe not found. Build it first:  cargo build --bin pblang 1>&2
        popd
        endlocal
        exit /b 1
    )
)

echo pbIDE launcher: %PBLANG% 1>&2
"%PBLANG%" ide %*
set "RC=%ERRORLEVEL%"
popd
endlocal & exit /b %RC%
