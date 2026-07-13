use super::template;
pub(in crate::shell) fn wrapper() -> String {
    template::render_script(TEMPLATE)
}
const TEMPLATE: &str = r#"@echo off
setlocal DisableDelayedExpansion
set "command_id=%~1"
set "directory=%~2"
set "working_directory=%~3"
set "input_dir=%directory%\@INPUT_DIR@"
set "output_dir=%directory%\@OUTPUT_DIR@"
set "state_dir=%directory%\@STATE_DIR@"
if not exist "%input_dir%" mkdir "%input_dir%" || exit /b 1
if not exist "%output_dir%" mkdir "%output_dir%" || exit /b 1
if not exist "%state_dir%" mkdir "%state_dir%" || exit /b 1
set "stdout_file=%output_dir%\@STDOUT@"
set "stderr_file=%output_dir%\@STDERR@"
set "script_file=%input_dir%\@SCRIPT@"
set "done_file=%state_dir%\@DONE@"
set "time_consumption=0ns"
set "had_previous_command_id="
set "had_previous_command_directory="
if defined @COMMAND_ID_ENV@ set "had_previous_command_id=1"
if defined @COMMAND_DIR_ENV@ set "had_previous_command_directory=1"
set "previous_command_id=%@COMMAND_ID_ENV@%"
set "previous_command_directory=%@COMMAND_DIR_ENV@%"
set "@COMMAND_ID_ENV@=%command_id%"
set "@COMMAND_DIR_ENV@=%directory%"
if not "%FUNCTERM_SHIM_DIR%"=="" (
    if "%@HELPER_ENV@%"=="" (
        echo @HELPER_ENV@ is not set 1>&2
        call :publish_done 1
        call :restore_command_environment
        exit /b 1
    )
    "%@HELPER_ENV@%" internal-ensure-shims --directory "%FUNCTERM_SHIM_DIR%"
    if errorlevel 1 (
        call :publish_done 1
        call :restore_command_environment
        exit /b 1
    )
)
call :prepend_shim_path
cd /d "%working_directory%"
if errorlevel 1 (
    call :publish_done 1
    call :restore_command_environment
    exit /b 1
)
call :publish_start
if errorlevel 1 (
    call :publish_done 1
    call :restore_command_environment
    exit /b 1
)
call :command_time_millis
set "command_started_at=%ERRORLEVEL%"
call "%script_file%" > "%stdout_file%" 2> "%stderr_file%"
set "exit_code=%ERRORLEVEL%"
call :command_time_millis
set /a command_elapsed=%ERRORLEVEL% - command_started_at
if %command_elapsed% LSS 0 set /a command_elapsed+=86400000
set "time_consumption=%command_elapsed%ms"
if exist "%stdout_file%" type "%stdout_file%"
if exist "%stderr_file%" type "%stderr_file%" 1>&2
if not exist "%state_dir%" mkdir "%state_dir%" || exit /b 1
call :publish_done %exit_code%
if errorlevel 1 (
    call :restore_command_environment
    exit /b 1
)
call :restore_command_environment
exit /b %exit_code%
:prepend_shim_path
if "%FUNCTERM_SHIM_DIR%"=="" exit /b 0
set "PATH=%FUNCTERM_SHIM_DIR%;%PATH%"
exit /b 0
:publish_done
if exist "%done_file%" exit /b 0
if "%@HELPER_ENV@%"=="" (
    echo @HELPER_ENV@ is not set 1>&2
    exit /b 1
)
"%@HELPER_ENV@%" internal-write-done --command-id "%command_id%" --exit-code "%~1" --time-consumption "%time_consumption%" --cwd "%CD%" --directory "%directory%"
exit /b %ERRORLEVEL%
:publish_start
if "%@HELPER_ENV@%"=="" (
    echo @HELPER_ENV@ is not set 1>&2
    exit /b 1
)
"%@HELPER_ENV@%" internal-write-start --command-id "%command_id%" --directory "%directory%"
exit /b %ERRORLEVEL%
:command_time_millis
for /f "tokens=1-4 delims=:. ," %%a in ("%TIME%") do (
    set /a "functerm_time_millis=(((1%%a %% 100) * 60 + 1%%b %% 100) * 60 + 1%%c %% 100) * 1000 + 1%%d0 %% 1000"
)
exit /b %functerm_time_millis%
:restore_command_environment
if defined had_previous_command_id (
    set "@COMMAND_ID_ENV@=%previous_command_id%"
) else (
    set "@COMMAND_ID_ENV@="
)
if defined had_previous_command_directory (
    set "@COMMAND_DIR_ENV@=%previous_command_directory%"
) else (
    set "@COMMAND_DIR_ENV@="
)
exit /b 0
"#;
