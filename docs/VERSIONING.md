# ADTP versioning

## Summary

ADTP uses a single **MAJOR.MINOR.PATCH** (SemVer-style) number, e.g. `1.2.3`.

- **`VERSION`** at the repository root is the **only** source of truth. It
  contains one bare line, e.g. `1.2.3` (no `v` prefix, no `-rc`, no build
  metadata).
- Everything else that carries a version is a **derived mirror**, rewritten
  atomically by `scripts/version.py`. `python3 scripts/version.py check` fails
  if any mirror drifts.
- The script uses only the Python standard library. It never installs anything
  and never runs a build.

Current product version: `1.0.0` (matches the existing `Mars v1.0.0` /
`adtp-desktop_1.0.0_amd64.deb` release identity). To adopt a different
baseline, run `python3 scripts/version.py set 1.2.3` once.

## The three layers

| Layer | Value | Lives in | Changed by |
| --- | --- | --- | --- |
| **Product version** — the user-facing ADTP Desktop / MARS release, used in release names and installer filenames | `1.0.0` | `VERSION` | `version.py bump\|set` |
| **Code version** — Rust workspace crates, Python packages, Tauri bundle metadata, npm shell package, CLI banner, runtime API version strings | same number as the product (**lockstep**) | `crates/*/Cargo.toml`, `Cargo.lock`, `desktop/src-tauri/{Cargo.toml,tauri.conf.json}`, `desktop/{package.json,package-lock.json}`, `packages/*/pyproject.toml`, `packages/*/uv.lock`, `adtp_api/version.py`, `adtp_mcp/__init__.py`, `crates/adtp-cli/src/art.rs` | `version.py bump\|set` |
| **Wire / protocol version** — binary packet format and HTTP API revision | `ADTP_VERSION: u8 = 1`, `/api/v1`, schema versions | `crates/adtp-core/src/types.rs`, REST routes, MCP media schemas | **manually**, never by `version.py` |

### How the layers relate

The code layers all ship inside one product: the desktop binary embeds the
Rust daemons and launches the Python gateway as a sidecar, and the MCP bridge
is imported by that gateway. There is no supported combination of, say,
desktop `1.0.0` with gateway `0.1.0`, so independent version numbers only
created drift (before this scheme the same checkout said `0.1.0`, `0.2.0`,
`v0.1.3` and `1.0.0`). Therefore **code version = product version**.

The wire version is deliberately different: it counts *format* revisions, not
releases. Many product releases can share `ADTP_VERSION = 1` and `/api/v1`.
Only a format-breaking release advances the wire version (e.g. to `ADTP_VERSION
= 2` or `/api/v2`), and such a release **must** be a product MAJOR bump so users
can tell that old peers will not interoperate.

Rule of thumb: if a change requires updating a peer's protocol constant, an
API path, or a persisted config schema, it is MAJOR; if it only adds to them in
a backwards-compatible way, it is at most MINOR.

## Bump rules

Starting from `1.2.3`:

| Bump | Result | Use when | ADTP examples |
| --- | --- | --- | --- |
| `patch` | `1.2.4` | Backwards-compatible fixes; no API, wire, config or behaviour contract changes | Fix TOFU fingerprint mismatch handling; close a path-traversal edge case in `/transfers/upload`; correct CLI banner text; fix a relay heartbeat timer |
| `minor` | `1.3.0` | New backwards-compatible functionality | Add `GET /api/v1/system/info` or `POST /api/v1/transfers/upload`; add an MCP tool/resource; add an optional field to config or Agent Card; add a new (additive) transfer type |
| `major` | `2.0.0` | Incompatible change for users, peers or integrators | Change `PacketHeader` layout and bump `ADTP_VERSION` 1→2; remove/rename an `/api/v1` endpoint without a compatibility shim; change QR pairing payload or config file schema in a way old builds cannot read; require a new mandatory auth handshake |

`major` resets minor and patch to zero; `minor` resets patch to zero. Numbers
never go backwards. No prerelease suffixes are used — ship prereleases from a
branch with a distinct tag if ever needed, and document that exception here.

