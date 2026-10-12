#!/usr/bin/env python3
"""FreeLane stdio MCP server. Python 3.11+, standard library, Linux/macOS."""
import argparse
import base64
import json
import ntpath
import os
from pathlib import Path
import platform
import selectors
import shutil
import signal
import struct
import subprocess
import sys
import threading
import time
import tomllib
import uuid
import zlib

MAX_MESSAGE = 1024 * 1024
MAX_OUTPUT = 16 * 1024
VERSIONS = ("2025-06-18", "2025-03-26", "2024-11-05")
TASKS = {
    "build": ["cargo", "build", "--locked", "--workspace"],
    "test": ["cargo", "test", "--locked", "--workspace"],
    "test_arm": ["cargo", "test", "--locked", "--test", "qemu_smoke", "--", "--ignored"],
    "lint": ["cargo", "clippy", "--locked", "--workspace", "--all-targets", "--", "-D", "warnings"],
    "format_check": ["cargo", "fmt", "--all", "--check"],
    "check_mac": ["cargo", "check", "--locked", "--workspace", "--exclude", "freelane-guest-agent", "--target", "aarch64-apple-darwin"],
    "build_agent": ["bash", "scripts/build-windows-agent.sh"],
    "build_linux_agent": ["bash", "scripts/build-linux-agent.sh"],
}


def schema(properties=None, required=None):
    return {"type": "object", "properties": properties or {},
            "required": required or [], "additionalProperties": False}


VM = {"type": "string", "minLength": 1, "maxLength": 64}
TOOLS = [
    ("mobile_control", "Control a registered Android AVD or Xcode iOS Simulator. Start is not boot readiness; iOS requires a Mac.",
     schema({"device": VM, "action": {"type":"string","enum":["start","status","stop","devices","window"]}}, ["device","action"])),
    ("mobile_capture", "Capture a genuine PNG from a registered emulator/simulator, not a live stream.", schema({"device":VM},["device"])),
    ("mobile_install", "Install an APK or Simulator-target .app into the registered device. No App Store IPA support; no automatic retry.",
     schema({"device":VM,"application":{"type":"string","minLength":1,"maxLength":4096}},["device","application"])),
    ("mobile_launch", "Launch a literal Android package/activity or iOS bundle ID on the registered device. Inspect actual app behavior separately.",
     schema({"device":VM,"application":{"type":"string","minLength":1,"maxLength":256}},["device","application"])),
    ("host_info", "Report host/tool paths and registered VM names; no credential values.", schema()),
    ("dev_run", "Start one fixed development task. Returns job ID, not a pass result. Poll dev_result.",
     schema({"task": {"type": "string", "enum": list(TASKS)}}, ["task"])),
    ("dev_result", "Read bounded output, actual exit code and timeout state of a development job.",
     schema({"job_id": {"type": "string", "maxLength": 64}}, ["job_id"])),
    ("vm_start", "Start a registered VM supervisor once. Check vm_control/status for actual readiness. Client disconnect leaves the guest running.",
     schema({"vm": VM}, ["vm"])),
    ("vm_control", "Status, pause, resume or request clean guest shutdown. Shutdown acknowledgment does not establish completion. No hard-stop tool.",
     schema({"vm": VM, "action": {"type": "string", "enum": ["status", "pause", "resume", "shutdown"]}}, ["vm", "action"])),
    ("vm_capture", "Capture the registered VM's real display as a PNG MCP image and retain the artifact. A still frame, not a live stream.",
     schema({"vm": VM}, ["vm"])),
    ("vm_keys", "Send 1-8 physical QEMU key codes as one chord. Inspect the screen first; not text entry or acknowledgment of a UI action.",
     schema({"vm": VM, "keys": {"type": "array", "minItems": 1, "maxItems": 8,
                                "items": {"type": "string", "minLength": 1, "maxLength": 32}}}, ["vm", "keys"])),
    ("vm_click", "Send a primary click at normalized guest coordinates 0-32767. Inspect the capture for the actual effect.",
     schema({"vm": VM, "x": {"type": "integer", "minimum": 0, "maximum": 32767},
             "y": {"type": "integer", "minimum": 0, "maximum": 32767}}, ["vm", "x", "y"])),
    ("guest_ping", "Ping the interactive Windows or Ubuntu agent through the registered VM's virtio channel.",
     schema({"vm": VM}, ["vm"])),
    ("guest_launch", "Request one Windows .exe or Ubuntu absolute executable launch with literal arguments, based on registered guest_os. Never automatically retry a timeout; PID is not proof of a healthy app window.",
     schema({"vm": VM, "executable": {"type": "string", "minLength": 1, "maxLength": 4096},
             "arguments": {"type": "array", "maxItems": 128,
                           "items": {"type": "string", "maxLength": 4096}}}, ["vm", "executable"])),
]


