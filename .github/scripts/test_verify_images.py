"""Reject publication of missing, swapped or untested platform images."""

import copy
import tempfile
import unittest
from pathlib import Path

from verify_images import read_digests, verify_manifests


class ImageVerification(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.digests = {"amd64": "sha256:" + "a" * 64, "arm64": "sha256:" + "b" * 64}
        for arch, digest in self.digests.items():
            (self.root / f"{arch}.txt").write_text(digest + "\n")
        self.manifest = {
            "schemaVersion": 2,
            "manifests": [
                {"digest": digest, "platform": {"os": "linux", "architecture": arch}}
                for arch, digest in self.digests.items()
            ],
        }

    def test_accepts_only_both_tested_platforms_and_matching_latest(self):
        self.assertEqual(read_digests(self.root), self.digests)
        verify_manifests(self.manifest, copy.deepcopy(self.manifest), self.digests)

    def test_missing_digest_fails_before_publication(self):
        (self.root / "arm64.txt").unlink()
        with self.assertRaises(FileNotFoundError):
            read_digests(self.root)

    def test_invalid_digest_and_reference_injection_rejected(self):
        for digest in ["", "sha256:abc", "sha256:" + "A" * 64, "latest", "x\n--tag=other"]:
            (self.root / "arm64.txt").write_text(digest)
            with self.subTest(digest=digest), self.assertRaises(ValueError):
                read_digests(self.root)

    def test_same_digest_cannot_represent_both_platforms(self):
        (self.root / "arm64.txt").write_text(self.digests["amd64"])
        with self.assertRaises(ValueError):
            read_digests(self.root)

    def test_latest_cannot_point_to_another_release(self):
        latest = copy.deepcopy(self.manifest)
        latest["manifests"][0]["digest"] = "sha256:" + "c" * 64
        with self.assertRaisesRegex(ValueError, "same manifest"):
            verify_manifests(self.manifest, latest, self.digests)

    def test_missing_or_extra_platform_rejected(self):
        for descriptors in [[], self.manifest["manifests"][:1], self.manifest["manifests"] * 2]:
            manifest = {"manifests": descriptors}
            with self.subTest(count=len(descriptors)), self.assertRaises(ValueError):
                verify_manifests(manifest, manifest, self.digests)

    def test_wrong_os_unknown_and_duplicate_architectures_rejected(self):
        for platform in [
            {"os": "windows", "architecture": "arm64"},
            {"os": "linux", "architecture": "386"},
            {"os": "linux", "architecture": "amd64"},
            {},
        ]:
            manifest = copy.deepcopy(self.manifest)
            manifest["manifests"][1]["platform"] = platform
            with self.subTest(platform=platform), self.assertRaises(ValueError):
                verify_manifests(manifest, manifest, self.digests)

    def test_swapped_or_rebuilt_images_rejected(self):
        for digest in [self.digests["arm64"], "sha256:" + "c" * 64]:
            manifest = copy.deepcopy(self.manifest)
            manifest["manifests"][0]["digest"] = digest
            with self.subTest(digest=digest), self.assertRaisesRegex(ValueError, "tested image"):
                verify_manifests(manifest, manifest, self.digests)


if __name__ == "__main__":
    unittest.main()
