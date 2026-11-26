use super::super::{VariableNamespace, template, variables};
pub(in crate::shell) fn wrapper() -> String {
    let protected = TEMPLATE.replace(
        "@CMD_PROTECTED_ENVIRONMENT_RESTORE@",
        &variables::cmd_environment_restore(),
    );
    let captured = protected.replace(
        "@CMD_PROTECTED_ENVIRONMENT_CAPTURE@",
        &variables::cmd_environment_capture(),
    );
    VariableNamespace::new()
        .render(&template::render_script(&captured))
        .replace('\n', "\r\n")
}
const TEMPLATE: &str = r#"@echo off
if "%~1"=="--capture" goto capture_user_state
set "@VAR_command_id@=%~1"
set "@VAR_directory@=%~2"
set "@VAR_working_directory@=%~3"
set "@VAR_stdout_file@=%@VAR_directory@%\@STDOUT@"
set "@VAR_stderr_file@=%@VAR_directory@%\@STDERR@"
set "@VAR_script_file@=%@VAR_directory@%\@SCRIPT@"
set "@VAR_shell_exit@=false"
set "@VAR_had_previous_command_id@="
set "@VAR_had_previous_command_directory@="
if defined @COMMAND_ID_ENV@ set "@VAR_had_previous_command_id@=1"
if defined @COMMAND_DIR_ENV@ set "@VAR_had_previous_command_directory@=1"
set "@VAR_previous_command_id@=%@COMMAND_ID_ENV@%"
set "@VAR_previous_command_directory@=%@COMMAND_DIR_ENV@%"
set "@COMMAND_ID_ENV@=%@VAR_command_id@%"
set "@COMMAND_DIR_ENV@=%@VAR_directory@%"
cd /d "%@VAR_working_directory@%"
if errorlevel 1 (
    call :publish_done 1
    call :restore_command_environment
    exit /b 1
)
call :prepend_functerm_paths
if errorlevel 1 (
    call :publish_done 1
    call :restore_command_environment
    exit /b 1
)
set > "%~dp0@VAR_environment_before_file@.txt"
if "%~4"=="1" (
    "%FUNCTERM_REAL_CMD%" /D /Q /V:OFF /S /C ""%~f0" --capture "%~2" "%~5"" > "%~2\@STDOUT@" 2> "%~2\@STDERR@"
) else (
    "%FUNCTERM_REAL_CMD%" /D /Q /V:OFF /S /C ""%~f0" --capture "%~2" "%~5""
)
set "@VAR_exit_code@=%ERRORLEVEL%"
if not exist "%~dp0@VAR_exit_code_file@.txt" (
    set "@VAR_shell_exit@=true"
    goto publish_result
)
if not exist "%~dp0@VAR_cwd_after_file@.txt" (
    echo CMD worker did not publish its working directory 1>&2
    exit 1
)
if not exist "%~dp0@VAR_environment_after_file@.txt" (
    echo CMD worker did not publish its environment 1>&2
    exit 1
)
@CMD_PROTECTED_ENVIRONMENT_CAPTURE@
for /f "usebackq delims=" %%e in ("%~dp0@VAR_environment_after_file@.txt") do set "%%e"
@CMD_PROTECTED_ENVIRONMENT_RESTORE@
set /p "@VAR_exit_code@="<"%~dp0@VAR_exit_code_file@.txt"
set /p "@VAR_current_directory@="<"%~dp0@VAR_cwd_after_file@.txt"
del /q "%~dp0@VAR_environment_before_file@.txt" "%~dp0@VAR_environment_after_file@.txt" "%~dp0@VAR_protected_environment_file@.txt" "%~dp0@VAR_exit_code_file@.txt" "%~dp0@VAR_cwd_after_file@.txt"
cd /d "%@VAR_current_directory@%"
call :prepend_functerm_paths
if errorlevel 1 set "@VAR_exit_code@=1"
:publish_result
if "%~4"=="1" if exist "%@VAR_stdout_file@%" type "%@VAR_stdout_file@%"
if "%~4"=="1" if exist "%@VAR_stderr_file@%" type "%@VAR_stderr_file@%" 1>&2
call :publish_done %@VAR_exit_code@%
if errorlevel 1 (
    call :restore_command_environment
    exit /b 1
)
call :restore_command_environment
if "%@VAR_shell_exit@%"=="true" exit %@VAR_exit_code@%
exit /b %@VAR_exit_code@%
:capture_user_state
call :publish_start
if errorlevel 1 exit /b 1
"%FUNCTERM_REAL_CMD%" /D /Q /C exit %~3
call "%~2\@SCRIPT@"
> "%~dp0@VAR_exit_code_file@.txt" echo %ERRORLEVEL%
cd > "%~dp0@VAR_cwd_after_file@.txt"
set > "%~dp0@VAR_environment_after_file@.txt" 2> nul
exit /b 0
:prepend_functerm_paths
"%@HELPER_ENV@%" internal-cmd-path --startup-directory "%~dp0." > "%~dp0@VAR_search_path_file@.txt"
if errorlevel 1 (
    del /q "%~dp0@VAR_search_path_file@.txt"
    exit /b 1
)
for /f "usebackq delims=" %%p in ("%~dp0@VAR_search_path_file@.txt") do set "PATH=%%p"
set "@VAR_path_exit_code@=%ERRORLEVEL%"
del /q "%~dp0@VAR_search_path_file@.txt"
exit /b %@VAR_path_exit_code@%
:publish_done
if "%@HELPER_ENV@%"=="" (
    echo @HELPER_ENV@ is not set 1>&2
    exit /b 1
)
"%@HELPER_ENV@%" internal-write-done --command-id "%@VAR_command_id@%" --exit-code "%~1" --session-exit %@VAR_shell_exit@% --cwd "%CD:\=/%" --directory "%@VAR_directory@%"
exit /b %ERRORLEVEL%
:publish_start
if "%@HELPER_ENV@%"=="" (
    echo @HELPER_ENV@ is not set 1>&2
    exit /b 1
)
"%@HELPER_ENV@%" internal-write-start --command-id "%@VAR_command_id@%" --directory "%@VAR_directory@%"
exit /b %ERRORLEVEL%
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
