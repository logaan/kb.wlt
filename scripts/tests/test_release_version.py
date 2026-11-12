import importlib.util
from pathlib import Path
import tempfile
import unittest


SPEC = importlib.util.spec_from_file_location("release_version", Path(__file__).resolve().parents[1] / "release-version.py")
RELEASE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(RELEASE)


class ReleaseVersionTests(unittest.TestCase):
    def test_semantic_bumps_and_explicit_version(self):
        self.assertEqual(RELEASE.next_version("1.2.3", "patch"), "1.2.4")
        self.assertEqual(RELEASE.next_version("1.2.3", "minor"), "1.3.0")
        self.assertEqual(RELEASE.next_version("1.2.3", "major"), "2.0.0")
        self.assertEqual(RELEASE.next_version("1.2.3", "set", "2.1.0"), "2.1.0")

    def test_first_release_can_explicitly_use_the_current_version(self):
        self.assertEqual(RELEASE.next_version("0.1.0", "set", "0.1.0", allow_current=True), "0.1.0")
        with self.assertRaises(ValueError):
            RELEASE.next_version("0.1.0", "set", "0.0.9", allow_current=True)

    def test_invalid_and_non_increasing_versions_are_rejected(self):
        for value in ["", "v1.2.3", "01.2.3", "1.2", "1.2.3-beta", "1.2.3+build", "1.2.3\n", "-1.2.3", "1.2.$(id)"]:
            with self.assertRaises(ValueError):
                RELEASE.parse_version(value)
        for value in ["1.2.3", "1.2.2", "0.99.99"]:
            with self.assertRaises(ValueError):
                RELEASE.next_version("1.2.3", "set", value)
        with self.assertRaises(ValueError):
            RELEASE.next_version("1.2.3", "patch", "1.2.4")
        with self.assertRaises(ValueError):
            RELEASE.next_version("1.2.18446744073709551615", "patch")

    def test_only_the_requested_toml_section_changes(self):
        original = '[workspace.package]\nversion = "0.1.0"\nedition = "2024"\n\n[dependencies]\nversion = "9.9.9"\n'
        updated = RELEASE.replace_version(original, "workspace.package", "0.2.0")
        self.assertIn('version = "0.2.0"', updated)
        self.assertIn('[dependencies]\nversion = "9.9.9"', updated)
        self.assertIn('edition = "2024"', updated)

    def make_manifests(self, root, versions=None):
        versions = versions or ("0.1.0",) * len(RELEASE.MANIFESTS)
        for (filename, section), version in zip(RELEASE.MANIFESTS, versions):
            manifest = root / filename
            manifest.parent.mkdir(parents=True, exist_ok=True)
            manifest.write_text('[' + section + ']\nversion = "' + version + '"\n')

    def test_preview_does_not_write_and_update_keeps_versions_together(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.make_manifests(root)
            before = {filename: (root / filename).read_text() for filename, _ in RELEASE.MANIFESTS}
            self.assertEqual(RELEASE.update(root, "minor", check=True), "0.2.0")
            self.assertEqual(before, {filename: (root / filename).read_text() for filename, _ in RELEASE.MANIFESTS})
            self.assertEqual(RELEASE.update(root, "minor"), "0.2.0")
            for filename, section in RELEASE.MANIFESTS:
                self.assertEqual(RELEASE.manifest_version((root / filename).read_text(), section), "0.2.0")

    def test_mismatched_versions_fail_before_any_file_is_modified(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.make_manifests(root, ("0.1.0", "0.2.0", "0.1.0", "0.1.0"))
            before = {filename: (root / filename).read_text() for filename, _ in RELEASE.MANIFESTS}
            with self.assertRaises(ValueError):
                RELEASE.update(root, "major")
            self.assertEqual(before, {filename: (root / filename).read_text() for filename, _ in RELEASE.MANIFESTS})

    def test_changelog_rotation_keeps_previous_releases_and_extracts_notes(self):
        original = "# Changelog\n\n## [Unreleased]\n\n### Added\n\n- New firmware.\n\n## [0.1.0] - 2026-11-01\n\n- Earlier release.\n"
        updated, notes = RELEASE.rotate_changelog(original, "0.2.0", "2026-11-12")
        self.assertIn("## [Unreleased]\n\n## [0.2.0] - 2026-11-12", updated)
        self.assertIn("## [0.1.0] - 2026-11-01\n\n- Earlier release.", updated)
        self.assertEqual(notes, "### Added\n\n- New firmware.\n")

    def test_empty_duplicate_or_invalid_changelog_release_is_rejected(self):
        for text in ["# Changelog\n", "## [Unreleased]\n", "## [Unreleased]\n\n### Added\n"]:
            with self.assertRaises(ValueError):
                RELEASE.rotate_changelog(text, "0.2.0", "2026-11-12")
        with self.assertRaises(ValueError):
            RELEASE.rotate_changelog("## [Unreleased]\n\n- Changed.\n\n## [0.2.0]\n", "0.2.0", "2026-11-12")
        with self.assertRaises(ValueError):
            RELEASE.rotate_changelog("## [Unreleased]\n\n- Changed.\n", "0.2.0", "2026-13-01")

    def test_changelog_validation_happens_before_version_files_change(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.make_manifests(root)
            (root / "CHANGELOG.md").write_text("## [Unreleased]\n")
            with self.assertRaises(ValueError):
                RELEASE.update(root, "minor", released_on="2026-11-12")
            self.assertEqual(RELEASE.manifest_version((root / "Cargo.toml").read_text(), "workspace.package"), "0.1.0")

    def test_release_updates_manifests_changelog_and_notes_together(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.make_manifests(root)
            (root / "CHANGELOG.md").write_text("# Changelog\n\n## [Unreleased]\n\n- New feature.\n")
            notes = root / "release-notes.md"
            RELEASE.update(root, "patch", released_on="2026-11-12", notes_path=notes)
            self.assertEqual(notes.read_text(), "- New feature.\n")
            self.assertIn("## [0.1.1] - 2026-11-12", (root / "CHANGELOG.md").read_text())


if __name__ == "__main__":
    unittest.main()
