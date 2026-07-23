# 以太测（StockPredict）

基于 **Tauri 2 + React 19 + Vite + Tailwind CSS 4 + Rust** 的 A 股研究演示应用，技术栈与 [film-shell](../film-shell) 一致，同时支持 **Windows 桌面** 与 **Android**。

按炒股流程组织界面：**行情 → 选股 → 股票池 → 买点 → 持仓 → 卖点 → 复盘**；差异化在可解释组合信号、同构回测与本地池/仓研究（不接券商下单）。

## 功能

- **五 Tab 主导航**：行情 / 选股 / 股票池 / 持仓 / 复盘；个股工作台（诊股 · 行情 · 买卖点 · 情景）不占底栏
- **股票池**：自定义分组（关注 / 待买 / 观察 / 已剔除等）；旧自选一次性迁入「关注」；盯盘、买点筛选、多股同步对比
- **持仓 / 复盘**：本地成本与数量、浮动盈亏；盘后摘要、预测对照、B/S 回顾与笔记（研究向，不接券商）
- **智能选股**：硬过滤 + 策略打分 TopN → 入池 / 诊股
- **组合诊股**：多信号策略（技术面 / 消息面 / 政策面 / 美股等）按股持久化；概率白话结论、滚动回测、高开/低开情景
- **技术图**：多周期 K + 指标窗格；日 K MACD B/S 标注；沉浸看图（全屏/横屏）
- **盯盘助手**：规则预警、系统通知；Android 前台服务支持锁屏监控
- **用户态**：`app_data_dir/user_data.sqlite`（schema v3：池 / 持仓 / 复盘）；设置页导出/导入备份；升级覆盖安装不丢数据

深色玻璃拟态 UI：手机底栏五 Tab · 宽屏左侧导航。

> 免责声明：预测与技术标记仅供研究演示，不构成任何投资建议。

## 快速开始（桌面）

```bash
cd stock-predict
npm install
npm run tauri:dev
```

仅前端预览（无 Tauri 后端，API 会失败）：

```bash
npm run dev
```

## Android

详见 [SETUP-ANDROID.md](./SETUP-ANDROID.md)。摘要：

```powershell
npm run android:setup   # 环境检查
npm run android:init    # 首次生成 gen/android
npm run android:dev     # 真机 / 模拟器调试
npm run android:build   # 打 APK
```

也可在 GitHub Actions 打 APK：手动运行 **Android APK** workflow，或推送 `v*` tag（详见 [SETUP-ANDROID.md](./SETUP-ANDROID.md)）。

发版改版本号请用脚本（一次改齐 Node / Tauri / Cargo），勿手改多处：

```bash
npm run version:get
npm run version:bump -- --auto                # patch +1，只改文件
npm run version:bump -- --auto --tag --push   # + commit + tag + push（触发 APK CI）
npm run version:bump -- 0.1.2 --tag           # 也可显式指定版本
```

Agent 维护说明见 `.cursor/skills/release/SKILL.md`。

## 项目结构

```
stock-predict/
├── src/                         # React 前端（桌面 + Android 共用）
│   ├── components/
│   ├── pages/                   # 行情 / 选股 / 池 / 持仓 / 复盘 / 个股工作台 / 设置
│   ├── stores/
│   └── services/                # Tauri invoke 与用户态持久化
├── docs/                        # 设计说明与产品架构文档（见 docs/README.md）
├── scripts/                     # Android 开发与环境检查脚本
├── SETUP-ANDROID.md
└── src-tauri/
    ├── src/                     # 行情 / 策略 / 预测 / 回测 / user_store
    ├── tauri.android.conf.json
    ├── capabilities/android.json
    └── resources/stocks.json
```

## 文档

产品与技术说明见 [docs/README.md](./docs/README.md)；信息架构以 [docs/产品架构升级方案.md](./docs/产品架构升级方案.md) 为准（Phase A/B/C 已落地）。
