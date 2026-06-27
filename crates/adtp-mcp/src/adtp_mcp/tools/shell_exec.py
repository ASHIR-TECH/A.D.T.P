import subprocess
import shlex

BLOCKED_COMMANDS = [
    "rm", "sudo", "chmod", "chown", "dd", "mkfs", "fdisk",
    "mount", "umount", "passwd", "useradd", "usermod", "groupadd",
    "kill", "pkill", "reboot", "shutdown", "init", "systemctl",
]


def shell_exec(command: str, allowed_commands: list = None, timeout: int = 30):
    if allowed_commands is None:
        allowed_commands = []

    parts = shlex.split(command)
    if not parts:
        return {"error": "Empty command", "stdout": "", "stderr": ""}

    cmd_name = parts[0].lower()

    if cmd_name in BLOCKED_COMMANDS:
        return {"error": f"Command '{cmd_name}' is blocked for security reasons", "stdout": "", "stderr": ""}

    if cmd_name not in [c.lower() for c in allowed_commands]:
        return {"error": f"Command '{cmd_name}' is not in allowed_commands", "stdout": "", "stderr": ""}

    try:
        result = subprocess.run(
            parts,
            capture_output=True,
            text=True,
            timeout=timeout,
        )
        return {
            "stdout": result.stdout,
            "stderr": result.stderr,
            "returncode": result.returncode,
        }
    except subprocess.TimeoutExpired:
        return {"error": f"Command timed out after {timeout}s", "stdout": "", "stderr": ""}
    except FileNotFoundError:
        return {"error": f"Command not found: {parts[0]}", "stdout": "", "stderr": ""}
    except Exception as e:
        return {"error": str(e), "stdout": "", "stderr": ""}
