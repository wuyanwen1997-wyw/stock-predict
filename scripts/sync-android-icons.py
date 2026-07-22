#!/usr/bin/env python3
"""Copy src-tauri/icons/android into gen/android app res (launcher icons)."""
from __future__ import annotations

import shutil
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "src-tauri" / "icons" / "android"
DST = ROOT / "src-tauri" / "gen" / "android" / "app" / "src" / "main" / "res"


def main() -> int:
    if not SRC.is_dir():
        print(f"missing icon source: {SRC}", file=sys.stderr)
        return 1
    if not DST.is_dir():
        print(
            "missing gen/android res — run: npm run android:init",
            file=sys.stderr,
        )
        return 1

    copied = 0
    for path in SRC.rglob("*"):
        if not path.is_file():
            continue
        rel = path.relative_to(SRC)
        target = DST / rel
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(path, target)
        copied += 1
        print(f"  {rel}")

    print(f"synced {copied} android icon file(s) -> {DST.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
