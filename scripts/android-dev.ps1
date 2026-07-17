param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$Args
)

$ErrorActionPreference = "Stop"

$ProjectRoot = Split-Path $PSScriptRoot -Parent
$DevPort = 1421
$AppActivity = "com.stockpredict.app/.MainActivity"

function Test-SymlinkCreationAllowed {
    $dir = Join-Path $env:TEMP "stockpredict-symlink-test"
    try {
        New-Item -ItemType Directory -Force -Path $dir | Out-Null
        $target = Join-Path $dir "target.txt"
        Set-Content -Path $target -Value "test"
        $link = Join-Path $dir "link.txt"
        New-Item -ItemType SymbolicLink -Path $link -Target $target -ErrorAction Stop | Out-Null
        return $true
    } catch {
        return $false
    } finally {
        if (Test-Path $dir) {
            Remove-Item $dir -Recurse -Force -ErrorAction SilentlyContinue
        }
    }
}

function Get-LanIpAddress {
    $ip = Get-NetIPAddress -AddressFamily IPv4 -ErrorAction SilentlyContinue |
        Where-Object { $_.IPAddress -match '^192\.168\.' -or $_.IPAddress -match '^10\.' } |
        Select-Object -First 1 -ExpandProperty IPAddress

    if (-not $ip) {
        throw "Could not detect LAN IP. Use Wi-Fi and retry, or run: npx tauri android dev --host <your-ip>"
    }

    return $ip
}

function Ensure-AndroidEnv {
    if (-not $env:ANDROID_HOME) {
        $env:ANDROID_HOME = Join-Path $env:LOCALAPPDATA "Android\Sdk"
    }
    if (-not $env:NDK_HOME) {
        $ndkRoot = Join-Path $env:ANDROID_HOME "ndk"
        if (Test-Path $ndkRoot) {
            $env:NDK_HOME = Get-ChildItem $ndkRoot | Sort-Object Name -Descending | Select-Object -First 1 -ExpandProperty FullName
        }
    }
    if (-not $env:JAVA_HOME) {
        $studioJbr = "C:\Program Files\Android\Android Studio\jbr"
        if (Test-Path $studioJbr) {
            $env:JAVA_HOME = $studioJbr
        }
    }
    $env:Path = "$env:JAVA_HOME\bin;$env:ANDROID_HOME\platform-tools;$env:ANDROID_HOME\cmdline-tools\latest\bin;$env:Path"
}

function Set-NdkCargoEnv {
    $ndk = $env:NDK_HOME
    if (-not $ndk) {
        throw "NDK_HOME is not set"
    }

    $toolchain = Join-Path $ndk "toolchains\llvm\prebuilt\windows-x86_64\bin"
    $clang = Join-Path $toolchain "aarch64-linux-android34-clang.cmd"
    $ar = Join-Path $toolchain "llvm-ar.exe"

    if (-not (Test-Path $clang)) {
        $clang = Get-ChildItem $toolchain -Filter "aarch64-linux-android*-clang.cmd" |
            Sort-Object Name -Descending |
            Select-Object -First 1 -ExpandProperty FullName
    }

    if (-not $clang -or -not (Test-Path $clang)) {
        throw "NDK clang for aarch64-linux-android not found under $toolchain"
    }

    $env:CC_aarch64_linux_android = $clang
    $env:CXX_aarch64_linux_android = $clang
    $env:AR_aarch64_linux_android = $ar
    $env:CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER = $clang
}

function Remove-JniLibsEntry {
    param([string]$Path)

    if (-not (Test-Path $Path)) {
        return
    }

    $item = Get-Item $Path -Force
    if ($item.LinkType -eq "Junction") {
        cmd /c "rmdir `"$Path`"" | Out-Null
    } else {
        Remove-Item $Path -Recurse -Force
    }
}

function Setup-JniLibsJunction {
    param(
        [string]$Target = "aarch64-linux-android",
        [string]$Abi = "arm64-v8a"
    )

    $srcDir = Join-Path $ProjectRoot "src-tauri\target\$Target\debug"
    $jniLibsRoot = Join-Path $ProjectRoot "src-tauri\gen\android\app\src\main\jniLibs"
    $destDir = Join-Path $jniLibsRoot $Abi
    $validAbis = @("arm64-v8a", "armeabi-v7a", "x86", "x86_64")

    New-Item -ItemType Directory -Force -Path $srcDir | Out-Null
    New-Item -ItemType Directory -Force -Path $jniLibsRoot | Out-Null

    Get-ChildItem $jniLibsRoot -Force -ErrorAction SilentlyContinue | ForEach-Object {
        if ($validAbis -notcontains $_.Name) {
            Remove-JniLibsEntry $_.FullName
        }
    }

    Remove-JniLibsEntry $destDir

    $junction = cmd /c "mklink /J `"$destDir`" `"$srcDir`"" 2>&1
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to create jniLibs junction: $junction"
    }

    Write-Host "jniLibs junction ready" -ForegroundColor Green
}

