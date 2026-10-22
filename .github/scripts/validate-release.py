"""Validate release metadata before GitHub Actions builds distributable assets."""

import json
import re
import sys
import tomllib
from pathlib import Path


SEMVER = re.compile(r"^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$")


def read_cargo_version(path: Path) -> str:
    with path.open("rb") as file:
        return tomllib.load(file)["package"]["version"]


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: validate-release.py vX.Y.Z", file=sys.stderr)
        return 2

    tag = sys.argv[1]
    version = tag.removeprefix("v")
    if not tag.startswith("v") or not SEMVER.fullmatch(version):
        print(f"release tag must be a stable semantic version like v0.11.0: {tag}", file=sys.stderr)
        return 1

    root = Path(__file__).resolve().parents[2]
    versions = {
        "Cargo.toml": read_cargo_version(root / "Cargo.toml"),
        "studio/Cargo.toml": read_cargo_version(root / "studio" / "Cargo.toml"),
    }
    with (root / "editors" / "vscode" / "package.json").open(encoding="utf-8") as file:
        versions["editors/vscode/package.json"] = json.load(file)["version"]

    errors = []
    for path, value in versions.items():
        if not SEMVER.fullmatch(value):
            errors.append(f"{path} has an invalid version: {value}")
    for path in ("Cargo.toml", "studio/Cargo.toml"):
        if versions[path] != version:
            errors.append(f"{path} is {versions[path]}, but the release tag is {version}")

    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1

    print(f"release metadata valid: {tag}")
    print(f"CLI/Studio version: {version}")
    print(f"VS Code extension version: {versions['editors/vscode/package.json']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
