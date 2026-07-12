@echo off
setlocal enabledelayedexpansion

REM ============================================================
REM  Navicat Patcher - EXE Installer Build Script
REM
REM  Usage: Double-click or run from command line
REM
REM  Prerequisites:
REM    1. JDK 17+ (includes jpackage)
REM    2. Maven 3.6+
REM    3. Inno Setup 6+ (for .exe installer generation)
REM       Download: https://jrsoftware.org/isdl.php
REM
REM  Output: target/exe-installer/NavicatPatcher-1.0.0.exe
REM ============================================================

cd /d "%~dp0"

REM ============================================================
REM  Tool Paths (configure this first)
REM  - Leave JAVA_HOME/MAVEN_HOME commented to use system env or auto-detect from PATH.
REM  - INNO_SETUP_DIR has a default; change if installed elsewhere.
REM ============================================================
REM set "JAVA_HOME=C:\YYJ\Software\JDK21"
REM set "MAVEN_HOME=C:\YYJ\Software\Maven"
if not defined INNO_SETUP_DIR set "INNO_SETUP_DIR=C:\YYJ\Software\Inno Setup 6"

REM ============================================================
REM  Application Configuration
REM ============================================================
set "APP_NAME=NavicatPatcher"
set "APP_VERSION=1.0.0"
set "APP_VENDOR=NavicatPatcher"
set "APP_DESCRIPTION=Navicat 17.3.x Patcher"
set "MAIN_CLASS=com.navicat.patcher.Main"
set "MAIN_JAR=navicat-patcher-1.0.0.jar"
set "UPGRADE_UUID=8B7E3F2A-1C4D-4E5B-9A6F-7D8C9E0F1A2B"
set "ICON_SRC=src\main\resources\icon.ico"
set "APP_COPYRIGHT=Copyright 2024-2026 NavicatPatcher"
set "INSTALL_LANGUAGE=en"
set "SHORTCUT_NAME=Navicat Patcher"

REM ============================================================
REM  Build Options
REM ============================================================
REM Set to 1 to skip Maven build (useful when re-running after ISS changes)
set "SKIP_MAVEN_BUILD=0"

echo.
echo ==================================================
echo   Navicat Patcher - EXE Installer Build Tool
echo ==================================================
echo.

REM ============================================================
REM Step 0: Check dependencies
REM ============================================================
echo [Step 0/5] Checking dependencies...
echo.

REM --- Resolve tool paths ---
set "JAVA_CMD=java"
set "JPACKAGE_CMD=jpackage"
set "MVN_CMD=mvn"
set "ISCC_CMD=ISCC"

REM Resolve JAVA_HOME
set "JAVA_HOME_RESOLVED="
if defined JAVA_HOME (
    if exist "!JAVA_HOME!\bin\java.exe" (
        set "JAVA_HOME_RESOLVED=!JAVA_HOME!"
        set "JAVA_CMD=!JAVA_HOME!\bin\java"
        set "JPACKAGE_CMD=!JAVA_HOME!\bin\jpackage"
    ) else (
        echo   [X] java.exe not found in JAVA_HOME: !JAVA_HOME!
        echo       Please check JAVA_HOME path.
        goto :error
    )
)

REM Resolve MAVEN_HOME
set "MAVEN_HOME_RESOLVED="
if defined MAVEN_HOME (
    if exist "!MAVEN_HOME!\bin\mvn.cmd" (
        set "MAVEN_HOME_RESOLVED=!MAVEN_HOME!"
        set "MVN_CMD=!MAVEN_HOME!\bin\mvn"
    ) else if exist "!MAVEN_HOME!\bin\mvn.bat" (
        set "MAVEN_HOME_RESOLVED=!MAVEN_HOME!"
        set "MVN_CMD=!MAVEN_HOME!\bin\mvn"
    ) else (
        echo   [X] mvn not found in MAVEN_HOME: !MAVEN_HOME!
        echo       Please check MAVEN_HOME path.
        goto :error
    )
)

REM --- Check Java ---
if not defined JAVA_HOME_RESOLVED (
    where java >nul 2>&1
    if errorlevel 1 (
        echo   [X] Java not found.
        echo.
        echo       Download JDK 17+: https://www.oracle.com/java/technologies/downloads/
        echo       Then set JAVA_HOME at the top of build-exe.bat or add java to PATH.
        goto :error
    )
    set "JAVA_CMD=java"
    set "JPACKAGE_CMD=jpackage"
)

