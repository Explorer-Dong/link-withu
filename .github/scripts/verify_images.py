"""Publish and verify only the immutable image digests recorded after smoke tests."""

import argparse
import json
import re
from pathlib import Path

ARCHITECTURES = ("amd64", "arm64")


def read_digests(directory):
    digests = {}
    for arch in ARCHITECTURES:
        digest = (directory / f"{arch}.txt").read_text().strip()
        if not re.fullmatch(r"sha256:[0-9a-f]{64}", digest):
            raise ValueError(f"Invalid tested digest for {arch}")
        digests[arch] = digest
    if len(set(digests.values())) != len(ARCHITECTURES):
        raise ValueError("Each architecture must have a distinct tested digest")
    return digests


def verify_manifests(version, latest, digests):
    if version != latest:
        raise ValueError("Version and latest must reference the same manifest")
    manifests = version.get("manifests", [])
    if len(manifests) != len(ARCHITECTURES):
        raise ValueError("Expected exactly two tested platform manifests")
    seen = set()
    for descriptor in manifests:
        platform = descriptor.get("platform", {})
        arch = platform.get("architecture")
        if platform.get("os") != "linux" or arch not in ARCHITECTURES:
            raise ValueError("Unexpected image platform")
        if arch in seen:
            raise ValueError(f"Duplicate image platform: {arch}")
        if descriptor.get("digest") != digests[arch]:
            raise ValueError(f"Published {arch} image differs from the tested image")
        seen.add(arch)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    references = commands.add_parser("references")
    references.add_argument("directory", type=Path)
    references.add_argument("--image", required=True)
    verify = commands.add_parser("verify")
    verify.add_argument("directory", type=Path)
    verify.add_argument("--version-manifest", type=Path, required=True)
    verify.add_argument("--latest-manifest", type=Path, required=True)
    args = parser.parse_args()
    try:
        digests = read_digests(args.directory)
        if args.command == "references":
            for arch in ARCHITECTURES:
                print(f"{args.image}@{digests[arch]}")
        else:
            verify_manifests(
                json.loads(args.version_manifest.read_text()),
                json.loads(args.latest_manifest.read_text()),
                digests,
            )
            print("Both platforms match the tested digests; version and latest agree.")
    except (ValueError, KeyError, OSError) as error:
        parser.exit(1, f"{error}\n")


if __name__ == "__main__":
    main()