def validate(value, spec):
    expected = spec["type"]
    good = {"object": isinstance(value, dict), "string": isinstance(value, str),
            "integer": type(value) is int, "array": isinstance(value, list)}[expected]
    if not good:
        raise ValueError(f"Expected {expected}")
    if "enum" in spec and value not in spec["enum"]:
        raise ValueError("Value outside the allowed choices")
    if expected == "object":
        if set(value) - set(spec["properties"]) or set(spec["required"]) - set(value):
            raise ValueError("Unknown or missing arguments")
        for key, item in value.items():
            validate(item, spec["properties"][key])
    elif expected in ("string", "array"):
        low = spec.get("minLength", spec.get("minItems", 0))
        high = spec.get("maxLength", spec.get("maxItems", MAX_MESSAGE))
        if not low <= len(value) <= high:
            raise ValueError("Argument length outside bounds")
        if expected == "string" and "\0" in value:
            raise ValueError("NUL is not permitted")
        if expected == "array":
            for item in value:
                validate(item, spec["items"])
    elif not spec.get("minimum", value) <= value <= spec.get("maximum", value):
        raise ValueError("Coordinate outside bounds")


def stop_process(process):
    # Development jobs only. Never call this for a VM supervisor.
    try:
        os.killpg(process.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass


def collect(process, timeout, update=None):
    """Drain stdout/stderr continuously, retain a bounded tail, enforce a total deadline."""
    tail = b""
    truncated = False
    timed_out = False
    deadline = time.monotonic() + timeout
    with selectors.DefaultSelector() as selector:
        selector.register(process.stdout, selectors.EVENT_READ)
        try:
            while selector.get_map() or process.poll() is None:
                if time.monotonic() >= deadline:
                    timed_out = True
                    stop_process(process)
                    break
                for key, _ in selector.select(0.1):
                    chunk = os.read(key.fileobj.fileno(), 4096)
                    if not chunk:
                        selector.unregister(key.fileobj)
                    else:
                        truncated |= len(tail) + len(chunk) > MAX_OUTPUT
                        tail = (tail + chunk)[-MAX_OUTPUT:]
                        if update:
                            update(tail, truncated)
            code = process.wait(timeout=5)
        finally:
            process.stdout.close()
    return {"exit_code": code, "timed_out": timed_out,
            "output_truncated": truncated, "output": tail.decode("utf-8", errors="replace")}


def ppm_png(data):
    """Convert actual QEMU P6 bytes to lossless PNG without an image dependency."""
    if len(data) > 64 * 1024 * 1024:
        raise ValueError("Display exceeds 64 MiB")
    tokens = []
    pos = 0
    while len(tokens) < 4:
        while pos < len(data) and data[pos] in b" \t\r\n":
            pos += 1
        if pos < len(data) and data[pos] == 35:
            end = data.find(b"\n", pos)
            if end < 0:
                raise ValueError("Invalid PPM comment")
            pos = end + 1
            continue
        start = pos
        while pos < len(data) and data[pos] not in b" \t\r\n":
            pos += 1
        if pos == start:
            raise ValueError("Truncated PPM header")
        tokens.append(data[start:pos])
    if tokens[0] != b"P6" or tokens[3] != b"255" or pos >= len(data):
        raise ValueError("Expected 8-bit P6 display")
    width, height = int(tokens[1]), int(tokens[2])
    if not 1 <= width <= 8192 or not 1 <= height <= 8192:
        raise ValueError("Invalid display dimensions")
    # Consume exactly the raster separator: leading pixel whitespace is image data.
    pos += 2 if data[pos:pos + 2] == b"\r\n" else 1
    pixels = memoryview(data)[pos:]
    if len(pixels) != width * height * 3:
        raise ValueError("Truncated or excessive PPM raster")
    compressor = zlib.compressobj()
    compressed = bytearray()
    stride = width * 3
    for row in range(height):
        compressed.extend(compressor.compress(b"\0"))
        compressed.extend(compressor.compress(pixels[row * stride:(row + 1) * stride]))
    compressed.extend(compressor.flush())
    if len(compressed) > 8 * 1024 * 1024:
        raise ValueError("PNG exceeds 8 MiB MCP image limit")
    def chunk(kind, payload):
        return struct.pack(">I", len(payload)) + kind + payload + struct.pack(">I", zlib.crc32(kind + payload))
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0))
            + chunk(b"IDAT", bytes(compressed)) + chunk(b"IEND", b""))


