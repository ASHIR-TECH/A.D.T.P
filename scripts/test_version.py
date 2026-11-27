"""Dependency-free unit tests for scripts/version.py.

Run directly (no pytest needed):
    python3 scripts/test_version.py
    python3 -m unittest scripts.test_version -v     # if scripts is importable
"""

from __future__ import annotations

import contextlib
import io
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import version as vt  # noqa: E402  (local import after sys.path setup)


def fixture_files(v: str = "0.1.0") -> dict[str, str]:
    """A minimal repo tree containing every declaration the tool manages."""
    files: dict[str, str] = {vt.VERSION_FILE: f"{v}\n"}
    for path in vt.CARGO_MANIFESTS:
        files[path] = f'[package]\nname = "x"\nversion = "{v}"\n'
    files["Cargo.lock"] = "".join(
        f'[[package]]\nname = "adtp-{name}"\nversion = "{v}"\n'
        for name in ("core", "cli", "desktop")
    )
    files["desktop/src-tauri/tauri.conf.json"] = f'{{"version": "{v}"}}\n'
    files["desktop/package.json"] = f'{{"version": "{v}"}}\n'
    files["desktop/package-lock.json"] = (
        '{\n'
        f'  "version": "{v}",\n'
        '  "packages": {\n'
        '    "": {\n'
        '      "name": "adtp-desktop",\n'
        f'      "version": "{v}",\n'
        '      "dependencies": {\n'
        '        "@tauri-apps/api": "^2"\n'
        '      }\n'
        '    },\n'
        '    "node_modules/@tauri-apps/api": {\n'
        '      "version": "2.11.0"\n'
        '    }\n'
        '  }\n'
        '}\n'
    )
    files["packages/adtp-api/pyproject.toml"] = (
        f'[project]\nversion = "{v}"\n'
        f'dependencies = ["adtp-mcp>={v}"]\n'
    )
    files["packages/adtp-api/uv.lock"] = "".join(
        f'[[package]]\nname = "adtp-{name}"\nversion = "{v}"\n'
        for name in ("api", "mcp")
    )
    files["packages/adtp-mcp/pyproject.toml"] = f'[project]\nversion = "{v}"\n'
    files["packages/adtp-mcp/uv.lock"] = (
        f'[[package]]\nname = "adtp-mcp"\nversion = "{v}"\n'
    )
    files["packages/adtp-api/src/adtp_api/version.py"] = f'__version__ = "{v}"\n'
    files["packages/adtp-mcp/src/adtp_mcp/__init__.py"] = f'__version__ = "{v}"\n'
    files["crates/adtp-cli/src/art.rs"] = (
        f'pub const VERSION: &str = "v{v}";\n'
    )
    return files


class ParseTests(unittest.TestCase):
    def test_valid_versions(self) -> None:
        self.assertEqual(str(vt.Version.parse("1.2.3")), "1.2.3")
        self.assertEqual(vt.Version.parse("0.0.0"), vt.Version(0, 0, 0))
        self.assertEqual(vt.Version.parse(" 10.20.30 \n"), vt.Version(10, 20, 30))

    def test_invalid_versions(self) -> None:
        for raw in (
            "",
            "1.2",
            "1.2.3.4",
            "v1.2.3",
            "1.2.3-rc1",
            "01.2.3",
            "1.02.3",
            "1.2.03",
            "-1.2.3",
            "1.2.x",
        ):
            with self.subTest(raw=raw), self.assertRaises(vt.VersionError):
                vt.Version.parse(raw)


class BumpTests(unittest.TestCase):
    def test_patch(self) -> None:
        self.assertEqual(str(vt.Version(1, 2, 3).bump("patch")), "1.2.4")

    def test_minor_resets_patch(self) -> None:
        self.assertEqual(str(vt.Version(1, 2, 3).bump("minor")), "1.3.0")

    def test_major_resets_minor_and_patch(self) -> None:
        self.assertEqual(str(vt.Version(1, 2, 3).bump("major")), "2.0.0")

    def test_unknown_part(self) -> None:
        with self.assertRaises(vt.VersionError):
            vt.Version(1, 2, 3).bump("build")


