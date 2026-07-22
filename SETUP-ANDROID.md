# StockPredict Android 构建指南

StockPredict 与 [film-shell](../film-shell) 一样基于 **Tauri 2 Mobile**：同一套 React + Rust，桌面与 Android 共用业务逻辑（行情、策略组合、回测）。

包名 / identifier：`com.stockpredict.app`

## 环境要求

| 工具 | 版本 | 说明 |
|------|------|------|
| Node.js | 18+ | 与桌面版相同 |
| Rust | stable | [rustup](https://rustup.rs/) |
| JDK | 17 | Android Gradle / Android Studio JBR |
| Android Studio | 最新 | 含 SDK Manager、NDK |
| Android SDK | API 34+ | 通过 Android Studio 安装 |
| Android NDK | r26+ | SDK Manager → SDK Tools → NDK |

## 1. 安装 JDK 17

```powershell
winget install EclipseAdoptium.Temurin.17.JDK
```

或直接使用 Android Studio 自带的 JBR：

```powershell
$env:JAVA_HOME = "C:\Program Files\Android\Android Studio\jbr"
$env:Path = "$env:JAVA_HOME\bin;$env:Path"
```

## 2. 安装 Android Studio / SDK / NDK

1. 下载 [Android Studio](https://developer.android.com/studio)
2. 打开 **SDK Manager**，安装：
   - Android SDK Platform 34（或更高）
   - Android SDK Build-Tools
   - Android SDK Command-line Tools
   - **NDK (Side by side)**
3. 设置环境变量：

```powershell
$env:ANDROID_HOME = "$env:LOCALAPPDATA\Android\Sdk"
$env:NDK_HOME = "$env:ANDROID_HOME\ndk\<版本号>"
$env:Path = "$env:ANDROID_HOME\platform-tools;$env:ANDROID_HOME\cmdline-tools\latest\bin;$env:Path"
```

检查环境：

```powershell
npm run android:setup
```

## 3. 初始化 Android 工程（首次）

```powershell
cd D:\workspace\stock-predict
npm install
npm run android:init
```

会在 `src-tauri/gen/android/` 生成 Gradle 工程（已在 `.gitignore`，每台机器执行一次）。

可选：提前添加 Android Rust 目标：

```powershell
rustup target add aarch64-linux-android
```

## 4. 开发调试

连接真机并开启 USB 调试，或启动模拟器：

```powershell
npm run android:dev
```

- 正常情况走 `tauri android dev`
- Windows 禁止符号链接时，脚本会自动 fallback：LAN Vite + `cargo` + `gradlew installArm64Debug`

Vite 开发端口为 **1421**（与桌面一致）。

## 5. 打包 APK

```powershell
npm run android:build
```

输出路径示例：

```
src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk
```

Release 签名见 [Tauri Android 签名文档](https://v2.tauri.app/distribute/sign/android/)。

## 6. GitHub Actions 打 APK

仓库已提供 [`.github/workflows/android-apk.yml`](./.github/workflows/android-apk.yml)：

| 触发方式 | 说明 |
|---------|------|
| Actions → **Android APK** → Run workflow | 手动构建，产物在 Artifacts |
| 推送 tag `v*`（如 `v0.1.0`） | 构建并把 APK 挂到 GitHub Release |

CI 每次会 `tauri android init`（`gen/android` 在 `.gitignore`），再 `tauri android build --apk`。

未配置签名时产物为 **unsigned release APK**。本仓库已配置 Actions Secrets（`ANDROID_KEY_*`），CI 发版会打**签名包**。

本地 keystore 在 `signing/`（已 gitignore，**勿提交、勿丢失**）：

| 文件 | 说明 |
|------|------|
| `signing/upload-keystore.jks` | 签名证书 |
| `signing/credentials.env` | 密码与 base64 备份（仅本机） |
| `signing/keystore.properties` | 供 Gradle 读取 |

`android init` 之后若要本地打签名包：

```powershell
npm run android:signing   # 同步到 gen/android 并 patch Gradle
npm run android:build
```

## 7. 改版本号

一次改齐 Node / Tauri / Cargo（勿手改多处）：

```bash
npm run version:get
npm run version:bump -- --auto                # patch +1，只改文件
npm run version:bump -- --auto --tag --push   # commit + tag + push → 触发 CI
```

详见 `.cursor/skills/release/SKILL.md`。

## 平台差异

| 功能 | Windows 桌面 | Android |
|------|-------------|---------|
| 热股 / 搜索 / 预测 | ✅ | ✅ |
| 多信号策略组合 | ✅ | ✅ |
| K 线 / 回测 | ✅ | ✅ |
| 自选 / 策略本地存储 | ✅ | ✅（WebView localStorage） |
| 内置 `stocks.json` | ✅ | ✅ |

## 常见问题

**`Java not found`**  
确认 `JAVA_HOME` 与 `java -version` 后重试 `npm run android:init`。

**`NDK not found`**  
在 Android Studio SDK Manager 安装 NDK，并设置 `NDK_HOME`。

**修改包名 / identifier 后构建失败**  
删除 `src-tauri/gen/android` 后重新 `npm run android:init`。

**`Could not read script ... tauri.settings.gradle`**  
该文件由 Tauri `build.rs` 在设置了 `TAURI_ANDROID_PROJECT_PATH` 时生成，`android init` 不会创建。`npm run android:dev` 的 fallback 路径已自动生成；若仍报错，先结束旧的 Gradle/Java 进程后重试。

**`Creation symbolic link is not allowed for this system`**  
脚本会自动走 junction 备用方案；若仍失败，重启或以管理员运行终端。

**`this and base files have different roots` / `Unresolved reference: TauriActivity`**  
项目在 D:、Cargo 在 C: 时，Kotlin 跨盘编译会失败，且 fallback 可能缺少 `generated/TauriActivity.kt`。`android:dev` 现会把 `tauri-android` 拷到工程盘并生成源码。仍可选用：

```powershell
$env:CARGO_HOME = "D:\workspace\stock-predict\.cargo-home"
```

**开发时手机打不开行情**  
确认手机与电脑同一局域网，并允许 Windows 防火墙放行 Node/Vite 端口 1421。