class Server:
    def __init__(self, config_path):
        config_path = Path(config_path).resolve(strict=True)
        config = tomllib.loads(config_path.read_text())
        if set(config) - {"repository", "artifacts", "job_timeout_seconds", "vms", "mobile_devices"}:
            raise ValueError("Unknown MCP configuration setting")
        self.repo = (config_path.parent / config["repository"]).resolve(strict=True)
        if not (self.repo / "Cargo.toml").is_file():
            raise ValueError("Repository must contain Cargo.toml")
        self.artifacts = (config_path.parent / config.get("artifacts", str(self.repo / "target/mcp"))).resolve()
        self.timeout = config.get("job_timeout_seconds", 600)
        if type(self.timeout) is not int or not 1 <= self.timeout <= 3600:
            raise ValueError("job_timeout_seconds must be 1-3600")
        self.vms = {name: (config_path.parent / path).resolve(strict=True)
                    for name, path in config.get("vms", {}).items()}
        for name, path in self.vms.items():
            validate(name, VM)
            if not path.is_file():
                raise ValueError("Registered VM config must be a file")
        self.mobile_devices = {name:(config_path.parent/path).resolve(strict=True) for name,path in config.get("mobile_devices",{}).items()}
        for name,path in self.mobile_devices.items():
            validate(name,VM)
            if not path.is_file() or path.stat().st_size>1024*1024:
                raise ValueError("Registered mobile profile must be a file under 1 MiB")
            if tomllib.loads(path.read_text()).get("platform") not in ("android","ios"):
                raise ValueError("Mobile profile must specify Android or iOS")
        self.jobs = {}
        self.lock = threading.Lock()
        self.supervisors = {}
        self.initialized = False

    def spawn(self, arguments, pipe=True):
        return subprocess.Popen(arguments, cwd=self.repo, stdin=subprocess.DEVNULL,
                                stdout=subprocess.PIPE if pipe else subprocess.DEVNULL,
                                stderr=subprocess.STDOUT, start_new_session=True)

    def cli(self, command, vm, *args):
        if vm not in self.vms:
            raise ValueError("VM is not registered; configure its name/path in the server's TOML")
        binary = self.repo / "target/debug/freelane"
        if not binary.is_file():
            raise ValueError("Build the CLI first with dev_run/build")
        return [str(binary), command, str(self.vms[vm]), *map(str, args)]

    def run_cli(self, command, vm, *args):
        return collect(self.spawn(self.cli(command, vm, *args)), 20)

    def mobile_cli(self,command,device,*arguments):
        if device not in self.mobile_devices:
            raise ValueError("Mobile device is not registered")
        binary=self.repo/"target/debug/freelane"
        if not binary.is_file():
            raise ValueError("Build the CLI first")
        return [str(binary),command,str(self.mobile_devices[device]),*map(str,arguments)]

    def call_mobile(self,name,arguments):
        device=arguments["device"]
        def run(command,*extra):
            return collect(self.spawn(self.mobile_cli(command,device,*extra)),150 if command=="mobile-install" else 45)
        if name=="mobile_control":
            action=arguments["action"]
            if action!="start":return run("mobile-"+action)
            command=self.mobile_cli("mobile-start",device)
            key="mobile:"+device
            existing=self.supervisors.get(key)
            if existing and existing.poll() is None:raise ValueError("Mobile supervisor is already running")
            state=run("mobile-status")
            if state["exit_code"]==0:
                try:
                    ready=json.loads(state["output"])
                except json.JSONDecodeError:
                    ready={}
                if ready.get("platform")=="android" or ready.get("state")=="Booted":
                    raise ValueError("Mobile device already running; inspect status")
            self.artifacts.mkdir(parents=True,exist_ok=True,mode=0o700)
            log_path=self.artifacts/(uuid.uuid4().hex+"-mobile-supervisor.log")
            with log_path.open("xb") as log:
                process=subprocess.Popen(command,cwd=self.repo,stdin=subprocess.DEVNULL,stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
            self.supervisors[key]=process
            threading.Thread(target=process.wait,daemon=True).start()
            return {"supervisor_pid":process.pid,"status":"starting","log":str(log_path),"next":"Poll mobile_control/status; client disconnect leaves the device running."}
        if name=="mobile_install":return run("mobile-install",arguments["application"])
        if name=="mobile_launch":return run("mobile-launch",arguments["application"])
        self.mobile_cli("mobile-capture",device)
        self.artifacts.mkdir(parents=True,exist_ok=True,mode=0o700)
        path=self.artifacts/(uuid.uuid4().hex+".png")
        result=run("mobile-capture",path)
        if result["exit_code"]!=0:return result
        if path.stat().st_size>8*1024*1024:raise ValueError("Mobile image exceeds 8 MiB; artifact retained")
        data=path.read_bytes()
        if not data.startswith(b"\x89PNG\r\n\x1a\n") or len(data)<33 or data[12:16]!=b"IHDR":raise ValueError("Mobile tool returned invalid PNG")
        width,height=struct.unpack(">II",data[16:24])
        if not 1<=width<=8192 or not 1<=height<=8192 or width*height*4>64*1024*1024:raise ValueError("Mobile PNG dimensions exceed decoding bounds")
        return {"content":[{"type":"text","text":json.dumps({"artifact":str(path),"device":device})},{"type":"image","mimeType":"image/png","data":base64.b64encode(data).decode()}],"isError":False}

    def start_job(self, task):
        with self.lock:
            if any(job["status"] == "running" for job in self.jobs.values()):
                raise ValueError("A development job is already running; poll its result before starting another")
            if len(self.jobs) >= 32:
                del self.jobs[next(iter(self.jobs))]
            process = self.spawn(TASKS[task])
            job_id = uuid.uuid4().hex
            self.jobs[job_id] = {"task": task, "status": "running", "process": process,
                                 "output": "", "output_truncated": False}
        def update(tail, truncated):
            with self.lock:
                self.jobs[job_id].update(output=tail.decode("utf-8", errors="replace"), output_truncated=truncated)
        def worker():
            try:
                result = collect(process, self.timeout, update)
                status = "timed_out" if result["timed_out"] else ("passed" if result["exit_code"] == 0 else "failed")
            except Exception as error:
                stop_process(process)
                process.wait()
                result, status = {"error": str(error)}, "failed"
            with self.lock:
                self.jobs[job_id].update(result, status=status)
        threading.Thread(target=worker, daemon=True).start()
        return {"job_id": job_id, "task": task, "status": "running"}

    def call(self, name, arguments):
        spec = next((spec for tool, _, spec in TOOLS if tool == name), None)
        if spec is None:
            raise ValueError("Unknown tool")
        validate(arguments, spec)
        if name == "host_info":
            return {"system": platform.system(), "architecture": platform.machine(),
                    "repository": str(self.repo), "vms": list(self.vms), "mobile_devices":list(self.mobile_devices),
                    "tools": {tool: shutil.which(tool) for tool in ("cargo", "qemu-system-aarch64", "qemu-img", "swtpm")},
                    "native_mac": sys.platform == "darwin" and platform.machine() == "arm64"}
        if name == "dev_run":
            return self.start_job(arguments["task"])
        if name == "dev_result":
            with self.lock:
                job = self.jobs.get(arguments["job_id"])
                if job is None:
                    raise ValueError("Unknown job ID (job history is session-local)")
                return {key: value for key, value in job.items() if key != "process"}
        if name.startswith("mobile_"):
            return self.call_mobile(name,arguments)
        vm = arguments["vm"]
        if name == "vm_start":
            command = self.cli("start", vm)
            existing = self.supervisors.get(vm)
            if existing and existing.poll() is None:
                raise ValueError("Supervisor is already running; use vm_control/status")
            # Also check adopted VMs. The Rust runtime lock handles any start race.
            if self.run_cli("status", vm)["exit_code"] == 0:
                raise ValueError("VM already exists; use vm_control/status")
            self.artifacts.mkdir(parents=True, exist_ok=True, mode=0o700)
            log_path = self.artifacts / (uuid.uuid4().hex + "-supervisor.log")
            with log_path.open("xb") as log:
                process = subprocess.Popen(command, cwd=self.repo, stdin=subprocess.DEVNULL,
                                           stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
            self.supervisors[vm] = process
            threading.Thread(target=process.wait, daemon=True).start()
            return {"supervisor_pid": process.pid, "status": "starting", "log": str(log_path),
                    "next": "Use vm_control/status to verify startup. Disconnect does not stop the VM."}
        if name == "vm_control":
            return self.run_cli(arguments["action"], vm)
        if name == "vm_capture":
            self.cli("screenshot", vm)  # Validate registered name before creating artifacts.
            self.artifacts.mkdir(parents=True, exist_ok=True, mode=0o700)
            path = self.artifacts / (uuid.uuid4().hex + ".ppm")
            result = self.run_cli("screenshot", vm, path)
            if result["exit_code"] != 0:
                return result
            if path.stat().st_size > 64 * 1024 * 1024:
                raise ValueError("Display exceeds 64 MiB; artifact retained")
            png = ppm_png(path.read_bytes())
            output = path.with_suffix(".png")
            with output.open("xb") as file:
                file.write(png)
            path.unlink()
            return {"content": [{"type": "text", "text": json.dumps({"artifact": str(output), "vm": vm})},
                                {"type": "image", "mimeType": "image/png", "data": base64.b64encode(png).decode()}],
                    "isError": False}
        if name == "vm_keys":
            return self.run_cli("send-key", vm, *arguments["keys"])
        if name == "vm_click":
            return self.run_cli("click", vm, arguments["x"], arguments["y"])
        if name == "guest_ping":
            return self.run_cli("agent-ping", vm)
        executable = arguments["executable"]
        if vm not in self.vms:
            raise ValueError("VM is not registered")
        guest_os = tomllib.loads(self.vms[vm].read_text()).get("guest_os", "windows")
        if guest_os == "ubuntu":
            if not executable.startswith("/"):
                raise ValueError("Use an absolute Ubuntu executable path")
        elif guest_os == "windows":
            if not ntpath.isabs(executable) or ntpath.splitext(executable)[1].lower() != ".exe":
                raise ValueError("Use an absolute Windows .exe path")
        else:
            raise ValueError("Unsupported guest_os in VM configuration")
        result = self.run_cli("launch", vm, executable, *arguments.get("arguments", []))
        result["next"] = "Inspect the actual app window. On a timeout execution may be unknown; do not automatically retry."
        return result

    def dispatch(self, message):
        if not isinstance(message, dict) or message.get("jsonrpc") != "2.0" or not isinstance(message.get("method"), str):
            return {"jsonrpc": "2.0", "id": None, "error": {"code": -32600, "message": "Invalid request"}}
        if "id" not in message:
            return None  # Notifications must not produce responses.
        request_id = message["id"]
        if type(request_id) not in (str, int):
            return {"jsonrpc": "2.0", "id": None, "error": {"code": -32600, "message": "Invalid request ID"}}
        response = {"jsonrpc": "2.0", "id": request_id}
        method = message["method"]
        params = message.get("params", {})
        if not isinstance(params, dict):
            response["error"] = {"code": -32602, "message": "Parameters must be an object"}
        elif method == "initialize":
            self.initialized = True
            requested = params.get("protocolVersion")
            response["result"] = {"protocolVersion": requested if requested in VERSIONS else VERSIONS[0],
                                  "capabilities": {"tools": {"listChanged": False}},
                                  "serverInfo": {"name": "freelane", "version": "0.1.0"},
                                  "instructions": "Use fixed dev jobs and registered VMs. Poll actual outcomes. Inspect captures before input; never replay an uncertain app launch. VM disconnect is not shutdown."}
        elif method == "ping":
            response["result"] = {}
        elif not self.initialized:
            response["error"] = {"code": -32000, "message": "Initialize first"}
        elif method == "tools/list":
            response["result"] = {"tools": [{"name": name, "description": description, "inputSchema": spec}
                                            for name, description, spec in TOOLS]}
        elif method == "tools/call":
            try:
                value = self.call(params.get("name"), params.get("arguments", {}))
                if isinstance(value, dict) and "content" in value:
                    response["result"] = value
                else:
                    failed = value.get("exit_code", 0) != 0 or value.get("timed_out", False) or value.get("status") in ("failed", "timed_out")
                    response["result"] = {"content": [{"type": "text", "text": json.dumps(value)}], "isError": failed}
            except Exception as error:
                response["result"] = {"content": [{"type": "text", "text": str(error)}], "isError": True}
        else:
            response["error"] = {"code": -32601, "message": "Method not found"}
        return response

    def close(self):
        with self.lock:
            for job in self.jobs.values():
                if job["status"] == "running":
                    stop_process(job["process"])
        # VM supervisors are independent, like the desktop's Start action. Never hard-stop on EOF.


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--config", required=True, help="Trusted local TOML with repository and registered VM paths")
    args = parser.parse_args()
    server = Server(args.config)
    try:
        while True:
            line = sys.stdin.buffer.readline(MAX_MESSAGE + 1)
            if not line:
                break
            if len(line) > MAX_MESSAGE:
                while line and not line.endswith(b"\n"):
                    line = sys.stdin.buffer.readline(MAX_MESSAGE + 1)
                response = {"jsonrpc": "2.0", "id": None, "error": {"code": -32700, "message": "Message exceeds 1 MiB"}}
            else:
                try:
                    response = server.dispatch(json.loads(line))
                except (ValueError, UnicodeDecodeError):
                    response = {"jsonrpc": "2.0", "id": None, "error": {"code": -32700, "message": "Invalid JSON"}}
            if response is not None:
                print(json.dumps(response, separators=(",", ":")), flush=True)
    finally:
        server.close()


if __name__ == "__main__":
    main()
