@echo off
setlocal
cd /d "%~dp0"
title Stardust Menu

:menu
cls
echo =============================
echo        Stardust Menu
echo =============================
echo 1. argon2-aes
echo 2. scrypt-aes
echo 3. pbkdf2-aes
echo 4. argon2-chacha20
echo 5. argon2-blake3
echo.
set "selection="
set /p "selection=Select an option (1-5): "

if "%selection%"=="1" goto option1
if "%selection%"=="2" goto option2
if "%selection%"=="3" goto option3
if "%selection%"=="4" goto option4
if "%selection%"=="5" goto option5

echo Invalid selection. Please enter 1, 2, 3, 4, or 5.
pause
goto menu

:option1
stardust argon2-aes
goto done

:option2
stardust scrypt-aes
goto done

:option3
stardust pbkdf2-aes
goto done

:option4
stardust argon2-chacha20
goto done

:option5
stardust argon2-blake3
goto done

:done
echo.
pause
endlocal
