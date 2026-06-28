@echo off
setlocal

:: MS Manage 启动脚本
:: 确保正确工作目录 + 无 cmd 窗口弹出

cd /d "%~dp0"
start "" "ms_manage.exe"

exit /b