REM Extract Java version (use temp file to avoid quoting issues with paths containing spaces)
set "JAVA_VER_STR="
set "JAVA_VER_TMP=%TEMP%\java_ver_%RANDOM%.tmp"
"!JAVA_CMD!" -version 2> "!JAVA_VER_TMP!"
for /f "tokens=3" %%v in ('findstr /i "version" "!JAVA_VER_TMP!"') do (
    set "JAVA_VER_STR=%%~v"
)
del "!JAVA_VER_TMP!" >nul 2>&1
if not defined JAVA_VER_STR (
    echo   [X] Failed to detect Java version.
    goto :error
)

REM Extract major version (handle both 17.x.x and 1.8.x formats)
set "JAVA_MAJOR=0"
for /f "tokens=1 delims=." %%a in ("!JAVA_VER_STR!") do set "JAVA_MAJOR=%%a"
if !JAVA_MAJOR! EQU 1 (
    for /f "tokens=2 delims=." %%a in ("!JAVA_VER_STR!") do set "JAVA_MAJOR=%%a"
)
if !JAVA_MAJOR! LSS 17 (
    echo   [X] JDK 17+ required, current: !JAVA_VER_STR!
    echo.
    echo       Download JDK 17+: https://www.oracle.com/java/technologies/downloads/
    goto :error
)
if defined JAVA_HOME_RESOLVED (
    echo   [OK] Java !JAVA_VER_STR!  [!JAVA_HOME_RESOLVED!]
) else (
    echo   [OK] Java !JAVA_VER_STR!  ^(from PATH^)
)

REM --- Check Maven ---
if not defined MAVEN_HOME_RESOLVED (
    where mvn >nul 2>&1
    if errorlevel 1 (
        echo   [X] Maven not found.
        echo.
        echo       Download Maven 3.6+: https://maven.apache.org/download.cgi
        echo       Then set MAVEN_HOME at the top of build-exe.bat or add mvn to PATH.
        goto :error
    )
    set "MVN_CMD=mvn"
)
if defined MAVEN_HOME_RESOLVED (
    echo   [OK] Maven  [!MAVEN_HOME_RESOLVED!]
) else (
    echo   [OK] Maven  ^(from PATH^)
)

REM --- Check jpackage ---
if not defined JAVA_HOME_RESOLVED (
    where jpackage >nul 2>&1
    if errorlevel 1 (
        echo   [X] jpackage not found.
        echo       Ensure full JDK is installed ^(not JRE^), or set JAVA_HOME.
        goto :error
    )
)
if defined JAVA_HOME_RESOLVED (
    echo   [OK] jpackage  [!JAVA_HOME_RESOLVED!\bin]
) else (
    echo   [OK] jpackage  ^(from PATH^)
)

REM --- Check Inno Setup ---
set "INNO_DIR="
set "INNO_FOUND=0"

if defined INNO_SETUP_DIR (
    if exist "!INNO_SETUP_DIR!\ISCC.exe" (
        set "INNO_DIR=!INNO_SETUP_DIR!"
        set "ISCC_CMD=!INNO_SETUP_DIR!\ISCC.exe"
        set "INNO_FOUND=1"
    ) else (
        echo   [X] ISCC.exe not found in: !INNO_SETUP_DIR!
        echo       Please check INNO_SETUP_DIR at the top of build-exe.bat
        goto :error
    )
)

if !INNO_FOUND! EQU 0 (
    where ISCC >nul 2>&1
    if not errorlevel 1 (
        set "INNO_FOUND=1"
        set "ISCC_CMD=ISCC"
    )
)

if !INNO_FOUND! EQU 0 (
    echo   [X] Inno Setup 6 not found.
    echo.
    echo       Download: https://jrsoftware.org/isdl.php
    echo       Set INNO_SETUP_DIR at the top of build-exe.bat
    echo       or add Inno Setup to system PATH.
    goto :error
)
if defined INNO_DIR (
    echo   [OK] Inno Setup 6  [!INNO_DIR!]
) else (
    echo   [OK] Inno Setup 6  ^(from PATH^)
)

