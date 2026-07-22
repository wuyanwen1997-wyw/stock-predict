#!/usr/bin/env python3
"""Copy signing/keystore.properties into gen/android and patch app/build.gradle.kts."""
from __future__ import annotations

import shutil
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PROPS_SRC = ROOT / "signing" / "keystore.properties"
ANDROID_ROOT = ROOT / "src-tauri" / "gen" / "android"
GRADLE = ANDROID_ROOT / "app" / "build.gradle.kts"


def patch_gradle(path: Path) -> None:
    text = path.read_text(encoding="utf-8")
    if "signingConfigs" in text:
        print("signingConfigs already present")
        return
    if "import java.io.FileInputStream" not in text:
        text = text.replace(
            "import java.util.Properties",
            "import java.util.Properties\nimport java.io.FileInputStream",
        )
    signing = """
    signingConfigs {
        create("release") {
            val keystorePropertiesFile = rootProject.file("keystore.properties")
            val keystoreProperties = Properties()
            if (keystorePropertiesFile.exists()) {
                keystoreProperties.load(FileInputStream(keystorePropertiesFile))
                keyAlias = keystoreProperties["keyAlias"] as String
                keyPassword = keystoreProperties["password"] as String
                storeFile = file(keystoreProperties["storeFile"] as String)
                storePassword = keystoreProperties["password"] as String
            }
        }
    }
"""
    text = text.replace("    buildTypes {", signing + "    buildTypes {", 1)
    old = '        getByName("release") {\n            isMinifyEnabled = true'
    new = (
        '        getByName("release") {\n'
        "            signingConfig = signingConfigs.getByName(\"release\")\n"
        "            isMinifyEnabled = true"
    )
    if old not in text:
        raise SystemExit("failed to patch release signingConfig in build.gradle.kts")
    path.write_text(text.replace(old, new, 1), encoding="utf-8")
    print(f"patched {path}")


def main() -> int:
    if not PROPS_SRC.is_file():
        print(f"missing {PROPS_SRC}", file=sys.stderr)
        return 1
    if not ANDROID_ROOT.is_dir():
        print("missing gen/android — run: npm run android:init", file=sys.stderr)
        return 1
    shutil.copy2(PROPS_SRC, ANDROID_ROOT / "keystore.properties")
    print(f"copied {PROPS_SRC.name} -> {ANDROID_ROOT / 'keystore.properties'}")
    patch_gradle(GRADLE)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
