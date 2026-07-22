#!/usr/bin/env python3
"""Patch gen/android MainActivity so background-service loads stock_predict_lib."""
from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MAIN = (
    ROOT
    / "src-tauri"
    / "gen"
    / "android"
    / "app"
    / "src"
    / "main"
    / "java"
    / "com"
    / "stockpredict"
    / "app"
    / "MainActivity.kt"
)

CONTENT = """package com.stockpredict.app

import android.os.Bundle
import androidx.activity.enableEdgeToEdge
import app.tauri.backgroundservice.HeadlessBridge

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    // 插件默认 loadLibrary("app_core")；本应用 cdylib 为 stock_predict_lib
    HeadlessBridge.nativeLibName = "stock_predict_lib"
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
  }
}
"""


def main() -> int:
    if not MAIN.parent.is_dir():
        print(f"missing android gen — run: npm run android:init", flush=True)
        return 1
    MAIN.write_text(CONTENT, encoding="utf-8")
    print(f"patched {MAIN.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