## Commands

Run from the repository root:

```bash
python3 scripts/version.py show                     # 1.0.0
python3 scripts/version.py check                    # validate every mirror

python3 scripts/version.py bump patch --dry-run     # preview, writes nothing
python3 scripts/version.py bump patch               # 1.0.0 -> 1.0.1
python3 scripts/version.py bump minor --dry-run
python3 scripts/version.py bump minor               # 1.0.1 -> 1.1.0
python3 scripts/version.py bump major --dry-run
python3 scripts/version.py bump major               # 1.1.0 -> 2.0.0
python3 scripts/version.py set 1.2.3 --dry-run      # pin an exact baseline

# One-command release bump: writes every mirror AND commits them together
python3 scripts/version.py bump patch --commit       # 1.0.0 -> 1.0.1, one commit
python3 scripts/version.py bump minor --commit       # 1.0.1 -> 1.1.0, one commit
python3 scripts/version.py set 1.2.3 --commit        # pin + commit

# If you already bumped without --commit, commit all mirrors in one shot:
python3 scripts/version.py commit
```

Unit tests (stdlib `unittest`, no installs):

```bash
python3 scripts/test_version.py
python3 -m pytest scripts/test_version.py -q        # optional; pytest just collects them
```

`--dry-run` prints each file and the old→new values but makes no writes.
`check` exits non-zero with a per-file list on drift. `--commit` stages **only**
version files (the 21 mirrors listed above) and creates a single commit — you
never add or commit them one by one. It refuses to run together with
`--dry-run`. The `commit` subcommand does the same for a bump you already made
by hand.

## Release procedure

1. Decide the bump using the table above (a wire change means MAJOR).
2. Preview: `python3 scripts/version.py bump <part> --dry-run`.
3. Apply and commit in one step: `python3 scripts/version.py bump <part> --commit`.
   (If you already applied it: `python3 scripts/version.py commit`.)
4. Verify: `python3 scripts/version.py check && python3 scripts/version.py show`.
5. Move the `Unreleased` section of `CHANGELOG.md` into a `## <version>` heading
   with the bump date; add the new empty `Unreleased` section.
6. Push. CI creates the `adtp-v<version>-<sha>` tag and the GitHub release; you
   do not need to create tags manually.

### CI handoff (not changed here)

`.github/workflows/ci.yml` is owned by another worker and was deliberately not
edited. It currently hardcodes `1.0.0` in the `.deb` `Version:` field, the
`.deb` filename and artifact path, and in the release notes/release name, and
it derives the git tag from the commit SHA (`adtp-v1.0.0-<sha>`). Until CI reads
`VERSION` (recommended: one step that writes `version=$(cat VERSION)` to
`$GITHUB_OUTPUT` and references `${{ steps.version.outputs.version }}` in those
places), **verify the tag and artifact names by hand after every release**.

## Deliberately not synchronised

- `.github/workflows/ci.yml` — separate owner; see handoff above.
- `CHANGELOG.md` — release notes are written by the releaser, not the script.
- `ADTP_VERSION` (`u8`), REST `/api/v1` paths, media/MCP schema versions —
  wire-level identifiers with their own rules; changed manually and only with
  a MAJOR product bump.
- `docs/spec/adtp-protocol-spec.md` and `docs/spec/a2a-transport-binding.md`
  headings (`ADTP v0.1.0`) — protocol document revisions, not product releases.
- `docs/HANDS_ON_ADTP_TEST.md` (`app 1.0.0`, captured `health` response with
  `version: 0.1.0`) and `phases/*` phase reports — historical records, not
  live declarations.
- Third-party dependency constraints and lockfile entries (e.g. `tokio 1.36`,
  `pydantic>=2.7`) — only the internal `adtp-mcp>=X.Y.Z` sibling pin is
  rewritten, because it pins a component that ships in lockstep.
- `Cargo.toml` at the root — a virtual workspace manifest with no `[package]`
  version field; nothing to rewrite there.
