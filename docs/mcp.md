# Vibe coding and VM tests through MCP

FreeLane's Python 3.11+ standard-library server exposes development tasks and registered VM controls over MCP stdio. The same server configuration can be used by Codex, Cursor and Claude clients that support local stdio MCP servers. There is no additional package installation, network listener or remote provisioning service. Run one server per client session on the machine that owns the repository and VMs.

The first verified environment is the existing Linux cloud checkout. Mac Apple Silicon is the later native test environment; the wrapper and protocol are portable to macOS, but this MCP workflow has not run on a real Mac. Linux TCG results are correctness tests, not HVF or graphics performance results. Connecting a local client to the cloud requires a supported terminal/SSH connection to that machine; copying a `/workspace` path into a client on a different machine will not connect it.

## Prepare the server

From the repository, build the CLI and run the protocol tests:

```sh
cargo build --locked --workspace
python3 -m unittest discover -s tests/mcp -v
```

Use `configs/mcp.cloud.example.toml` in the retained cloud environment. It registers the existing Windows VM at `/workspace/windows-secure-preview/vm.toml` as `windows`, without starting it. On another Linux machine, copy the file to `configs/mcp.cloud.local.toml` and replace that path with an existing VM you own. The config file is trusted local configuration; tool calls select registered names rather than arbitrary filesystem paths. Local config files are ignored by Git.

```sh
bash scripts/run-mcp.sh --config configs/mcp.cloud.example.toml
```

This command waits for MCP JSON requests on stdin. It is not an interactive terminal prompt. Stdout is reserved for protocol replies; development output is captured in job results. The wrapper activates the retained cloud Rust/QEMU tools when present. On other machines, provide Python 3.11+, the pinned Rust toolchain, QEMU/swtpm and the firmware environment variables described in [development.md](development.md).

The server accepts protocol versions 2024-11-05, 2025-03-26 and 2025-06-18. It implements initialize, notifications, ping, tools/list and tools/call. It does not implement HTTP/SSE transport, prompts or resources. Newer clients must accept the negotiated supported version.

## Connect Codex, Cursor and Claude

Generate configuration with actual absolute paths on the server's machine:

```sh
python3 scripts/mcp-client-config.py --client codex --config configs/mcp.cloud.example.toml
python3 scripts/mcp-client-config.py --client cursor --config configs/mcp.cloud.example.toml
python3 scripts/mcp-client-config.py --client claude --config configs/mcp.cloud.example.toml
```

The generator prints configuration and never overwrites existing settings. Merge it with existing MCP servers in the client. Restart/reconnect that client as required, then require an actual `host_info` reply and the tool catalog before calling the connection successful. This repository cannot automatically install its server into an already running chat or change a user's local client settings from the cloud.

For Codex, the generated block belongs in the client's MCP TOML configuration. In this cloud checkout it is:

```toml
[mcp_servers.freelane]
command = "bash"
args = ["/workspace/freelane/scripts/run-mcp.sh", "--config", "/workspace/freelane/configs/mcp.cloud.example.toml"]
```

For Cursor, merge the generated JSON into the project's `.cursor/mcp.json` or the client's user MCP settings. For Claude Code, use its project MCP configuration; for Claude Desktop, merge into its MCP server settings. The common JSON block is:

```json
{
  "mcpServers": {
    "freelane": {
      "command": "bash",
      "args": [
        "/workspace/freelane/scripts/run-mcp.sh",
        "--config",
        "/workspace/freelane/configs/mcp.cloud.example.toml"
      ]
    }
  }
}
```

These examples require the client process to run on that Linux machine. For a remote editor, run the MCP process in its remote workspace if supported. If using an existing SSH connection, configure the client to start the same wrapper through SSH on the server; keep key/host verification, use an already configured SSH host alias, and ensure the remote shell emits no welcome text on stdout. No SSH endpoint or access was provisioned in this experiment. Never paste a private key or password into MCP configuration or chat.

## Tool workflow

| Tool | Purpose |
| --- | --- |
| `mobile_control`, `mobile_install`, `mobile_launch`, `mobile_capture` | Registered Android AVD/Xcode Simulator operations; [mobile setup and limitations](mobile.md) |
| `host_info` | Host architecture, tool paths, repository and registered VM names |
| `dev_run` | Start one fixed task: build, test, test_arm, lint, format_check, check_mac, build_agent or build_linux_agent |
| `dev_result` | Poll a session-local job ID; running/passed/failed/timed_out, actual exit code and bounded output tail |
| `vm_start` | Request one independent foreground CLI supervisor for a registered VM; returns starting, not readiness |
| `vm_control` | Status, pause, resume or request clean guest shutdown |
| `vm_capture` | Actual PNG display as an MCP image plus the retained artifact path |
| `vm_keys` | One physical QEMU key chord; layout-dependent, not arbitrary text |
| `vm_click` | Primary mouse click using normalized coordinates 0-32767 |
| `guest_ping` | Check an already running interactive Windows or Ubuntu guest agent |
| `guest_launch` | One absolute Windows .exe or Ubuntu executable path selected by registered guest_os, plus literal arguments; inspect its actual behavior afterward |

