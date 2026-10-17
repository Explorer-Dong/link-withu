"""Validate the stable v-prefixed release against all four package versions."""

import json
import os
import re
import tomllib
from pathlib import Path


def validate_release(root, tag):
    if not re.fullmatch(r"v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)", tag):
        raise ValueError("A stable vMAJOR.MINOR.PATCH tag is required")
    version = tag[1:]
    frontend = json.loads((root / "web/package.json").read_text())
    lock = json.loads((root / "web/package-lock.json").read_text())
    api = tomllib.loads((root / "api/Cargo.toml").read_text())
    api_lock = tomllib.loads((root / "api/Cargo.lock").read_text())
    packages = [p for p in api_lock["package"] if p["name"] == "link-withu"]
    versions = {
        "Frontend": frontend["version"],
        "Frontend lock": lock["version"],
        "Frontend root lock": lock["packages"][""]["version"],
        "API": api["package"]["version"],
        "API lock": packages[0]["version"] if len(packages) == 1 else None,
    }
    for package, actual in versions.items():
        if actual != version:
            raise ValueError(f"{package} version mismatch: expected {version}")
    return version


def main():
    try:
        tag = os.environ["GITHUB_REF_NAME"]
        version = validate_release(Path.cwd(), tag)
        for name in ["DOCKERHUB_USERNAME", "DOCKERHUB_TOKEN"]:
            if not os.environ.get(name):
                raise ValueError(f"Configure {name} in Actions secrets")
        with open(os.environ["GITHUB_OUTPUT"], "a") as output:
            output.write(f"tag={tag}\nversion={version}\n")
    except (ValueError, KeyError, OSError) as error:
        raise SystemExit(str(error)) from None


if __name__ == "__main__":
    main()
