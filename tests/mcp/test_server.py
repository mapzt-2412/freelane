"""Protocol and subprocess checks; fake CLI cases do not prove guest operation."""
import importlib.util
import json
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import patch
import zlib

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("freelane_mcp", ROOT / "scripts/freelane_mcp.py")
mcp = importlib.util.module_from_spec(spec)
spec.loader.exec_module(mcp)


class ServerTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        (self.root / "Cargo.toml").write_text('[package]\nname="fixture"\nversion="0.1.0"\n')
        (self.root / "vm.toml").write_text('# protocol fixture only\n')
        self.config = self.root / "mcp.toml"
        self.config.write_text('repository="."\njob_timeout_seconds=1\n[vms]\ntest="vm.toml"\n')
        self.server = mcp.Server(self.config)

    def tearDown(self):
        self.server.close()
        self.temp.cleanup()

    def request(self, method, params=None, request_id=1):
        return self.server.dispatch({"jsonrpc": "2.0", "id": request_id, "method": method, "params": params or {}})

    def cli_fixture(self):
        directory = self.root / "target/debug"
        directory.mkdir(parents=True)
        script = directory / "freelane"
        script.write_text(f'#!{sys.executable}\nimport json,sys\nprint(json.dumps(sys.argv[1:]))\nsys.exit(7 if sys.argv[1]=="agent-ping" else 0)\n')
        script.chmod(0o700)

    def await_job(self, job_id):
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            result = self.server.call("dev_result", {"job_id": job_id})
            if result["status"] != "running":
                return result
            time.sleep(0.01)
        self.fail("Job did not finish")

    def test_stdio_initialize_notifications_and_tool_catalog(self):
        messages = [
            {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2024-11-05"}},
            {"jsonrpc": "2.0", "method": "notifications/initialized"},
            {"jsonrpc": "2.0", "id": "list", "method": "tools/list"},
            {"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "host_info", "arguments": {}}},
        ]
        process = subprocess.run([sys.executable, str(ROOT / "scripts/freelane_mcp.py"), "--config", str(self.config)],
                                 input="".join(json.dumps(message) + "\n" for message in messages),
                                 text=True, capture_output=True, timeout=10, check=True)
        replies = [json.loads(line) for line in process.stdout.splitlines()]
        self.assertEqual([reply["id"] for reply in replies], [1, "list", 3])
        self.assertEqual(replies[0]["result"]["protocolVersion"], "2024-11-05")
        self.assertEqual(len(replies[1]["result"]["tools"]), len(mcp.TOOLS))
        self.assertFalse(replies[2]["result"]["isError"])
        self.assertEqual(process.stderr, "")

    def test_protocol_errors_and_notifications_do_not_execute_tools(self):
        self.assertEqual(self.request("tools/list")["error"]["code"], -32000)
        self.assertIsNone(self.server.dispatch({"jsonrpc": "2.0", "method": "tools/call", "params": {"name": "dev_run", "arguments": {"task": "build"}}}))
        self.assertFalse(self.server.jobs)
        self.assertEqual(self.server.dispatch([])["error"]["code"], -32600)
        self.assertEqual(self.request("initialize", request_id=True)["error"]["code"], -32600)
        self.request("initialize")
        self.assertEqual(self.request("unknown")["error"]["code"], -32601)

    def test_bad_and_oversized_stdio_messages_do_not_desynchronize_next_request(self):
        healthy = json.dumps({"jsonrpc": "2.0", "id": 8, "method": "ping"}).encode() + b"\n"
        process = subprocess.run([sys.executable, str(ROOT / "scripts/freelane_mcp.py"), "--config", str(self.config)],
                                 input=b"{broken\n" + b"x" * (mcp.MAX_MESSAGE + 10) + b"\n" + healthy,
                                 capture_output=True, timeout=10, check=True)
        replies = [json.loads(line) for line in process.stdout.splitlines()]
        self.assertEqual([reply["error"]["code"] for reply in replies[:2]], [-32700, -32700])
        self.assertEqual(replies[2], {"jsonrpc": "2.0", "id": 8, "result": {}})

    def test_nonzero_cli_exit_is_a_tool_error_and_arguments_are_literal(self):
        self.cli_fixture()
        self.request("initialize")
        result = self.request("tools/call", {"name": "guest_ping", "arguments": {"vm": "test"}})["result"]
        self.assertTrue(result["isError"])
        self.assertEqual(json.loads(result["content"][0]["text"])["exit_code"], 7)
        args = ["file with spaces & $(touch forbidden).txt", 'a"b']
        launch = self.server.call("guest_launch", {"vm": "test", "executable": "C:\\Windows\\notepad.exe", "arguments": args})
        self.assertEqual(json.loads(launch["output"])[3:], args)
        self.assertFalse((self.root / "forbidden").exists())

    def test_unregistered_paths_unknown_arguments_and_nul_are_rejected(self):
        self.cli_fixture()
        for name, args in [
            ("vm_control", {"vm": "../vm.toml", "action": "status"}),
            ("dev_run", {"task": "test; echo injected"}),
            ("dev_run", {"task": "test", "command": "rm"}),
            ("vm_click", {"vm": "test", "x": True, "y": 1}),
            ("vm_click", {"vm": "test", "x": 32768, "y": 1}),
            ("guest_launch", {"vm": "test", "executable": "app.exe"}),
            ("guest_launch", {"vm": "test", "executable": "C:\\app.exe", "arguments": ["a\0b"]}),
        ]:
            with self.subTest(name=name, args=args), self.assertRaises(ValueError):
                self.server.call(name, args)

    def test_ubuntu_launch_uses_registered_profile_and_literal_linux_path(self):
        self.cli_fixture()
        (self.root / "vm.toml").write_text('guest_os="ubuntu"\n')
        result = self.server.call("guest_launch", {"vm": "test", "executable": "/usr/bin/gedit", "arguments": ["a & b.txt"]})
        self.assertEqual(json.loads(result["output"])[2:], ["/usr/bin/gedit", "a & b.txt"])
        with self.assertRaises(ValueError):
            self.server.call("guest_launch", {"vm": "test", "executable": "C:\\app.exe"})
        with self.assertRaises(ValueError):
            self.server.call("guest_launch", {"vm": "test", "executable": "gedit"})

    def test_job_keeps_failure_exit_and_bounded_output(self):
        command = [sys.executable, "-c", "import sys; print('x'*40000); print('FINAL_MARKER'); sys.exit(23)"]
        with patch.dict(mcp.TASKS, {"test": command}):
            job = self.server.call("dev_run", {"task": "test"})
            result = self.await_job(job["job_id"])
        self.assertEqual(result["status"], "failed")
        self.assertEqual(result["exit_code"], 23)
        self.assertFalse(result["timed_out"])
        self.assertTrue(result["output_truncated"])
        self.assertLessEqual(len(result["output"]), mcp.MAX_OUTPUT)
        self.assertIn("FINAL_MARKER", result["output"])

    def test_timeout_does_not_become_a_pass_and_parallel_jobs_are_rejected(self):
        with patch.dict(mcp.TASKS, {"test": [sys.executable, "-c", "import time; print('START',flush=True); time.sleep(30)"]}):
            job = self.server.call("dev_run", {"task": "test"})
            with self.assertRaises(ValueError):
                self.server.call("dev_run", {"task": "build"})
            result = self.await_job(job["job_id"])
        self.assertEqual(result["status"], "timed_out")
        self.assertTrue(result["timed_out"])
        self.assertNotEqual(result["exit_code"], 0)
        self.assertIn("START", result["output"])

    def test_eof_cancels_owned_build_without_stopping_a_vm(self):
        with patch.dict(mcp.TASKS, {"test": [sys.executable, "-c", "import time; time.sleep(30)"]}):
            job = self.server.call("dev_run", {"task": "test"})
            with patch.object(mcp, "stop_process", wraps=mcp.stop_process) as stop:
                self.server.close()
                self.assertEqual(stop.call_count, 1)
            result = self.await_job(job["job_id"])
        self.assertNotEqual(result["exit_code"], 0)

    def test_png_preserves_pixel_whitespace_and_rejects_bad_raster(self):
        pixels = b"\n #\r\t\0"
        png = mcp.ppm_png(b"P6\n# actual pixels\n2 1\n255\n" + pixels)
        self.assertTrue(png.startswith(b"\x89PNG\r\n\x1a\n"))
        pos, payload = 8, b""
        while pos < len(png):
            length = struct.unpack(">I", png[pos:pos + 4])[0]
            if png[pos + 4:pos + 8] == b"IDAT":
                payload += png[pos + 8:pos + 8 + length]
            pos += 12 + length
        self.assertEqual(zlib.decompress(payload), b"\0" + pixels)
        for bad in (b"P6\n1 1\n255\nxx", b"P6\n8193 1\n255\n", b"P3\n1 1\n255\nxxx"):
            with self.assertRaises(ValueError):
                mcp.ppm_png(bad)


if __name__ == "__main__":
    unittest.main()
