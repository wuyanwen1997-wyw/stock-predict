# StockPredict

基于 **Tauri 2 + React 19 + Vite + Tailwind CSS 4 + Rust** 的股票预测应用，技术栈与 [film-shell](../film-shell) 一致，同时支持 **Windows 桌面** 与 **Android**。

## 功能

- A 股热股 / 搜索选股，自选股本地保存
- 多信号策略组合（技术面 / 消息面 / 政策面 / 美股等），按股票持久化
- 明日涨跌概率、K 线、滚动回测与高开/低开场景图
- 深色玻璃拟态 UI（桌面侧栏 / 手机底部导航）

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

## 项目结构

```
stock-predict/
├── src/                         # React 前端（桌面 + Android 共用）
│   ├── components/
│   ├── pages/
│   ├── stores/
│   └── services/api.ts          # Tauri invoke 封装
├── scripts/                     # Android 开发与环境检查脚本
├── SETUP-ANDROID.md
└── src-tauri/
    ├── src/                     # 行情 / 策略 / 预测 / 回测
    ├── tauri.android.conf.json
    ├── capabilities/android.json
    └── resources/stocks.json
```

> 免责声明：预测结果仅供研究演示，不构成任何投资建议。
