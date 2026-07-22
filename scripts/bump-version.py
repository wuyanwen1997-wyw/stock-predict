#!/usr/bin/env python3
"""Bump StockPredict version across Node / Tauri / Cargo; optional commit/tag/push."""
from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PACKAGE_JSON = ROOT / "package.json"
PACKAGE_LOCK = ROOT / "package-lock.json"
TAURI_CONF = ROOT / "src-tauri" / "tauri.conf.json"
CARGO_TOML = ROOT / "src-tauri" / "Cargo.toml"
CARGO_LOCK = ROOT / "src-tauri" / "Cargo.lock"

VERSION_FILES = (
    PACKAGE_JSON,
    PACKAGE_LOCK,
    TAURI_CONF,
    CARGO_TOML,
    CARGO_LOCK,
)

SEMVER = re.compile(r"^v?(\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?)$")


def normalize(version: str) -> str:
    m = SEMVER.match(version.strip())
    if not m:
        raise SystemExit(f"invalid semver: {version!r} (expected 0.1.2 or v0.1.2)")
    return m.group(1)


def read_current() -> str:
    data = json.loads(PACKAGE_JSON.read_text(encoding="utf-8"))
    return str(data["version"])


def auto_bump_patch(current: str) -> str:
    """Increment the last numeric segment: 0.1.1 -> 0.1.2 (drops pre-release/build)."""
    core = normalize(current).split("+", 1)[0].split("-", 1)[0]
    parts = core.split(".")
    if len(parts) != 3 or not all(p.isdigit() for p in parts):
        raise SystemExit(f"--auto needs X.Y.Z current version, got {current!r}")
    major, minor, patch = (int(p) for p in parts)
    return f"{major}.{minor}.{patch + 1}"


def run_git(args: list[str], *, check: bool = True) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["git", *args],
        cwd=ROOT,
        check=check,
        text=True,
        capture_output=True,
    )


def patch_json_version_field(path: Path, new: str, *, occurrences: int) -> None:
    """Replace only `"version": "..."` occurrences without reformatting the file."""
    text = path.read_text(encoding="utf-8")
    updated, n = re.subn(
        r'("version"\s*:\s*")([^"]+)(")',
        rf"\g<1>{new}\g<3>",
        text,
        count=occurrences,
    )
    if n != occurrences:
        raise SystemExit(
            f"expected {occurrences} version field(s) in {path}, patched {n}"
        )
    path.write_text(updated, encoding="utf-8", newline="\n")


def replace_cargo_toml(path: Path, new: str) -> None:
    text = path.read_text(encoding="utf-8")
    updated, n = re.subn(
        r'(?m)^version\s*=\s*"[^"]+"',
        f'version = "{new}"',
        text,
        count=1,
    )
    if n != 1:
        raise SystemExit(f"failed to patch version in {path}")
    path.write_text(updated, encoding="utf-8", newline="\n")


def replace_cargo_lock(path: Path, new: str) -> None:
    text = path.read_text(encoding="utf-8")
    pattern = re.compile(
        r'(name = "stock-predict"\nversion = ")[^"]+(")',
        re.MULTILINE,
    )
    updated, n = pattern.subn(rf"\g<1>{new}\g<2>", text, count=1)
    if n != 1:
        raise SystemExit(f"failed to patch stock-predict version in {path}")
    path.write_text(updated, encoding="utf-8", newline="\n")


def bump(new: str) -> str:
    old = read_current()

    patch_json_version_field(PACKAGE_JSON, new, occurrences=1)
    patch_json_version_field(PACKAGE_LOCK, new, occurrences=2)
    patch_json_version_field(TAURI_CONF, new, occurrences=1)
    replace_cargo_toml(CARGO_TOML, new)
    if CARGO_LOCK.is_file():
        replace_cargo_lock(CARGO_LOCK, new)

    print(f"{old} -> {new}")
    print("updated:")
    for p in VERSION_FILES:
        if p.is_file():
            print(f"  - {p.relative_to(ROOT)}")
    return old


