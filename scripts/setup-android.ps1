$ErrorActionPreference = "Stop"

Write-Host "=== StockPredict Android environment check ===" -ForegroundColor Cyan

function Test-CommandExists($name) {
    return [bool](Get-Command $name -ErrorAction SilentlyContinue)
}

function Find-JavaHome {
    if ($env:JAVA_HOME -and (Test-Path "$env:JAVA_HOME\bin\java.exe")) {
        return $env:JAVA_HOME
    }

    $candidates = @(
        "C:\Program Files\Android\Android Studio\jbr",
        "C:\Program Files\Eclipse Adoptium\jdk-17*",
        "C:\Program Files\Microsoft\jdk-17*",
        "C:\Program Files\Java\jdk-17*"
    )

    foreach ($pattern in $candidates) {
        $match = Get-ChildItem $pattern -ErrorAction SilentlyContinue | Sort-Object Name -Descending | Select-Object -First 1
        if ($match) {
            return $match.FullName
        }
    }

    return $null
}

$ok = $true

if (Test-CommandExists "node") {
    Write-Host "[OK] Node.js $(node -v)" -ForegroundColor Green
} else {
    Write-Host "[!!] Node.js not found" -ForegroundColor Red
    $ok = $false
}

if (Test-CommandExists "rustc") {
    Write-Host "[OK] Rust $(rustc --version)" -ForegroundColor Green
} else {
    Write-Host "[!!] Rust not found" -ForegroundColor Red
    $ok = $false
}

$javaHome = Find-JavaHome
if ($javaHome) {
    Write-Host "[OK] JAVA_HOME = $javaHome" -ForegroundColor Green
    if (-not $env:JAVA_HOME) {
        Write-Host "     tip: `$env:JAVA_HOME = '$javaHome'" -ForegroundColor Yellow
    }
} elseif (Test-CommandExists "java") {
    Write-Host "[OK] java is on PATH" -ForegroundColor Green
} else {
    Write-Host "[!!] Java 17 not found (Temurin JDK 17 or Android Studio JBR)" -ForegroundColor Red
    $ok = $false
}

$sdk = $env:ANDROID_HOME
if (-not $sdk) {
    $sdk = Join-Path $env:LOCALAPPDATA "Android\Sdk"
}

if (Test-Path $sdk) {
    Write-Host "[OK] Android SDK: $sdk" -ForegroundColor Green
    if (-not $env:ANDROID_HOME) {
        Write-Host "     tip: `$env:ANDROID_HOME = '$sdk'" -ForegroundColor Yellow
    }
} else {
    Write-Host "[!!] Android SDK not found (install Android Studio)" -ForegroundColor Red
    $ok = $false
}

if ($sdk -and (Test-Path "$sdk\platform-tools\adb.exe")) {
    Write-Host "[OK] adb available" -ForegroundColor Green
} elseif ($sdk) {
    Write-Host "[!!] platform-tools missing" -ForegroundColor Red
    $ok = $false
}

$ndkRoot = $env:NDK_HOME
if (-not $ndkRoot -and $sdk) {
    $ndkDir = Join-Path $sdk "ndk"
    if (Test-Path $ndkDir) {
        $ndkRoot = Get-ChildItem $ndkDir -ErrorAction SilentlyContinue | Sort-Object Name -Descending | Select-Object -First 1 -ExpandProperty FullName
    }
}

if ($ndkRoot -and (Test-Path $ndkRoot)) {
    Write-Host "[OK] NDK: $ndkRoot" -ForegroundColor Green
} else {
    Write-Host "[!!] NDK not found (install via SDK Manager)" -ForegroundColor Red
    $ok = $false
}

if (Test-Path "src-tauri\gen\android") {
    Write-Host "[OK] gen/android initialized" -ForegroundColor Green
} else {
    Write-Host "[--] run npm run android:init first" -ForegroundColor Yellow
}

Write-Host ""
if ($ok) {
    Write-Host "Ready. Next: npm run android:init / npm run android:dev" -ForegroundColor Green
} else {
    Write-Host "Fix the items above. See SETUP-ANDROID.md" -ForegroundColor Red
    exit 1
}
