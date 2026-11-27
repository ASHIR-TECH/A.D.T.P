#!/usr/bin/env python3
"""ADTP version tool: one source of truth, zero dependencies.

`VERSION` at the repository root is authoritative. Every other version
declaration that ships with the product is a derived mirror, listed in
``build_rules()``. This script validates, displays, and atomically rewrites
those mirrors so they cannot drift.

Usage (from anywhere, with any Python 3.9+):

    python3 scripts/version.py show
    python3 scripts/version.py check
    python3 scripts/version.py bump patch --dry-run
    python3 scripts/version.py bump minor
    python3 scripts/version.py bump minor --commit   # one commit, no manual adds
    python3 scripts/version.py set 1.2.3 --dry-run
    python3 scripts/version.py set 1.2.3 --commit
    python3 scripts/version.py commit                # commit an already-made bump

See docs/VERSIONING.md for the policy behind the numbers.
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Iterator

VERSION_FILE = "VERSION"
SEMVER_RE = re.compile(r"^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$")
PARTS = ("major", "minor", "patch")

# Every Rust package in the workspace, including the Tauri desktop shell.
CARGO_MANIFESTS = (
    "crates/adtp-core/Cargo.toml",
    "crates/adtp-client/Cargo.toml",
    "crates/adtp-server/Cargo.toml",
    "crates/adtp-cli/Cargo.toml",
    "crates/adtp-proto/Cargo.toml",
    "crates/adtp-peer-core/Cargo.toml",
    "crates/adtp-peer/Cargo.toml",
    "crates/adtp-rendezvous/Cargo.toml",
    "desktop/src-tauri/Cargo.toml",
)

# Regexes use three named groups: `pre` (unchanged prefix), `v` (the version
# text itself) and `post` (unchanged suffix). Everything outside `v` is kept
# byte-for-byte, so indentation, quoting and `v` prefixes in display strings
# survive a rewrite.
CARGO_VERSION = r'^(?P<pre>version = ")(?P<v>[^"\n]+)(?P<post>")'
JSON_VERSION = r'(?P<pre>"version": ")(?P<v>[^"\n]+)(?P<post>")'
PYTHON_VERSION = r'(?P<pre>__version__ = ")(?P<v>[^"\n]+)(?P<post>")'
SIBLING_PIN = r'(?P<pre>"adtp-mcp>=)(?P<v>[^"\n]+)(?P<post>")'
CLI_BANNER = r'(?P<pre>pub const VERSION: &str = "v)(?P<v>[^"\n]+)(?P<post>")'
# Lockfiles: scope the match to ADTP package blocks only. Third-party crate
# and PyPI dependency versions must never be touched.
CARGO_LOCK_VERSION = (
    r'(?m)^(?P<pre>\[\[package\]\]\nname = "adtp-[^"\n]+"\nversion = ")'
    r'(?P<v>[^"\n]+)(?P<post>")'
)
UV_LOCK_VERSION = (
    r'(?m)^(?P<pre>\[\[package\]\]\nname = "{name}"\nversion = ")'
    r'(?P<v>[^"\n]+)(?P<post>")'
)


class VersionError(Exception):
    """Invalid version input or an out-of-sync repository."""


@dataclass(frozen=True, order=True)
class Version:
    major: int
    minor: int
    patch: int

    def __str__(self) -> str:
        return f"{self.major}.{self.minor}.{self.patch}"

    @classmethod
    def parse(cls, text: str) -> "Version":
        """Parse a strict MAJOR.MINOR.PATCH string (no `v`, no prerelease)."""
        raw = text.strip()
        match = SEMVER_RE.fullmatch(raw)
        if match is None:
            raise VersionError(
                f"invalid version {text!r}: expected MAJOR.MINOR.PATCH "
                "with non-negative integers and no leading zeros (e.g. 1.2.3)"
            )
        return cls(*(int(group) for group in match.groups()))

    def bump(self, part: str) -> "Version":
        """Return the next version, zeroing all lower-priority components."""
        if part == "major":
            return Version(self.major + 1, 0, 0)
        if part == "minor":
            return Version(self.major, self.minor + 1, 0)
        if part == "patch":
            return Version(self.major, self.minor, self.patch + 1)
        raise VersionError(f"unknown bump part {part!r}: expected one of {', '.join(PARTS)}")


@dataclass(frozen=True)
class Rule:
    """One mirror of the product version, located by a strict regex."""

    path: str
    pattern: re.Pattern[str]

    def matches(self, text: str) -> Iterator[re.Match[str]]:
        return self.pattern.finditer(text)

    def rewrite(self, text: str, version: Version) -> tuple[str, list[str]]:
        found = list(self.matches(text))
        if not found:
            raise VersionError(f"{self.path}: no version declaration matched")
        replacement = f"\\g<pre>{version}\\g<post>"
        return self.pattern.sub(replacement, text), [m.group("v") for m in found]


@dataclass
class Change:
    """A planned write. Nothing is written until every plan step succeeds."""

    path: Path
    text: str
    found: list[str] = field(default_factory=list)


def _rule(path: str, pattern: str) -> Rule:
    return Rule(path, re.compile(pattern, re.MULTILINE))


def build_rules() -> list[Rule]:
    """Return every derived version declaration that must track VERSION."""
    rules = [_rule(path, CARGO_VERSION) for path in CARGO_MANIFESTS]
    rules.extend(
        [
            _rule("Cargo.lock", CARGO_LOCK_VERSION),
            _rule("desktop/src-tauri/tauri.conf.json", JSON_VERSION),
            _rule("desktop/package.json", JSON_VERSION),
            # The npm lockfile has many `"version"` keys. Scope to the root
            # entry and to the `packages[""]` block, never to dependencies.
            _rule("desktop/package-lock.json", r'(?m)^(?P<pre>  "version": ")(?P<v>[^"\n]+)(?P<post>")'),
            _rule(
                "desktop/package-lock.json",
                r'(?m)^(?P<pre>    "": \{\n      "name": "adtp-desktop",\n      "version": ")(?P<v>[^"\n]+)(?P<post>")',
            ),
            _rule("packages/adtp-api/pyproject.toml", CARGO_VERSION),
            _rule("packages/adtp-api/pyproject.toml", SIBLING_PIN),
            _rule(
                "packages/adtp-api/uv.lock",
                UV_LOCK_VERSION.format(name="adtp-(?:api|mcp)"),
            ),
            _rule("packages/adtp-mcp/pyproject.toml", CARGO_VERSION),
            _rule("packages/adtp-mcp/uv.lock", UV_LOCK_VERSION.format(name="adtp-mcp")),
            _rule("packages/adtp-api/src/adtp_api/version.py", PYTHON_VERSION),
            _rule("packages/adtp-mcp/src/adtp_mcp/__init__.py", PYTHON_VERSION),
            _rule("crates/adtp-cli/src/art.rs", CLI_BANNER),
        ]
    )
    return rules


def read_product_version(root: Path) -> Version:
    path = root / VERSION_FILE
    if not path.is_file():
        raise VersionError(f"missing {VERSION_FILE} at {root}")
    return Version.parse(path.read_text(encoding="utf-8"))


def check(root: Path) -> tuple[Version, list[str]]:
    """Validate VERSION and report every mirror that disagrees with it."""
    version = read_product_version(root)
    expected = str(version)
    problems: list[str] = []
    for rule in build_rules():
        path = root / rule.path
        if not path.is_file():
            problems.append(f"{rule.path}: file is missing")
            continue
        matches = list(rule.matches(path.read_text(encoding="utf-8")))
        if not matches:
            problems.append(f"{rule.path}: no version declaration matched")
        for match in matches:
            if match.group("v") != expected:
                problems.append(
                    f"{rule.path}: found {match.group('v')!r}, expected {expected!r}"
                )
    return version, problems


def plan_changes(root: Path, version: Version) -> list[Change]:
    """Compute all rewrites in memory; raise before writing if anything fails.

    Rules are applied in order to a per-file working copy: two rules may target
    the same file (e.g. `adtp-api/pyproject.toml`), so a later rule must not
    overwrite the edits made by an earlier one.
    """
    texts: dict[Path, str] = {}
    found_by_path: dict[Path, list[str]] = {}
    for rule in build_rules():
        path = root / rule.path
        if path not in texts:
            if not path.is_file():
                raise VersionError(f"{rule.path}: file is missing")
            texts[path] = path.read_text(encoding="utf-8")
        new_text, found = rule.rewrite(texts[path], version)
        texts[path] = new_text
        found_by_path.setdefault(path, []).extend(found)
    version_path = root / VERSION_FILE
    old = version_path.read_text(encoding="utf-8").strip()
    texts[version_path] = f"{version}\n"
    found_by_path[version_path] = [old]
    return [
        Change(path=path, text=text, found=found_by_path[path])
        for path, text in texts.items()
    ]


def _print_plan(changes: list[Change], target: Version, root: Path) -> None:
    for change in changes:
        rel = change.path.relative_to(root)
        summary = ", ".join(dict.fromkeys(f"{old!r} -> {target}" for old in change.found))
        print(f"  {rel}: {summary} ({len(change.found)} declaration(s))")


def _git(root: Path, *args: str, check: bool = True) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(
        ["git", *args],
        cwd=root,
        text=True,
        capture_output=True,
    )
    if check and result.returncode != 0:
        detail = (result.stderr or result.stdout or "").strip()
        raise VersionError(f"git {' '.join(args)} failed: {detail}")
    return result


def version_paths(root: Path) -> list[Path]:
    """Every file that may carry the product version, including VERSION."""
    paths = [root / VERSION_FILE]
    paths.extend(root / rule.path for rule in build_rules())
    # A path can be covered by two rules (pyproject.toml); de-duplicate.
    return list(dict.fromkeys(paths))


def commit_version_changes(root: Path, target: Version, paths: list[Path]) -> bool:
    """Stage and commit only version files. Returns False if nothing changed."""
    _git(root, "rev-parse", "--is-inside-work-tree")
    changed: list[str] = []
    for path in paths:
        rel = path.relative_to(root).as_posix()
        status = _git(root, "status", "--porcelain", "--", rel).stdout.strip()
        if status:
            changed.append(rel)
    if not changed:
        print("No version changes to commit.")
        return False
    _git(root, "add", "--", *changed)
    _git(root, "commit", "-m", f"chore(release): bump version to {target}")
    print(f"Committed {len(changed)} version file(s) as one commit: {target}")
    return True


def repo_root() -> Path:
    return Path(__file__).resolve().parent.parent


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "--root",
        default=str(repo_root()),
        help="repository root (default: parent of this script's directory)",
    )
    parser.add_argument(
        "--dry-run",
        "-n",
        action="store_true",
        help="show what would change without writing any files",
    )
    commands = parser.add_subparsers(dest="command", required=True)

    def dry_run_flag(subparser: argparse.ArgumentParser) -> None:
        # Accept the flag before or after the subcommand. SUPPRESS keeps the
        # value already parsed at the top level when the flag is not repeated.
        subparser.add_argument(
            "--dry-run",
            "-n",
            action="store_true",
            default=argparse.SUPPRESS,
            help="show what would change without writing any files",
        )

    def commit_flag(subparser: argparse.ArgumentParser) -> None:
        subparser.add_argument(
            "--commit",
            action="store_true",
            default=argparse.SUPPRESS,
            help="stage and commit every version file as one commit",
        )

    show_cmd = commands.add_parser("show", help="print the product version")
    dry_run_flag(show_cmd)
    check_cmd = commands.add_parser("check", help="validate VERSION and every mirror")
    dry_run_flag(check_cmd)
    bump = commands.add_parser("bump", help="increment major, minor or patch")
    bump.add_argument("part", choices=PARTS)
    dry_run_flag(bump)
    commit_flag(bump)
    set_cmd = commands.add_parser("set", help="pin an exact version")
    set_cmd.add_argument("version", help="MAJOR.MINOR.PATCH, e.g. 1.2.3")
    dry_run_flag(set_cmd)
    commit_flag(set_cmd)
    commit_cmd = commands.add_parser(
        "commit", help="commit pending version changes as one commit"
    )
    args = parser.parse_args(argv)

    root = Path(args.root).resolve()
    try:
        if args.command == "show":
            print(read_product_version(root))
            return 0

        if args.command == "check":
            version, problems = check(root)
            if problems:
                print(f"VERSION = {version} — OUT OF SYNC", file=sys.stderr)
                for problem in problems:
                    print(f"  {problem}", file=sys.stderr)
                return 1
            print(f"VERSION = {version} — all mirrors in sync")
            return 0

        if args.command == "commit":
            target = read_product_version(root)
            commit_version_changes(root, target, version_paths(root))
            return 0

        current = read_product_version(root)
        if args.command == "bump":
            target = current.bump(args.part)
        else:
            target = Version.parse(args.version)
        if args.dry_run and getattr(args, "commit", False):
            raise VersionError("--dry-run and --commit cannot be used together")
        changes = plan_changes(root, target)
        print(f"{current} -> {target}{' (dry run, nothing written)' if args.dry_run else ''}")
        _print_plan(changes, target, root)
        if args.dry_run:
            return 0
        for change in changes:
            change.path.write_text(change.text, encoding="utf-8")
        print(f"Updated {len(changes)} files to {target}.")
        if getattr(args, "commit", False):
            commit_version_changes(root, target, [change.path for change in changes])
        return 0
    except (VersionError, OSError) as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