def commit_version(new: str) -> None:
    rels = [str(p.relative_to(ROOT)).replace("\\", "/") for p in VERSION_FILES if p.is_file()]
    run_git(["add", "--", *rels])
    # Skip empty commit if nothing staged for these paths
    staged = run_git(["diff", "--cached", "--name-only", "--", *rels])
    if not staged.stdout.strip():
        print("commit: nothing to commit (version files unchanged)")
        return
    msg = f"chore: bump version to {new}"
    run_git(["commit", "-m", msg])
    print(f"commit: {msg}")


def create_tag(new: str) -> str:
    tag = f"v{new}"
    existing = run_git(["tag", "-l", tag], check=False)
    if existing.stdout.strip():
        raise SystemExit(f"tag already exists: {tag}")
    run_git(["tag", "-a", tag, "-m", tag])
    print(f"tag: {tag}")
    return tag


def push_release(tag: str | None) -> None:
    # Push current branch
    branch = run_git(["rev-parse", "--abbrev-ref", "HEAD"]).stdout.strip()
    if branch == "HEAD":
        raise SystemExit("detached HEAD: checkout a branch before --push")
    print(f"push: branch {branch}")
    proc = run_git(["push", "-u", "origin", "HEAD"], check=False)
    if proc.returncode != 0:
        sys.stderr.write(proc.stderr)
        raise SystemExit(f"git push branch failed ({proc.returncode})")
    if tag:
        print(f"push: tag {tag}")
        proc = run_git(["push", "origin", tag], check=False)
        if proc.returncode != 0:
            sys.stderr.write(proc.stderr)
            raise SystemExit(f"git push tag failed ({proc.returncode})")
    print("push: done")


def main() -> int:
    parser = argparse.ArgumentParser(
        description=(
            "Bump version in package.json / lockfiles / tauri.conf.json / Cargo. "
            "Optionally commit, create annotated tag vX.Y.Z, and push."
        )
    )
    parser.add_argument(
        "version",
        nargs="?",
        help="New version, e.g. 0.1.2 or v0.1.2 (omit when using --auto)",
    )
    parser.add_argument(
        "--get",
        action="store_true",
        help="Print current package.json version and exit",
    )
    parser.add_argument(
        "--auto",
        action="store_true",
        help="Bump patch (+1 on last digit), e.g. 0.1.1 -> 0.1.2; do not pass version",
    )
    parser.add_argument(
        "--commit",
        action="store_true",
        help="git add version files and commit (chore: bump version to X.Y.Z)",
    )
    parser.add_argument(
        "--tag",
        action="store_true",
        help="Create annotated tag vX.Y.Z (implies --commit)",
    )
    parser.add_argument(
        "--push",
        action="store_true",
        help="git push current branch; with --tag also push the tag (triggers APK CI)",
    )
    args = parser.parse_args()

    if args.get:
        print(read_current())
        return 0

    if args.auto and args.version:
        raise SystemExit("use either --auto or an explicit version, not both")
    if not args.auto and not args.version:
        parser.print_help()
        print(f"\ncurrent: {read_current()}", file=sys.stderr)
        return 1

    if args.push and not (args.tag or args.commit):
        # Allow bump+push without tag only if also committing; otherwise refuse
        # confusing half-states. Require --commit or --tag with --push.
        raise SystemExit("--push requires --commit and/or --tag")

    if args.auto:
        new = auto_bump_patch(read_current())
        print(f"auto: {new}")
    else:
        new = normalize(args.version)
    bump(new)

    do_commit = args.commit or args.tag
    tag_name: str | None = None
    if do_commit:
        commit_version(new)
    if args.tag:
        tag_name = create_tag(new)
    if args.push:
        push_release(tag_name)

    if not do_commit and not args.push:
        print("\nnext: commit, or re-run with --tag / --tag --push")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
