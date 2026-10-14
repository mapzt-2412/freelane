#!/usr/bin/env python3
"""Opt-in real stdio MCP/QEMU check. Uses only a fresh disposable UEFI VM."""
import base64
import json
import os
from pathlib import Path
import select
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]


class Client:
    def __init__(self, config, response_timeout=30):
        self.response_timeout = response_timeout
        self.process = subprocess.Popen(["bash", str(ROOT / "scripts/run-mcp.sh"), "--config", str(config)],
                                        stdin=subprocess.PIPE, stdout=subprocess.PIPE, cwd=ROOT)
        self.request_id = 0
        reply = self.request("initialize", {"protocolVersion": "2025-06-18", "capabilities": {},
                                             "clientInfo": {"name": "freelane-real-smoke", "version": "1"}})
        assert reply["protocolVersion"] == "2025-06-18"
        self.process.stdin.write(b'{"jsonrpc":"2.0","method":"notifications/initialized"}\n')
        self.process.stdin.flush()

    def request(self, method, params):
        self.request_id += 1
        message = {"jsonrpc": "2.0", "id": self.request_id, "method": method, "params": params}
        self.process.stdin.write((json.dumps(message) + "\n").encode())
        self.process.stdin.flush()
        assert select.select([self.process.stdout], [], [], self.response_timeout)[0], "MCP response deadline"
        reply = json.loads(self.process.stdout.readline())
        assert reply["id"] == self.request_id and "error" not in reply, reply
        return reply["result"]

    def tool(self, name, **arguments):
        return self.request("tools/call", {"name": name, "arguments": arguments})

    def data(self, name, **arguments):
        reply = self.tool(name, **arguments)
        assert not reply["isError"], reply
        return json.loads(reply["content"][0]["text"])

    def close(self):
        self.process.stdin.close()
        assert self.process.wait(timeout=10) == 0
        self.process.stdout.close()


def wait_status(client, expected):
    deadline = time.monotonic() + 25
    while time.monotonic() < deadline:
        result = client.tool("vm_control", vm="uefi", action="status")
        if not result["isError"]:
            text = json.loads(result["content"][0]["text"])["output"]
            if json.loads(text)["status"] == expected:
                return
        time.sleep(0.2)
    raise AssertionError("VM did not reach " + expected)


def main():
    cli = ROOT / "target/debug/freelane"
    firmware = Path(os.environ["FREELANE_TEST_FIRMWARE"]).resolve(strict=True)
    variables = Path(os.environ["FREELANE_TEST_NVRAM"]).resolve(strict=True)
    before = variables.read_bytes()
    with tempfile.TemporaryDirectory(prefix="freelane-mcp-uefi-") as directory:
        temp = Path(directory)
        vm = temp / "vm"
        subprocess.run([str(cli), "init", str(vm), str(firmware), str(variables)], check=True)
        config = vm / "vm.toml"
        text = config.read_text().replace('accelerator = "hvf"', 'accelerator = "tcg"')
        text = text.replace("headless = false", "headless = true").replace("cpus = 4", "cpus = 1")
        text = text.replace("memory_mib = 4096", "memory_mib = 2048")
        config.write_text(text)
        mcp_config = temp / "mcp.toml"
        mcp_config.write_text(f'repository={json.dumps(str(ROOT))}\nartifacts={json.dumps(str(ROOT / "target/mcp-real-smoke"))}\n[vms]\nuefi={json.dumps(str(config))}\n')
        client = Client(mcp_config)
        try:
            catalog = client.request("tools/list", {})
            assert len(catalog["tools"]) == 21
            assert client.data("host_info")["vms"] == ["uefi"]
            for task in ("test", "test_arm"):
                job = client.data("dev_run", task=task)
                deadline = time.monotonic() + 180
                while True:
                    result = client.data("dev_result", job_id=job["job_id"])
                    if result["status"] != "running":
                        assert result["status"] == "passed" and result["exit_code"] == 0, result
                        assert not result["timed_out"]
                        print("MCP development task passed:", task)
                        break
                    assert time.monotonic() < deadline, "Development job deadline"
                    time.sleep(0.2)
            client.data("vm_start", vm="uefi")
            wait_status(client, "running")
            duplicate = client.tool("vm_start", vm="uefi")
            assert duplicate["isError"], "Duplicate start accepted"
            client.data("vm_control", vm="uefi", action="pause")
            wait_status(client, "paused")
            client.data("vm_control", vm="uefi", action="resume")
            wait_status(client, "running")
            time.sleep(3)
            capture = client.tool("vm_capture", vm="uefi")
            assert not capture["isError"], capture
            image = next(item for item in capture["content"] if item["type"] == "image")
            assert base64.b64decode(image["data"]).startswith(b"\x89PNG\r\n\x1a\n")
            print("Actual MCP screenshot:", json.loads(capture["content"][0]["text"])["artifact"])
            client.data("vm_keys", vm="uefi", keys=["shift"])
            client.data("vm_click", vm="uefi", x=16384, y=16384)
            client.close()
            client = None
            subprocess.run([str(cli), "status", str(config)], check=True)
            client = Client(mcp_config)
            wait_status(client, "running")
            client.data("vm_control", vm="uefi", action="shutdown")
            print("Real stdio MCP checks passed: jobs, startup, duplicate rejection, pause/resume, PNG, key/click, disconnect/adoption, guest powerdown request.")
        finally:
            if client:
                client.close()
            # UEFI may ignore ACPI powerdown. Only this newly created fixture is force-stopped.
            subprocess.run([str(cli), "stop", str(config)], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            deadline = time.monotonic() + 10
            while time.monotonic() < deadline:
                status = subprocess.run([str(cli), "status", str(config)], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
                if status.returncode != 0:
                    break
                time.sleep(0.1)
            else:
                raise AssertionError("Disposable UEFI supervisor did not clean up")
            assert before == variables.read_bytes(), "Shared firmware variables changed"


if __name__ == "__main__":
    main()
