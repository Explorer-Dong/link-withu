"""Release validation never publishes or reads real credentials."""

import json
import os
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from check_release import main, validate_release


class ReleaseValidation(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        (self.root / "api").mkdir()
        (self.root / "web").mkdir()
        (self.root / "api/Cargo.toml").write_text('[package]\nversion="1.0.1"\n')
        (self.root / "api/Cargo.lock").write_text(
            '[[package]]\nname="link-withu"\nversion="1.0.1"\n'
        )
        (self.root / "web/package.json").write_text('{"version":"1.0.1"}')
        self.write_lock("1.0.1", "1.0.1")

    def write_lock(self, outer, inner):
        (self.root / "web/package-lock.json").write_text(
            json.dumps({"version": outer, "packages": {"": {"version": inner}}})
        )

    def test_matching_stable_version(self):
        self.assertEqual(validate_release(self.root, "v1.0.1"), "1.0.1")

    def test_prefix_prerelease_leading_zero_and_injection_rejected(self):
        for tag in [
            "1.0.1",
            "v1.0",
            "v01.0.1",
            "v1.0.1-rc1",
            "v1.0.1\nversion=bad",
            "v١.0.1",
        ]:
            with self.subTest(tag=tag), self.assertRaises(ValueError):
                validate_release(self.root, tag)

    def test_frontend_manifest_mismatch(self):
        (self.root / "web/package.json").write_text('{"version":"1.0.0"}')
        with self.assertRaisesRegex(ValueError, "Frontend version mismatch"):
            validate_release(self.root, "v1.0.1")

    def test_both_frontend_lock_versions_are_checked(self):
        for outer, inner in [("1.0.0", "1.0.1"), ("1.0.1", "1.0.0")]:
            self.write_lock(outer, inner)
            with self.assertRaises(ValueError):
                validate_release(self.root, "v1.0.1")

    def test_api_manifest_mismatch(self):
        (self.root / "api/Cargo.toml").write_text('[package]\nversion="1.0.0"\n')
        with self.assertRaisesRegex(ValueError, "API version mismatch"):
            validate_release(self.root, "v1.0.1")

    def test_api_lock_missing_duplicate_or_mismatched(self):
        for content in [
            '[[package]]\nname="other"\nversion="1.0.1"\n',
            '[[package]]\nname="link-withu"\nversion="1.0.0"\n',
            '[[package]]\nname="link-withu"\nversion="1.0.1"\n' * 2,
        ]:
            (self.root / "api/Cargo.lock").write_text(content)
            with self.assertRaisesRegex(ValueError, "API lock version mismatch"):
                validate_release(self.root, "v1.0.1")

    def test_cli_requires_credentials_and_writes_exact_outputs(self):
        previous = Path.cwd()
        self.addCleanup(os.chdir, previous)
        os.chdir(self.root)
        output = self.root / "output"
        env = {"GITHUB_REF_NAME": "v1.0.1", "GITHUB_OUTPUT": str(output)}
        with patch.dict(os.environ, env, clear=True), self.assertRaises(SystemExit):
            main()
        self.assertFalse(output.exists())
        env.update(DOCKERHUB_USERNAME="test-only", DOCKERHUB_TOKEN="test-only")
        with patch.dict(os.environ, env, clear=True):
            main()
        self.assertEqual(output.read_text(), "tag=v1.0.1\nversion=1.0.1\n")


if __name__ == "__main__":
    unittest.main()
