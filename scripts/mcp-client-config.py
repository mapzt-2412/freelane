#!/usr/bin/env python3
"""Print a client-specific stdio configuration; never edit the user's settings."""
import argparse
import json
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--client", choices=("codex", "cursor", "claude"), required=True)
parser.add_argument("--config", required=True)
args = parser.parse_args()
root = Path(__file__).resolve().parents[1]
config = Path(args.config).resolve(strict=True)
command_args = [str(root / "scripts/run-mcp.sh"), "--config", str(config)]
if args.client == "codex":
    print('[mcp_servers.freelane]\ncommand = "bash"\nargs = ' + json.dumps(command_args))
else:
    print(json.dumps({"mcpServers": {"freelane": {"command": "bash", "args": command_args}}}, indent=2))