function Set-AndroidDevConfig {
    param([string]$HostIp)

    $env:TAURI_CONFIG = "{ `"build`": { `"devUrl`": `"http://${HostIp}:${DevPort}`" } }"
}

function Clear-AndroidDevConfig {
    Remove-Item Env:TAURI_CONFIG -ErrorAction SilentlyContinue
}

function Start-ViteDevServer {
    param([string]$HostIp)

    $env:TAURI_DEV_HOST = $HostIp
    Write-Host "Starting Vite on http://${HostIp}:${DevPort} ..." -ForegroundColor Cyan

    $viteJs = Join-Path $ProjectRoot "node_modules\vite\bin\vite.js"
    if (-not (Test-Path $viteJs)) {
        throw "Vite not found. Run npm install in the project root first."
    }

    return Start-Process -FilePath "node" `
        -ArgumentList @($viteJs) `
        -WorkingDirectory $ProjectRoot `
        -PassThru `
        -NoNewWindow
}

function Wait-DevServerAlive {
    param([string]$HostIp)

    while ($true) {
        Start-Sleep -Seconds 2
        try {
            $client = New-Object System.Net.Sockets.TcpClient
            $client.Connect($HostIp, $DevPort)
            $client.Close()
        } catch {
            Write-Host "Vite dev server stopped." -ForegroundColor Yellow
            break
        }
    }
}

function Stop-ViteDevServer {
    param([int]$PreferredPid = 0)

    if ($PreferredPid -gt 0) {
        Stop-Process -Id $PreferredPid -Force -ErrorAction SilentlyContinue
    }

    Get-NetTCPConnection -LocalPort $DevPort -State Listen -ErrorAction SilentlyContinue |
        ForEach-Object {
            Stop-Process -Id $_.OwningProcess -Force -ErrorAction SilentlyContinue
        }
}

function Wait-ViteReady {
    param(
        [string]$HostIp,
        [int]$TimeoutSeconds = 90
    )

    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while ((Get-Date) -lt $deadline) {
        try {
            $client = New-Object System.Net.Sockets.TcpClient
            $client.Connect($HostIp, $DevPort)
            $client.Close()
            return
        } catch {
            Start-Sleep -Milliseconds 400
        }
    }

    throw "Vite did not start on http://${HostIp}:${DevPort} within ${TimeoutSeconds}s"
}

function Sync-AndroidNetworkConfig {
    $xmlDir = Join-Path $ProjectRoot "src-tauri\gen\android\app\src\main\res\xml"
    $xmlPath = Join-Path $xmlDir "network_security_config.xml"
    New-Item -ItemType Directory -Force -Path $xmlDir | Out-Null
    @'
<?xml version="1.0" encoding="utf-8"?>
<network-security-config>
    <base-config cleartextTrafficPermitted="true" />
</network-security-config>
'@ | Set-Content -Path $xmlPath -Encoding UTF8

    $manifestPath = Join-Path $ProjectRoot "src-tauri\gen\android\app\src\main\AndroidManifest.xml"
    if (-not (Test-Path $manifestPath)) {
        return
    }

    $manifest = Get-Content $manifestPath -Raw
    if ($manifest -notmatch "networkSecurityConfig") {
        $manifest = $manifest.Replace(
            'android:usesCleartextTraffic="${usesCleartextTraffic}"',
            'android:usesCleartextTraffic="${usesCleartextTraffic}" android:networkSecurityConfig="@xml/network_security_config"'
        )
        Set-Content -Path $manifestPath -Value $manifest -NoNewline
    }
}

