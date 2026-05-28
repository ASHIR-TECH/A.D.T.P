Day 1:
- i did alll the push and commits to github
- removed ci.yml, release.yml and security.yml from .github/workflows
to avoid actions issues 

steps: first i had to install rust :
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
rustc --version

then uv python Package manager :
curl -LsSf https://astral.sh/uv/install.sh | sh
source "$HOME/.cargo/env"
uv --version

then node: 
curl -fsSL https://deb.nodesource.com/setup_20.x | sudo -E bash -
sudo apt install -y nodejs
node --version

then tauri CLI:
cargo install tauri-cli

to test cargo and crates for phase 1:
- cargo test --package adtp-core 
then 
- cargo check --workspace && cargo clippy --package adtp-core -- -D warnings

Built Phase 0 backend core as scoped from the doc.

Implemented

Workspace deps and release profile in Cargo.toml:1
Core crate manifest in crates/adtp-core/Cargo.toml:1
Protocol foundation modules: types.rs:1, errors.rs:1, protocol.rs:1, integrity.rs:1, encryption.rs:1, session.rs:1, transfer.rs:1
Public exports in crates/adtp-core/src/lib.rs:1
Minimal crate scaffolds so the documented workspace loads cleanly.
Verified

cargo test --package adtp-core — 14 passed, doctest passed
cargo check --workspace — passed
cargo clippy --package adtp-core -- -D warnings — passed
cargo audit — not available in this toolchain (cargo-audit not installed)