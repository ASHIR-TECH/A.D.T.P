#!/usr/bin/env bash
# Install the ADTP gateway as a per-user systemd service.
#
# This is optional: the desktop app also starts the gateway on launch and
# detaches it so it survives closing the window. A systemd unit additionally
# starts the listener at login (before the desktop app), which is what makes
# the phone connect immediately after the PC boots.
#
# Usage:
#   ./scripts/install-listener.sh
#   PYTHON=python3.12 ./scripts/install-listener.sh
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
data_home="${XDG_DATA_HOME:-$HOME/.local/share}"
config_home="${XDG_CONFIG_HOME:-$HOME/.config}"
venv="$data_home/adtp/venv"
env_file="$config_home/adtp/api.env"
unit_dir="$config_home/systemd/user"
unit_file="$unit_dir/adtp-api.service"

python_bin="${PYTHON:-python3.13}"
if ! command -v "$python_bin" >/dev/null 2>&1; then
  python_bin="python3"
fi

echo "==> Repository: $repo_root"
echo "==> Python:     $python_bin"

mkdir -p "$data_home/adtp" "$config_home/adtp" "$unit_dir"

if [ ! -x "$venv/bin/python" ]; then
  echo "==> Creating virtualenv at $venv"
  "$python_bin" -m venv "$venv"
fi

echo "==> Installing adtp-api and adtp-mcp (editable)"
"$venv/bin/python" -m pip install --upgrade pip >/dev/null
# Install the sibling package first so pip does not look for adtp-mcp on PyPI.
"$venv/bin/python" -m pip install -e "$repo_root/packages/adtp-mcp" -e "$repo_root/packages/adtp-api"

if [ ! -f "$env_file" ] || ! grep -q '^ADTP_API_TOKEN=' "$env_file"; then
  echo "==> Generating pairing token in $env_file"
  token="$("$venv/bin/python" -c 'import secrets; print(secrets.token_hex(32))')"
  umask 077
  {
    echo "ADTP_API_HOST=0.0.0.0"
    echo "ADTP_API_PORT=40003"
    echo "ADTP_API_TOKEN=$token"
  } > "$env_file"
fi
chmod 600 "$env_file"

echo "==> Installing systemd user unit"
sed \
  -e "s|%h/.local/share/adtp/venv|$venv|g" \
  -e "s|%h/.config/adtp/api.env|$env_file|g" \
  "$repo_root/packaging/adtp-api.service" > "$unit_file"

systemctl --user daemon-reload
systemctl --user enable --now adtp-api.service

if command -v loginctl >/dev/null 2>&1; then
  # Without lingering, the unit stops when you log out; with it, the listener
  # stays up 24/7. This may require polkit approval.
  loginctl enable-linger "$USER" 2>/dev/null \
    || echo "!! Could not enable linger. Run: sudo loginctl enable-linger $USER"
fi

port="$(sed -n 's/^ADTP_API_PORT=//p' "$env_file" | head -1)"
port="${port:-40003}"

echo
echo "Listener installed."
echo "  Status:  systemctl --user status adtp-api"
echo "  Logs:    journalctl --user -u adtp-api -f"
echo "  Health:  curl http://127.0.0.1:$port/api/v1/health"
echo
echo "If the phone still cannot connect, allow TCP $port on the LAN:"
echo "  sudo ufw allow $port/tcp"