echo.
echo All checks passed!
echo.

REM ============================================================
REM Step 1: Build Fat JAR with Maven
REM ============================================================
echo [Step 1/5] Building Fat JAR with Maven...
echo --------------------------------------------------

if !SKIP_MAVEN_BUILD! EQU 1 (
    echo [SKIP] Maven build skipped ^(SKIP_MAVEN_BUILD=1^)
    if not exist "target\%MAIN_JAR%" (
        echo [ERROR] JAR not found: target\%MAIN_JAR%
        echo         Cannot skip Maven build without existing JAR.
        goto :error
    )
    echo [INFO] Using existing JAR: target\%MAIN_JAR%
    goto :after_maven
)

call "!MVN_CMD!" clean package -DskipTests
if errorlevel 1 (
    echo [ERROR] Maven build failed
    goto :error
)

if not exist "target\%MAIN_JAR%" (
    echo [ERROR] Build output not found: target\%MAIN_JAR%
    goto :error
)

:after_maven

echo --------------------------------------------------
echo [DONE] JAR ready: target\%MAIN_JAR%
echo.

REM ============================================================
REM Step 2: Prepare staging directory
REM ============================================================
echo [Step 2/5] Preparing staging directory...

set "STAGE_DIR=%CD%\target\jpackage-input"
if exist "%STAGE_DIR%" rmdir /s /q "%STAGE_DIR%"
mkdir "%STAGE_DIR%"

copy /y "target\%MAIN_JAR%" "%STAGE_DIR%\" >nul 2>&1
if errorlevel 1 (
    echo [ERROR] Failed to copy JAR to staging directory
    goto :error
)

echo [DONE] Staging: %STAGE_DIR%
echo.

REM ============================================================
REM Step 3: Prepare application icon
REM ============================================================
echo [Step 3/5] Preparing application icon...

set "ICO_PATH=%STAGE_DIR%\icon.ico"
set "ICON_FLAG="
set "ICO_QUOTED="

if exist "%ICON_SRC%" (
    echo [INFO] Copying icon: %ICON_SRC%
    copy /y "%ICON_SRC%" "%ICO_PATH%" >nul 2>&1
    if exist "%ICO_PATH%" (
        echo [DONE] Icon ready: %ICO_PATH%
    ) else (
        echo [WARN] Icon copy failed, will use default icon
    )
) else (
    echo [WARN] Icon source not found: %ICON_SRC% - will use default icon
)

REM Set icon params outside if-block (avoid nested quote issues)
if exist "%ICO_PATH%" set "ICON_FLAG=--icon"
if exist "%ICO_PATH%" set ICO_QUOTED="%ICO_PATH%"

echo.

REM ============================================================
REM Step 4: Build EXE installer (jpackage app-image + Inno Setup)
REM ============================================================
echo [Step 4/5] Building EXE installer...
echo --------------------------------------------------
echo [INFO] App name:    %APP_NAME%
echo [INFO] App version: %APP_VERSION%
echo [INFO] Main class:  %MAIN_CLASS%
echo [INFO] Main JAR:    %MAIN_JAR%
echo.

set "OUTPUT_DIR=%CD%\target\exe-installer"
if exist "%OUTPUT_DIR%" rmdir /s /q "%OUTPUT_DIR%"
mkdir "%OUTPUT_DIR%" 2>nul

REM Use a separate directory for app-image output to avoid
REM jpackage scanning its own output as input (recursive dir bug)
set "IMAGE_DIR=%CD%\target\jpackage-image"
if exist "%IMAGE_DIR%" rmdir /s /q "%IMAGE_DIR%"
mkdir "%IMAGE_DIR%" 2>nul

REM --- Step 4a: Create application image with jpackage ---
echo [4a] Creating application image with jpackage...
call "!JPACKAGE_CMD!" --type app-image --name "%APP_NAME%" --input "%STAGE_DIR%" --main-jar "%MAIN_JAR%" --main-class %MAIN_CLASS% --dest "%IMAGE_DIR%" --app-version %APP_VERSION% --vendor "%APP_VENDOR%" --description "%APP_DESCRIPTION%" --copyright "%APP_COPYRIGHT%" %ICON_FLAG% %ICO_QUOTED%
if errorlevel 1 (
    echo.
    echo [ERROR] jpackage app-image creation failed
    echo.
    echo Common causes:
    echo   1. JDK version too old ^(need 14+^)
    echo   2. Special characters in path
    echo   3. Insufficient disk space ^(need ~500MB temp^)
    goto :error
)
echo [DONE] App image: %IMAGE_DIR%\%APP_NAME%
echo.

