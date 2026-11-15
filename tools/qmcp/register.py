#!/usr/bin/env python3
"""Register the built qmcp with Codex, Claude Code and agy, preserving backups.
Run --check to validate all three configs and handshake with their commands.
"""
import datetime
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tomllib

ROOT = Path(__file__).resolve().parents[2]
BINARY = ROOT / "target/debug/qmcp"
HOME_DIR = Path.home()
CONFIGS = [HOME_DIR / ".codex/config.toml", HOME_DIR / ".claude.json",
           HOME_DIR / ".gemini/config/mcp_config.json"]


def check():
    for path in CONFIGS:
        if path.suffix == ".toml":
            entry = tomllib.loads(path.read_text())["mcp_servers"]["quake-tools"]
        else:
            entry = json.loads(path.read_text())["mcpServers"]["quake-tools"]
        assert entry["command"] == str(BINARY), f"Unexpected command in {path}"
        assert entry["args"] == ["--root", str(ROOT)], f"Unexpected args in {path}"
        requests = [
            {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "registration-check", "version": "1"}}},
            {"jsonrpc": "2.0", "method": "notifications/initialized"},
            {"jsonrpc": "2.0", "id": 2, "method": "tools/list"},
        ]
        result = subprocess.run([entry["command"], *entry["args"]], input="".join(json.dumps(r) + "\n" for r in requests), text=True, capture_output=True, check=True, timeout=15, cwd="/tmp")
        responses = [json.loads(line) for line in result.stdout.splitlines()]
        assert responses[0]["result"]["serverInfo"]["name"] == "quake-tools"
        assert len(responses[1]["result"]["tools"]) == 9
        print(f"OK: {path} (initialize + 9 tools, launched from /tmp)")


def main():
    if sys.argv[1:] == ["--check"]:
        check()
        return
    if sys.argv[1:]:
        sys.exit("usage: register.py [--check]")
    if not BINARY.is_file():
        sys.exit("Build first: cargo build -p qmcp")
    for command in ["codex", "claude", "agy"]:
        if not shutil.which(command):
            sys.exit(f"Missing harness: {command}")
    stamp = datetime.datetime.now().strftime("%Y%m%d-%H%M%S-%f")
    for path in CONFIGS:
        if path.exists():
            backup = path.with_name(path.name + ".before-qmcp-" + stamp)
            shutil.copy2(path, backup)
            backup.chmod(0o600)
    commands = [
        ["codex", "mcp", "add", "quake-tools", "--", str(BINARY), "--root", str(ROOT)],
        ["claude", "mcp", "add", "--scope", "user", "quake-tools", "--", str(BINARY), "--root", str(ROOT)],
        ["agy", "mcp", "add", "quake-tools", str(BINARY), "--root", str(ROOT)],
    ]
    # Claude's add rejects duplicates. Leave matching registrations untouched;
    # fail clearly on a conflict rather than silently replacing another server.
    for path, command in zip(CONFIGS, commands):
        if path.exists():
            doc = tomllib.loads(path.read_text()) if path.suffix == ".toml" else json.loads(path.read_text())
            entry = doc.get("mcp_servers" if path.suffix == ".toml" else "mcpServers", {}).get("quake-tools")
            if entry:
                if entry.get("command") == str(BINARY) and entry.get("args") == ["--root", str(ROOT)]:
                    print(f"Already registered: {path}")
                    continue
                sys.exit(f"Conflicting quake-tools registration in {path}; resolve it before retrying")
        subprocess.run(command, check=True, timeout=30)
    check()


if __name__ == "__main__":
    main()
