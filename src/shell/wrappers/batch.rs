use super::template;
pub(in crate::shell) fn wrapper() -> String {
    let protected = TEMPLATE.replace(
        "@CMD_PROTECTED_ENVIRONMENT_RESTORE@",
        &super::variables::cmd_environment_restore(),
    );
    let captured = protected.replace(
        "@CMD_PROTECTED_ENVIRONMENT_CAPTURE@",
        &super::variables::cmd_environment_capture(),
    );
    super::VariableNamespace::new().render(&template::render_script(&captured))
}
const TEMPLATE: &str = r#"@echo off
set "@VAR_command_id@=%~1"
set "@VAR_directory@=%~2"
set "@VAR_working_directory@=%~3"
set "@VAR_input_dir@=%@VAR_directory@%\@INPUT_DIR@"
set "@VAR_output_dir@=%@VAR_directory@%\@OUTPUT_DIR@"
set "@VAR_state_dir@=%@VAR_directory@%\@STATE_DIR@"
if not exist "%@VAR_input_dir@%" mkdir "%@VAR_input_dir@%" || exit /b 1
if not exist "%@VAR_output_dir@%" mkdir "%@VAR_output_dir@%" || exit /b 1
if not exist "%@VAR_state_dir@%" mkdir "%@VAR_state_dir@%" || exit /b 1
set "@VAR_stdout_file@=%@VAR_output_dir@%\@STDOUT@"
set "@VAR_stderr_file@=%@VAR_output_dir@%\@STDERR@"
set "@VAR_script_file@=%@VAR_input_dir@%\@SCRIPT@"
set "@VAR_done_file@=%@VAR_state_dir@%\@DONE@"
set "@VAR_time_consumption@=0ns"
set "@VAR_had_previous_command_id@="
set "@VAR_had_previous_command_directory@="
if defined @COMMAND_ID_ENV@ set "@VAR_had_previous_command_id@=1"
if defined @COMMAND_DIR_ENV@ set "@VAR_had_previous_command_directory@=1"
set "@VAR_previous_command_id@=%@COMMAND_ID_ENV@%"
set "@VAR_previous_command_directory@=%@COMMAND_DIR_ENV@%"
set "@COMMAND_ID_ENV@=%@VAR_command_id@%"
set "@COMMAND_DIR_ENV@=%@VAR_directory@%"
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
cd /d "%@VAR_working_directory@%"
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
set "@VAR_command_started_at@=%ERRORLEVEL%"
set > "%~dp0@VAR_environment_before_file@.txt"
setlocal DisableDelayedExpansion
call "%~2\@INPUT_DIR@\@SCRIPT@" > "%~2\@OUTPUT_DIR@\@STDOUT@" 2> "%~2\@OUTPUT_DIR@\@STDERR@"
> "%~dp0@VAR_exit_code_file@.txt" echo %ERRORLEVEL%
cd > "%~dp0@VAR_cwd_after_file@.txt"
set > "%~dp0@VAR_environment_after_file@.txt" 2> nul
endlocal
@CMD_PROTECTED_ENVIRONMENT_CAPTURE@
for /f "usebackq delims=" %%e in ("%~dp0@VAR_environment_after_file@.txt") do set "%%e"
@CMD_PROTECTED_ENVIRONMENT_RESTORE@
set /p "@VAR_exit_code@="<"%~dp0@VAR_exit_code_file@.txt"
set /p "@VAR_current_directory@="<"%~dp0@VAR_cwd_after_file@.txt"
del /q "%~dp0@VAR_environment_before_file@.txt" "%~dp0@VAR_environment_after_file@.txt" "%~dp0@VAR_protected_environment_file@.txt" "%~dp0@VAR_exit_code_file@.txt" "%~dp0@VAR_cwd_after_file@.txt"
cd /d "%@VAR_current_directory@%"
call :command_time_millis
set /a @VAR_command_elapsed@=%ERRORLEVEL% - @VAR_command_started_at@
if %@VAR_command_elapsed@% LSS 0 set /a @VAR_command_elapsed@+=86400000
set "@VAR_time_consumption@=%@VAR_command_elapsed@%ms"
if exist "%@VAR_stdout_file@%" type "%@VAR_stdout_file@%"
if exist "%@VAR_stderr_file@%" type "%@VAR_stderr_file@%" 1>&2
if not exist "%@VAR_state_dir@%" mkdir "%@VAR_state_dir@%" || exit /b 1
call :publish_done %@VAR_exit_code@%
if errorlevel 1 (
    call :restore_command_environment
    exit /b 1
)
call :restore_command_environment
exit /b %@VAR_exit_code@%
:prepend_shim_path
if "%FUNCTERM_SHIM_DIR%"=="" exit /b 0
set "@VAR_new_path@=%FUNCTERM_SHIM_DIR%"
set "@VAR_remaining_path@=%PATH%"
:prepend_shim_path_entry
if not defined @VAR_remaining_path@ goto prepend_shim_path_done
for /f "tokens=1* delims=;" %%a in ("%@VAR_remaining_path@%") do set "@VAR_path_entry@=%%~a" & set "@VAR_remaining_path@=%%b"
if /i "%@VAR_path_entry@%"=="%FUNCTERM_SHIM_DIR%" goto prepend_shim_path_entry
if defined @VAR_path_entry@ set "@VAR_new_path@=%@VAR_new_path@%;%@VAR_path_entry@%"
goto prepend_shim_path_entry
:prepend_shim_path_done
set "PATH=%@VAR_new_path@%"
exit /b 0
:publish_done
if exist "%@VAR_done_file@%" exit /b 0
if "%@HELPER_ENV@%"=="" (
    echo @HELPER_ENV@ is not set 1>&2
    exit /b 1
)
"%@HELPER_ENV@%" internal-write-done --command-id "%@VAR_command_id@%" --exit-code "%~1" --time-consumption "%@VAR_time_consumption@%" --cwd "%CD%" --directory "%@VAR_directory@%"
exit /b %ERRORLEVEL%
:publish_start
if "%@HELPER_ENV@%"=="" (
    echo @HELPER_ENV@ is not set 1>&2
    exit /b 1
)
"%@HELPER_ENV@%" internal-write-start --command-id "%@VAR_command_id@%" --directory "%@VAR_directory@%"
exit /b %ERRORLEVEL%
:command_time_millis
set "@VAR_time_value@=%TIME: =0%"
set /a "@VAR_time_millis@=((1%@VAR_time_value@:~0,2% %% 100 * 60 + 1%@VAR_time_value@:~3,2% %% 100) * 60 + 1%@VAR_time_value@:~6,2% %% 100) * 1000 + 1%@VAR_time_value@:~9,2% %% 100 * 10"
exit /b %@VAR_time_millis@%
:restore_command_environment
if defined @VAR_had_previous_command_id@ (
    set "@COMMAND_ID_ENV@=%@VAR_previous_command_id@%"
) else (
    set "@COMMAND_ID_ENV@="
)
if defined @VAR_had_previous_command_directory@ (
    set "@COMMAND_DIR_ENV@=%@VAR_previous_command_directory@%"
) else (
    set "@COMMAND_DIR_ENV@="
)
exit /b 0
"#;
