---
name: release
description: >-
  Bumps app version across Node/Tauri/Cargo, tags releases, and maintains
  Android CI signing/APK workflows. Use when the user asks to bump version,
  cut a release, tag v*, publish APK, or change version numbers / GitHub
  Actions Android packaging.
---

# Release / 版本发布

## 必读

- 版本脚本：`scripts/bump-version.py`
- Android 图标同步：`scripts/sync-android-icons.py`（`npm run android:icons`）
- Android CI：`.github/workflows/android-apk.yml`
- 签名本地备份：`signing/`（**gitignore，勿提交**）
- 说明：`SETUP-ANDROID.md`（GitHub Actions / Secrets）

## 版本源（必须一起改）

| 文件 | 用途 |
|------|------|
| `package.json` | npm |
| `package-lock.json` | 与 package.json 对齐 |
| `src-tauri/tauri.conf.json` | **打进 App/APK 的 versionName** |
| `src-tauri/Cargo.toml` | Rust crate |
| `src-tauri/Cargo.lock` | 与 Cargo.toml 对齐 |

**禁止**手改多处 version。一律用脚本。

## 命令

```bash
npm run version:get

# patch +1（0.1.1 -> 0.1.2），只改文件
npm run version:bump -- --auto

# 显式版本，只改文件
npm run version:bump -- 0.1.2

# 改文件 + commit + 打 annotated tag
npm run version:bump -- --auto --tag
npm run version:bump -- 0.1.2 --tag

# 一条龙：auto + commit + tag + push（触发 Android APK CI）
npm run version:bump -- --auto --tag --push

# 只要 commit、暂不 tag
npm run version:bump -- --auto --commit

# commit 后 push 当前分支（不打 tag）
npm run version:bump -- --auto --commit --push
```

等价：`python scripts/bump-version.py ...`  
接受 `0.1.2` / `v0.1.2`；写入与 tag 名用去 `v` 后的 semver（tag 为 `vX.Y.Z`）。

| 标志 | 行为 |
|------|------|
| `--auto` | 当前版本最后一位 +1，无需传 version |
| （显式 version） | 设为指定版本 |
| `--commit` | `git add` 版本文件并 `chore: bump version to X.Y.Z` |
| `--tag` | 隐含 `--commit`，再 `git tag -a vX.Y.Z` |
| `--push` | 需配合 `--commit` 和/或 `--tag`；`git push` 当前分支，有 tag 则再 push tag |

`--push` 会推远程并可能触发 CI，执行前确认用户要发版。

## 发版流程（推荐一条龙）

```bash
npm run version:bump -- --auto --tag --push
```

推送 `v*` tag → **Android APK** workflow → Artifacts + GitHub Release。

CI 在 `android init` 后会跑 `sync-android-icons.py`，把 `src-tauri/icons/android/` 拷进 `gen/.../res/`；否则 APK 是默认占位图标。本地同样：`npm run android:init && npm run android:icons`。

仅重跑 CI、不改版本：Actions 手动 Run，或重建 tag（慎用）。

## 签名（GitHub Actions Secrets）

- `ANDROID_KEY_BASE64` / `ANDROID_KEY_ALIAS` / `ANDROID_KEY_PASSWORD`
- 本地：`signing/`（勿提交）；`npm run android:signing` 同步到 `gen/android`

## Agent 清单

- [ ] 用 `bump-version.py`，不手工散改
- [ ] 默认发版用 `--tag --push`；用户未要求 push 时不要加 `--push`
- [ ] 不提交 `signing/`
- [ ] CI 失败：Gradle 缓存在 `android init` **之后**；Node 24+