For a coding loop, ask the client to call `host_info`, run `dev_run` with `task="test"`, then poll `dev_result` until completion. Inspect failures, edit code through the client's normal editor tools, and rerun the relevant fixed task. MCP deliberately has no generic shell or file-edit tool; the coding client's existing repository tools do that work.

For VM tests, call `vm_control` with `action="status"`; start only an intended stopped VM. Poll status after `vm_start`, capture the screen, send a deliberate input, and capture again to observe its effect. A successful input request is weaker evidence than the actual guest behavior. Windows agent tools require `runtime.guest_agent=true`, the compatible signed ARM64 driver and an agent started in the interactive Windows session; see [guest-agent.md](guest-agent.md). A process PID is not proof that the app works. Never automatically retry a launch whose response timed out.

There is one development job at a time, a configurable 1-3600 second deadline (default 600), and a 16 KiB output tail with an explicit truncation flag. A nonzero exit or timeout is a tool error. Job history retains at most 32 entries and does not survive a server restart. Requests are capped at 1 MiB; captures have 64 MiB raster and 8 MiB compressed-image limits. Images remain in the configured artifact directory; clean up only your own captures when no longer needed. Full supervisor startup logs are retained there too.

Closing the client/server cancels its active development job but leaves VM supervisors running, matching the desktop manager's behavior. Reconnect and use status to adopt the same registered VM. Request Windows shutdown, verify actual supervisor/QEMU/TPM exit, and save disk/NVRAM/TPM together. No hard-stop tool is exposed. Do not publish a cloud snapshot with an active guest; live processes, RAM, job IDs and sockets do not restore.

## Real VM acceptance

The opt-in test requires the existing QEMU/firmware tools and uses a new disposable UEFI VM, never the retained Windows guest:

```sh
export FREELANE_TEST_FIRMWARE=/path/to/AAVMF_CODE.fd
export FREELANE_TEST_NVRAM=/path/to/AAVMF_VARS.fd
python3 tests/mcp_real_smoke.py
```

The latest local Linux run completed a real stdio handshake/catalog of 14 tools, ran 54 portable checks and six real ARM/QEMU tests as MCP jobs, then verified new VM startup, duplicate-start rejection, pause/resume, actual PNG output, key/click requests and client disconnect/reconnection while the guest stayed running. It also sent a guest powerdown request. UEFI may ignore that request, so the test explicitly stops only its own disposable fixture and checks runtime cleanup and preservation of the shared firmware variable template. It does not claim that a shutdown acknowledgment proves Windows exited.

Eleven protocol/subprocess tests passed separately, including failed exit codes, output truncation, total timeouts, literal arguments, rejected unknown VM names, notifications producing no responses, recovery after malformed/oversized requests and lossless conversion of actual PPM bytes. Fake-CLI cases are protocol tests, not Windows evidence. The official Node MCP SDK 1.32.1 also completed real stdio negotiation, listed the then-current ten tools (before the four mobile tools were added), read host information, observed an expected tool error and ran/polled an actual format-check job. It was installed outside the repository with registry integrity verification as an optional check; the server has no Node runtime dependency. Actual installed Windows operations through this MCP server and actual Codex/Cursor/Claude UI connections remain to be verified. CI is configured to repeat the Python/VM checks; remote CI results are separate from local results.

## Installed Ubuntu acceptance

Installed Ubuntu Server ARM64 was subsequently tested through actual MCP guest ping/launch: normal UID 1000, literal path/UTF-8 payload, write/read/copy/ZIP checks, native reboot with a changed boot ID and unchanged saved bytes/CRC. MCP pause/status/resume and a fresh ping after resume also passed. See [Ubuntu evidence](ubuntu-test-evidence.md). This is separate from the UEFI fixtures above and does not provision a client's UI or demonstrate graphical app behavior.

## Add a Mac later

Clone the repository on an Apple Silicon Mac, install the documented toolchain/QEMU/swtpm/firmware prerequisites and create a new VM with `freelane init`. Copy `configs/mcp.mac.example.toml` to `configs/mcp.mac.local.toml`, enter that Mac's existing VM path, and generate client configuration on the Mac. Choose HVF explicitly in the VM config; do not silently fall back to TCG.

Connect locally or through an authorized existing remote connection. First verify `host_info`, build/test jobs, native signing/probe and an actual guest boot. Measure real app behavior and performance there. Linux results and a Mac cross-target compiler check do not replace this hardware validation.