class SyncTests(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        for rel, text in fixture_files().items():
            path = self.root / rel
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text, encoding="utf-8")

    def tearDown(self) -> None:
        self.tmp.cleanup()

    def test_every_rule_matches_the_fixture(self) -> None:
        for rule in vt.build_rules():
            text = (self.root / rule.path).read_text(encoding="utf-8")
            self.assertTrue(list(rule.matches(text)), rule.path)

    def test_plan_and_write_keep_every_mirror_in_sync(self) -> None:
        target = vt.Version.parse("2.3.4")
        changes = vt.plan_changes(self.root, target)
        expected_files = len({rule.path for rule in vt.build_rules()}) + 1  # + VERSION
        self.assertEqual(len(changes), expected_files)
        for change in changes:
            change.path.write_text(change.text, encoding="utf-8")
        version, problems = vt.check(self.root)
        self.assertEqual(str(version), "2.3.4")
        self.assertEqual(problems, [])
        self.assertEqual(
            (self.root / "crates/adtp-cli/src/art.rs").read_text(),
            'pub const VERSION: &str = "v2.3.4";\n',
        )
        self.assertIn(
            '"adtp-mcp>=2.3.4"',
            (self.root / "packages/adtp-api/pyproject.toml").read_text(),
        )

    def test_npm_dependency_versions_are_never_rewritten(self) -> None:
        for change in vt.plan_changes(self.root, vt.Version.parse("3.0.0")):
            change.path.write_text(change.text, encoding="utf-8")
        lock = (self.root / "desktop/package-lock.json").read_text(encoding="utf-8")
        self.assertIn('"version": "3.0.0"', lock)
        self.assertIn('"version": "2.11.0"', lock)  # third-party dependency
        self.assertEqual(lock.count('"version": "2.11.0"'), 1)

    def test_check_reports_drift(self) -> None:
        health_path = self.root / "desktop/package.json"
        health_path.write_text('{"version": "9.9.9"}\n', encoding="utf-8")
        version, problems = vt.check(self.root)
        self.assertEqual(str(version), "0.1.0")
        self.assertTrue(any("desktop/package.json" in p for p in problems))
        self.assertTrue(any("9.9.9" in p for p in problems))

    def test_check_reports_missing_file(self) -> None:
        (self.root / "desktop/src-tauri/tauri.conf.json").unlink()
        _, problems = vt.check(self.root)
        self.assertTrue(any("tauri.conf.json: file is missing" in p for p in problems))

    def test_bump_dry_run_writes_nothing(self) -> None:
        before = {
            rel: (self.root / rel).read_text(encoding="utf-8")
            for rel in fixture_files()
        }
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            code = vt.main(["--root", str(self.root), "--dry-run", "bump", "minor"])
        self.assertEqual(code, 0)
        self.assertIn("0.1.0 -> 0.2.0", out.getvalue())
        self.assertIn("nothing written", out.getvalue())
        after = {rel: (self.root / rel).read_text(encoding="utf-8") for rel in before}
        self.assertEqual(before, after)

    def test_set_rejects_bad_version_without_writing(self) -> None:
        before = (self.root / vt.VERSION_FILE).read_text(encoding="utf-8")
        err = io.StringIO()
        with contextlib.redirect_stderr(err):
            code = vt.main(["--root", str(self.root), "set", "1.2.3-rc1"])
        self.assertEqual(code, 1)
        self.assertIn("invalid version", err.getvalue())
        self.assertEqual((self.root / vt.VERSION_FILE).read_text(), before)

    def test_show_and_check_commands(self) -> None:
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            code = vt.main(["--root", str(self.root), "show"])
        self.assertEqual(code, 0)
        self.assertEqual(out.getvalue().strip(), "0.1.0")
        with contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(vt.main(["--root", str(self.root), "check"]), 0)

    def test_plan_is_pure(self) -> None:
        before = {
            rel: (self.root / rel).read_text(encoding="utf-8")
            for rel in fixture_files()
        }
        vt.plan_changes(self.root, vt.Version(9, 9, 9))
        after = {rel: (self.root / rel).read_text(encoding="utf-8") for rel in before}
        self.assertEqual(before, after)

    def test_commit_flag_rejects_dry_run(self) -> None:
        err = io.StringIO()
        with contextlib.redirect_stderr(err):
            code = vt.main([
                "--root", str(self.root), "bump", "patch", "--dry-run", "--commit"
            ])
        self.assertEqual(code, 1)
        self.assertIn("cannot be used together", err.getvalue())

    def test_version_paths_cover_every_mirror(self) -> None:
        paths = {p.relative_to(self.root).as_posix() for p in vt.version_paths(self.root)}
        self.assertIn(vt.VERSION_FILE, paths)
        for rule in vt.build_rules():
            self.assertIn(rule.path, paths)


if __name__ == "__main__":
    unittest.main(verbosity=2)
