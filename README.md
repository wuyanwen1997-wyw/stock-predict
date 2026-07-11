# StockPredict

基于 **Tauri 2 + React 19 + Vite + Tailwind CSS 4 + Rust** 的股票预测桌面应用，技术栈与 [film-shell](../film-shell) 一致。

## 功能（v0.1）

- 内置 20 只 A 股样本，支持选择与自选股
- 明日 **涨 / 跌 / 平** 概率可视化（环形仪表 + 比例条）
- **高开 / 低开** 两种开盘场景的日内走势预测图
- 预测算法占位模块（`predictor.rs`），预留 LSTM / XGBoost 接口
- 深色玻璃拟态 UI，Framer Motion 动画 + Recharts 图表

## 快速开始

```bash
cd stock-predict
npm install
npm run tauri:dev
```

仅前端预览（无 Tauri 后端，API 会失败）：

```bash
npm run dev
```

## 项目结构

```
stock-predict/
├── src/                    # React 前端
│   ├── components/         # UI 组件
│   ├── pages/              # 预测 / 自选 / 设置
│   ├── stores/             # Zustand 状态
│   └── services/api.ts     # Tauri invoke 封装
└── src-tauri/
    ├── src/predictor.rs    # 预测算法（可替换）
    └── resources/stocks.json
```

## 接入真实算法

1. 在 `src-tauri/src/predictor.rs` 实现新算法
2. 在 `commands.rs` 的 `list_algorithms` 中注册并启用
3. 按需接入行情 API（如 akshare、tushare、东方财富等）

> 免责声明：当前预测结果为演示数据，不构成任何投资建议。