function Sync-AndroidAssets {
    $assetsRoot = Join-Path $ProjectRoot "src-tauri\gen\android\app\src\main\assets"
    $configDest = Join-Path $assetsRoot "resources\stocks.json"
    $configSrc = Join-Path $ProjectRoot "src-tauri\resources\stocks.json"

    New-Item -ItemType Directory -Force -Path (Split-Path $configDest -Parent) | Out-Null
    Copy-Item $configSrc $configDest -Force
}

function Sync-LocalProperties {
    $androidRoot = Join-Path $ProjectRoot "src-tauri\gen\android"
    $propsPath = Join-Path $androidRoot "local.properties"
    $sdk = $env:ANDROID_HOME
    if (-not $sdk) {
        $sdk = Join-Path $env:LOCALAPPDATA "Android\Sdk"
    }
    $escaped = $sdk.Replace("\", "\\")
    "sdk.dir=$escaped" | Set-Content -Path $propsPath -Encoding ASCII
}

function Write-Utf8NoBomFile {
    param(
        [string]$Path,
        [string]$Content
    )

    $utf8NoBom = New-Object System.Text.UTF8Encoding $false
    [System.IO.File]::WriteAllText($Path, $Content.Replace("`r`n", "`n"), $utf8NoBom)
}

function Find-TauriAndroidPluginPath {
    $searchRoots = @()
    if ($env:CARGO_HOME) {
        $searchRoots += (Join-Path $env:CARGO_HOME "registry\src")
    }
    $searchRoots += (Join-Path $env:USERPROFILE ".cargo\registry\src")

    foreach ($registrySrc in $searchRoots) {
        if (-not (Test-Path $registrySrc)) { continue }
        $found = Get-ChildItem $registrySrc -Directory -Recurse -ErrorAction SilentlyContinue |
            Where-Object { $_.FullName -match '\\tauri-2\.\d[^\\]*\\mobile\\android$' } |
            Sort-Object Name -Descending |
            Select-Object -First 1 -ExpandProperty FullName
        if ($found) { return $found }
    }

    return $null
}

function Find-TemplateDir {
    param(
        [string]$Pattern
    )

    $searchRoots = @()
    if ($env:CARGO_HOME) {
        $searchRoots += (Join-Path $env:CARGO_HOME "registry\src")
    }
    $searchRoots += (Join-Path $env:USERPROFILE ".cargo\registry\src")

    foreach ($registrySrc in $searchRoots) {
        if (-not (Test-Path $registrySrc)) { continue }
        $found = Get-ChildItem $registrySrc -Directory -Recurse -ErrorAction SilentlyContinue |
            Where-Object { $_.FullName -match $Pattern } |
            Sort-Object FullName -Descending |
            Select-Object -First 1 -ExpandProperty FullName
        if ($found) { return $found }
    }

    return $null
}

function Copy-TemplatesToGenerated {
    param(
        [string]$TemplateDir,
        [string]$OutDir,
        [string]$PackageName,
        [string]$LibraryName,
        [switch]$PrefixAutoComment
    )

    if (-not $TemplateDir -or -not (Test-Path $TemplateDir)) {
        return
    }

    New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
    Get-ChildItem $TemplateDir -File | ForEach-Object {
        $content = Get-Content $_.FullName -Raw -Encoding UTF8
        $content = $content.
            Replace("{{package}}", $PackageName).
            Replace("{{package-unescaped}}", $PackageName).
            Replace("{{library}}", $LibraryName).
            Replace("{{class-extension}}", "").
            Replace("{{class-init}}", "")

        if ($PrefixAutoComment -and $_.Extension -eq ".kt" -and $content -notmatch "AUTO-GENERATED") {
            $content = "/* THIS FILE IS AUTO-GENERATED. DO NOT MODIFY!! */`n`n" + $content
        }

        Write-Utf8NoBomFile -Path (Join-Path $OutDir $_.Name) -Content $content
    }
}

function Ensure-GeneratedKotlinSources {
    $androidRoot = Join-Path $ProjectRoot "src-tauri\gen\android"
    $outDir = Join-Path $androidRoot "app\src\main\java\com\stockpredict\app\generated"
    $packageName = "com.stockpredict.app"
    $libraryName = "stock_predict_lib"

    $wryTemplates = Find-TemplateDir -Pattern '\\wry-0\.\d[^\\]*\\src\\android\\kotlin$'
    $tauriCodegen = Find-TemplateDir -Pattern '\\tauri-2\.\d[^\\]*\\mobile\\android-codegen$'

    if (-not $wryTemplates -and -not $tauriCodegen) {
        throw "Cannot find wry/tauri Android Kotlin templates under Cargo registry"
    }

    Copy-TemplatesToGenerated -TemplateDir $wryTemplates -OutDir $outDir `
        -PackageName $packageName -LibraryName $libraryName -PrefixAutoComment
    Copy-TemplatesToGenerated -TemplateDir $tauriCodegen -OutDir $outDir `
        -PackageName $packageName -LibraryName $libraryName

    Write-Host "Generated Kotlin sources in app/.../generated" -ForegroundColor Green
}

function Ensure-TauriGradleFiles {
    $androidRoot = Join-Path $ProjectRoot "src-tauri\gen\android"
    $settingsPath = Join-Path $androidRoot "tauri.settings.gradle"
    $buildPath = Join-Path $androidRoot "app\tauri.build.gradle.kts"
    $vendorDir = Join-Path $androidRoot "tauri-android"

    $pluginPath = Find-TauriAndroidPluginPath
    if (-not $pluginPath) {
        if ((Test-Path $settingsPath) -and (Test-Path $buildPath) -and (Test-Path $vendorDir)) {
            return
        }
        throw "tauri mobile/android plugin not found under Cargo registry"
    }

    # Kotlin incremental compile breaks across drive roots (C: cargo vs D: project).
    # Vendor the plugin onto the project drive.
    if (Test-Path $vendorDir) {
        Remove-Item $vendorDir -Recurse -Force
    }
    New-Item -ItemType Directory -Force -Path $vendorDir | Out-Null
    Copy-Item (Join-Path $pluginPath "*") $vendorDir -Recurse -Force

    Write-Utf8NoBomFile -Path $settingsPath -Content @"
// THIS IS AN AUTOGENERATED FILE. DO NOT EDIT THIS FILE DIRECTLY.
include ':tauri-android'
project(':tauri-android').projectDir = new File(settingsDir, 'tauri-android')
"@

    New-Item -ItemType Directory -Force -Path (Split-Path $buildPath -Parent) | Out-Null
    Write-Utf8NoBomFile -Path $buildPath -Content @"
// THIS IS AN AUTOGENERATED FILE. DO NOT EDIT THIS FILE DIRECTLY.
val implementation by configurations
dependencies {
  implementation("androidx.lifecycle:lifecycle-process:2.10.0")
  implementation(project(":tauri-android"))
}
"@
}

function Invoke-AndroidCargoBuild {
    $androidRoot = Join-Path $ProjectRoot "src-tauri\gen\android"
    $kotlinOut = Join-Path $androidRoot "app\src\main\java\com\stockpredict\app\generated"
    New-Item -ItemType Directory -Force -Path $kotlinOut | Out-Null

    $env:TAURI_ANDROID_PROJECT_PATH = $androidRoot
    $env:TAURI_ANDROID_PACKAGE_UNESCAPED = "com.stockpredict.app"
    $env:WRY_ANDROID_PACKAGE = "com.stockpredict.app"
    $env:WRY_ANDROID_LIBRARY = "stock_predict_lib"
    $env:WRY_ANDROID_KOTLIN_FILES_OUT_DIR = $kotlinOut

    # Force build.rs to re-run so Tauri can emit Gradle include files.
    $buildRs = Join-Path $ProjectRoot "src-tauri\build.rs"
    if (Test-Path $buildRs) {
        (Get-Item $buildRs).LastWriteTime = Get-Date
    }

    Push-Location (Join-Path $ProjectRoot "src-tauri")
    try {
        & cargo build --target aarch64-linux-android --lib
        if ($LASTEXITCODE -ne 0) {
            throw "cargo build failed"
        }
    } finally {
        Pop-Location
    }

    # Always normalize generated sources + same-drive plugin path after cargo.
    Ensure-GeneratedKotlinSources
    Ensure-TauriGradleFiles
}

function Clear-StaleGradleLocks {
    $androidRoot = Join-Path $ProjectRoot "src-tauri\gen\android"
    $lockDir = Join-Path $androidRoot ".gradle"
    if (Test-Path $lockDir) {
        Get-ChildItem $lockDir -Recurse -Filter "*.lock" -ErrorAction SilentlyContinue | ForEach-Object {
            Remove-Item $_.FullName -Force -ErrorAction SilentlyContinue
        }
    }

    Push-Location $androidRoot
    try {
        if (Test-Path ".\gradlew.bat") {
            & .\gradlew.bat --stop 2>$null | Out-Null
        }
    } finally {
        Pop-Location
    }
}

function Invoke-AndroidDevCopyFallback {
    Ensure-AndroidEnv
    Set-NdkCargoEnv
    $ip = Get-LanIpAddress

    Write-Host ""
    Write-Host "Developer Mode is on but symlink still blocked on this PC." -ForegroundColor Yellow
    Write-Host "Using junction + cargo + Gradle fallback (no symlink needed)." -ForegroundColor Yellow
    Write-Host "Tip: restart Windows or use an Administrator terminal for the normal flow." -ForegroundColor Gray
    Write-Host ""

    Setup-JniLibsJunction
    Sync-AndroidAssets
    Sync-AndroidNetworkConfig
    Sync-LocalProperties
    Ensure-GeneratedKotlinSources
    Ensure-TauriGradleFiles
    Clear-StaleGradleLocks
    Set-AndroidDevConfig -HostIp $ip
    $vite = Start-ViteDevServer -HostIp $ip

    try {
        Wait-ViteReady -HostIp $ip

        Write-Host "Building Rust for Android..." -ForegroundColor Cyan
        Invoke-AndroidCargoBuild

        Write-Host "Installing debug APK..." -ForegroundColor Cyan
        Push-Location (Join-Path $ProjectRoot "src-tauri\gen\android")
        try {
            & .\gradlew.bat installArm64Debug `
                -x rustBuildArm64Debug `
                -x rustBuildUniversalDebug
            if ($LASTEXITCODE -ne 0) {
                $apk = Join-Path $ProjectRoot "src-tauri\gen\android\app\build\outputs\apk\arm64\debug\app-arm64-debug.apk"
                Write-Host ""
                Write-Host "Gradle install failed." -ForegroundColor Red
                if (Test-Path $apk) {
                    Write-Host "APK is ready at:" -ForegroundColor Yellow
                    Write-Host "  $apk" -ForegroundColor Yellow
                    Write-Host "If you see INSTALL_FAILED_USER_RESTRICTED (Xiaomi/HyperOS):" -ForegroundColor Yellow
                    Write-Host "  1. Phone: Settings -> Additional settings -> Developer options" -ForegroundColor Gray
                    Write-Host "  2. Enable: USB debugging + Install via USB (USB安装)" -ForegroundColor Gray
                    Write-Host "  3. Unlock phone, tap Allow when the USB install prompt appears" -ForegroundColor Gray
                    Write-Host "  4. Retry: adb install -r `"$apk`"" -ForegroundColor Gray
                }
                throw "Gradle installArm64Debug failed"
            }
        } finally {
            Pop-Location
        }

        Write-Host "Launching app..." -ForegroundColor Cyan
        & adb shell am start -n $AppActivity | Out-Null

        Write-Host ""
        Write-Host "Installed. Dev server: http://${ip}:${DevPort}" -ForegroundColor Green
        Write-Host "Press Ctrl+C to stop Vite." -ForegroundColor Gray
        Write-Host ""

        if ($vite -and -not $vite.HasExited) {
            Wait-Process -Id $vite.Id -ErrorAction SilentlyContinue
        } else {
            Wait-DevServerAlive -HostIp $ip
        }
    } finally {
        Stop-ViteDevServer -PreferredPid $(if ($vite) { $vite.Id } else { 0 })
        Clear-AndroidDevConfig
    }
}

Set-Location $ProjectRoot
Ensure-AndroidEnv

if (-not (Test-Path (Join-Path $ProjectRoot "src-tauri\gen\android"))) {
    throw "Android project not initialized. Run: npm run android:init"
}

if (Test-SymlinkCreationAllowed) {
    & npx tauri android dev @Args
} else {
    Invoke-AndroidDevCopyFallback
}