REM --- Step 4b: Generate Inno Setup script ---
echo [4b] Generating Inno Setup script...

set "ISS_FILE=%STAGE_DIR%\installer.iss"
set "ISS_ICON="
if exist "%ICO_PATH%" set "ISS_ICON=SetupIconFile=%ICO_PATH%"

(
echo [Setup]
echo AppId={{!UPGRADE_UUID!}}
echo AppName=!APP_NAME!
echo AppVersion=!APP_VERSION!
echo AppVerName=!APP_NAME! !APP_VERSION!
echo AppPublisher=!APP_VENDOR!
echo AppCopyright=!APP_COPYRIGHT!
echo DefaultDirName={autopf}\!APP_NAME!
echo DefaultGroupName=!SHORTCUT_NAME!
echo UninstallDisplayIcon={app}\!APP_NAME!.exe
echo OutputDir=!OUTPUT_DIR!
echo OutputBaseFilename=!APP_NAME!-!APP_VERSION!
echo Compression=lzma2
echo SolidCompression=yes
echo ArchitecturesAllowed=x64
echo ArchitecturesInstallIn64BitMode=x64
echo DisableProgramGroupPage=yes
echo WizardStyle=modern
echo.!ISS_ICON!
echo.
echo [Languages]
echo Name: "!INSTALL_LANGUAGE!"; MessagesFile: "compiler:Default.isl"
echo.
echo [Tasks]
echo Name: "desktopicon"; Description: "Create a desktop icon"; GroupDescription: "Additional icons:"
echo.
echo [Files]
echo Source: "!IMAGE_DIR!\!APP_NAME!\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs
echo.
echo [Icons]
echo Name: "{group}\!SHORTCUT_NAME!"; Filename: "{app}\!APP_NAME!.exe"
echo Name: "{commondesktop}\!SHORTCUT_NAME!"; Filename: "{app}\!APP_NAME!.exe"; Tasks: desktopicon
echo.
echo [Run]
echo Filename: "{app}\!APP_NAME!.exe"; Description: "Launch !SHORTCUT_NAME!"; Flags: nowait postinstall skipifsilent
) > "%ISS_FILE%"

echo [DONE] Script: %ISS_FILE%
echo.

REM --- Step 4c: Compile installer with Inno Setup ---
echo [4c] Compiling installer with Inno Setup...
"!ISCC_CMD!" "%ISS_FILE%"
if errorlevel 1 (
    echo.
    echo [ERROR] Inno Setup compilation failed
    echo.
    echo Common causes:
    echo   1. Inno Setup version too old ^(need 6.0+^)
    echo   2. Path contains special characters
    echo   3. Insufficient disk space
    goto :error
)

echo --------------------------------------------------
echo [DONE] EXE installer generated successfully
echo.

REM ============================================================
REM Step 5: Done
REM ============================================================
echo [Step 5/5] Build complete!
echo.
echo ==================================================
echo.
echo   EXE installer generated successfully!
echo.
echo   Output directory: %OUTPUT_DIR%
echo.
echo   Installer file:
for %%F in ("%OUTPUT_DIR%\*.exe") do (
    echo     %%~nxF  ^(%%~zF bytes^)
)
echo.
echo   Usage:
echo     - Send the .exe file to end users
echo     - Users can install by double-clicking, no Java needed
echo     - Start menu and desktop shortcuts are created
echo.
echo ==================================================
echo.

REM Open output directory
explorer "%OUTPUT_DIR%"

goto :end

REM ============================================================
REM Error handler
REM ============================================================
:error
echo.
echo ==================================================
echo   Build failed! Please check the error messages above.
echo ==================================================
echo.
echo Press any key to exit...
pause >nul
exit /b 1

:end
echo Press any key to exit...
pause >nul
exit /b 0
