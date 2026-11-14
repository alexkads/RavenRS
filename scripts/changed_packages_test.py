#!/usr/bin/env python3
"""Tests of how scripts/changed-packages.py reads Cargo.toml and Cargo.lock: what it skips is what
the quick check does not run, so a mistake here hides a broken package. scripts/check.sh runs it."""
import importlib.util
import pathlib
import unittest

spec = importlib.util.spec_from_file_location(
    "changed_packages", pathlib.Path(__file__).with_name("changed-packages.py")
)
cp = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cp)

MANIFEST = """[workspace]
resolver = "3"
members = ["crates/*", "examples/*"]

[workspace.package]
version = "1.0.0"
edition = "2024"

[workspace.dependencies]
ravenrs = { version = "1.0.0", path = "crates/ravenrs" }
serde = { version = "1", features = ["derive"] }
wgpu = "27"
ignored = { version = "1", features = [
    "a",
    "b",
] }

[workspace.lints.clippy]
all = "warn"

[profile.dev]
debug = "line-tables-only"
"""

LOCK = """version = 4

[[package]]
name = "app"
version = "1.0.0"
dependencies = [
 "ravenrs",
 "serde",
]

[[package]]
name = "ravenrs"
version = "1.0.0"
dependencies = [
 "wgpu",
]

[[package]]
name = "serde"
version = "1.0.1"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "aaa"

[[package]]
name = "naga"
version = "27.0.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "bbb"

[[package]]
name = "wgpu"
version = "27.0.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "ccc"
dependencies = [
 "naga",
]
"""
MEMBERS = {"app", "ravenrs", "newcrate"}


class Manifest(unittest.TestCase):
    def test_nothing_changed(self):
        self.assertEqual(cp.manifest_changes(MANIFEST, MANIFEST), (set(), False, False))

    def test_version_bump_is_not_everything(self):
        new = MANIFEST.replace('version = "1.0.0"', 'version = "1.0.1"')
        self.assertEqual(cp.manifest_changes(MANIFEST, new), (set(), True, False))

    def test_new_member_is_not_everything(self):
        new = MANIFEST.replace('members = ["crates/*", "examples/*"]', 'members = ["crates/*", "examples/*", "x"]')
        self.assertEqual(cp.manifest_changes(MANIFEST, new), (set(), False, False))

    def test_changed_dependency_reaches_its_users(self):
        new = MANIFEST.replace('serde = { version = "1", features = ["derive"] }', 'serde = { version = "1", features = ["derive", "rc"] }')
        self.assertEqual(cp.manifest_changes(MANIFEST, new)[0], {"serde"})

    def test_new_dependency_and_multiline_value(self):
        new = MANIFEST.replace('    "b",\n', '    "b",\n    "c",\n').replace('wgpu = "27"', 'wgpu = "27"\nnewdep = "2"')
        self.assertEqual(cp.manifest_changes(MANIFEST, new)[0], {"ignored", "newdep"})

    def test_internal_path_change_reaches_users(self):
        new = MANIFEST.replace('path = "crates/ravenrs"', 'path = "crates/ravenrs2"')
        self.assertEqual(cp.manifest_changes(MANIFEST, new)[0], {"ravenrs"})

    def test_profile_lints_and_edition_are_everything(self):
        for old, new in [
            ('debug = "line-tables-only"', 'debug = true'),
            ('all = "warn"', 'all = "deny"'),
            ('edition = "2024"', 'edition = "2021"'),
            ('resolver = "3"', 'resolver = "2"'),
        ]:
            with self.subTest(new), self.assertRaises(cp.Everything):
                cp.manifest_changes(MANIFEST, MANIFEST.replace(old, new))

    def test_new_section_is_everything(self):
        with self.assertRaises(cp.Everything):
            cp.manifest_changes(MANIFEST, MANIFEST + '\n[patch.crates-io]\nwgpu = { path = "../wgpu" }\n')

    def test_lint_configuration(self):
        new = MANIFEST + '\n[workspace.metadata.ravenrs-lint]\nrules = ["x"]\n'
        self.assertEqual(cp.manifest_changes(MANIFEST, new), (set(), False, True))


class Lock(unittest.TestCase):
    def test_nothing_changed(self):
        self.assertEqual(cp.lock_changes(LOCK, LOCK, MEMBERS), set())

    def test_deep_external_change_reaches_members_through_externals_only(self):
        new = LOCK.replace('version = "27.0.0"\nsource = "registry+https://github.com/rust-lang/crates.io-index"\nchecksum = "bbb"',
                           'version = "27.0.1"\nsource = "registry+https://github.com/rust-lang/crates.io-index"\nchecksum = "bbc"')
        # naga -> wgpu -> ravenrs; app comes in later as a dependent of ravenrs.
        self.assertEqual(cp.lock_changes(LOCK, new, MEMBERS), {"ravenrs"})

    def test_direct_external_change(self):
        new = LOCK.replace('version = "1.0.1"', 'version = "1.0.2"').replace('"aaa"', '"aab"')
        self.assertEqual(cp.lock_changes(LOCK, new, MEMBERS), {"app"})

    def test_member_version_bump_is_nothing(self):
        new = LOCK.replace('name = "app"\nversion = "1.0.0"', 'name = "app"\nversion = "1.0.1"')
        self.assertEqual(cp.lock_changes(LOCK, new, MEMBERS), set())

    def test_new_member(self):
        new = LOCK + '\n[[package]]\nname = "newcrate"\nversion = "1.0.0"\ndependencies = [\n "serde",\n]\n'
        self.assertEqual(cp.lock_changes(LOCK, new, MEMBERS), {"newcrate"})

    def test_member_with_a_new_dependency(self):
        new = LOCK.replace(' "ravenrs",\n "serde",\n', ' "ravenrs",\n "serde",\n "wgpu",\n')
        self.assertEqual(cp.lock_changes(LOCK, new, MEMBERS), {"app"})


if __name__ == "__main__":
    unittest.main()
