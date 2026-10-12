use freelane::{qemu, Backend, Config, Error, GuestOs, Result};
use std::{
    env,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

fn load(path: &str) -> Result<Config> {
    let text = std::fs::read_to_string(path).map_err(|e| Error(format!("{path}: {e}")))?;
    Config::parse(&text)
}
fn run() -> Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.as_slice() {
        [command] if command == "doctor" => {
            println!("Host: {} / {}", env::consts::OS, env::consts::ARCH);
            println!("Apple Silicon native acceleration host: {}", freelane::supported_host());
            match std::process::Command::new("qemu-system-aarch64").arg("--version").output() {
                Ok(output) if output.status.success() => println!("{}", String::from_utf8_lossy(&output.stdout).lines().next().unwrap_or("QEMU found")),
                _ => println!("QEMU missing: install QEMU before using start"),
            }
            println!("Guest boot and hardware acceleration require runtime verification on this host\nAccelerated guest graphics: not implemented");
            freelane::require_supported_host()
        }
        [command] if command == "probe-hypervisor" => {
            freelane::probe_hypervisor()?;
            println!("Empty native VM created and destroyed successfully; guest execution not tested.");
            Ok(())
        }
        [command, path] if command == "agent-ping" => {
            let reply = freelane::agent::request(Path::new(path), freelane_agent_protocol::Action::Ping)?;
            println!("{}", serde_json::to_string(&reply).map_err(|e| Error(e.to_string()))?);
            Ok(())
        }
        [command, path, executable, arguments @ ..] if command == "launch" => {
            let reply = freelane::agent::request(Path::new(path), freelane_agent_protocol::Action::Launch { executable: executable.clone(), arguments: arguments.to_vec() })?;
            println!("{}", serde_json::to_string(&reply).map_err(|e| Error(e.to_string()))?);
            Ok(())
        }
        [command, path, output] if command == "screenshot" => {
            qemu::screenshot(Path::new(path), Path::new(output))?;
            println!("Saved display frame to {output}");
            Ok(())
        }
        [command, path, keys @ ..] if command == "send-key" => {
            qemu::send_keys(Path::new(path), keys)?;
            println!("Key chord sent to guest");
            Ok(())
        }
        [command, path, x, y] if command == "click" => {
            let coordinate = |text: &str| text.parse::<u16>().map_err(|_| Error("Mouse coordinates must be integers between 0 and 32767".into()));
            qemu::click(Path::new(path), coordinate(x)?, coordinate(y)?)?;
            println!("Mouse click sent to guest; check the display for its effect");
            Ok(())
        }
        [command, path] if command == "benchmark" => {
            let text = std::fs::read_to_string(path).map_err(|e| Error(e.to_string()))?;
            let report = freelane::benchmark::FrameReport::from_csv(&text)?;
            println!("{}", serde_json::to_string_pretty(&report).map_err(|e| Error(e.to_string()))?);
            Ok(())
        }
        [command, path] if command == "validate" => {
            let config = load(path)?;
            config.check_images(Path::new(path).parent().unwrap_or(Path::new(".")))?;
            println!("Configuration and image accessibility checked: {} ({} CPUs, {} MiB). Image format/bootability not verified.", config.name, config.cpus, config.memory_mib);
            Ok(())
        }
        [command, directory, firmware, template] if command == "init" => {
            let path = qemu::initialize(Path::new(directory), Path::new(firmware), Path::new(template), Path::new("qemu-img"))?;
            println!("Created {} with a sparse 64 GiB disk. Add your licensed Windows ARM ISO as installer in vm.toml before installation.", path.display());
            Ok(())
        }
        [command, directory, firmware, template, guest] if command == "init" => {
            let guest_os = match guest.as_str() {
                "windows" => GuestOs::Windows,
                "ubuntu" => GuestOs::Ubuntu,
                _ => return Err(Error("Guest OS must be windows or ubuntu".into())),
            };
            let path = qemu::initialize_guest(Path::new(directory), Path::new(firmware), Path::new(template), Path::new("qemu-img"), guest_os)?;
            println!("Created {} with a sparse 64 GiB disk. Add a matching ARM64 installer image as installer in vm.toml.", path.display());
            Ok(())
        }
        [command, path] if command == "plan" => {
            let config = load(path)?;
            let base = Path::new(path).parent().unwrap_or(Path::new("."));
            let arguments = qemu::arguments(&config, base, &qemu::runtime_dir(Path::new(path))?.join("qmp.sock"))?;
            println!("{}", serde_json::to_string_pretty(&arguments).map_err(|e| Error(e.to_string()))?);
            Ok(())
        }
        [command, path] if command == "start" => {
            let config = load(path)?;
            let interrupted = Arc::new(AtomicBool::new(false));
            let signal = interrupted.clone();
            ctrlc::set_handler(move || signal.store(true, Ordering::SeqCst)).map_err(|e| Error(e.to_string()))?;
            let mut backend = qemu::QemuBackend::new(Path::new(path))?;
            backend.prepare(&config)?;
            backend.start()?;
            println!("VM started. Monitor/log directory: {}", backend.directory().display());
            println!("Ctrl-C stops QEMU. Use shutdown from another terminal for a guest shutdown request.");
            loop {
                if let Some(status) = backend.poll()? {
                    if status.success() { return Ok(()); }
                    return Err(Error(format!("QEMU exited with {status}; see {}/qemu.log", backend.directory().display())));
                }
                if interrupted.load(Ordering::SeqCst) { backend.stop()?; return Ok(()); }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
        [command, path] if ["status", "pause", "resume", "shutdown", "stop"].contains(&command.as_str()) => {
            println!("{}", qemu::control(Path::new(path), command)?);
            if command == "shutdown" { println!("Guest powerdown requested; completion is not guaranteed."); }
            Ok(())
        }
        _ => Err(Error("Usage: freelane doctor | probe-hypervisor | init <new-dir> <firmware.fd> <vars-template.fd> [windows|ubuntu] | agent-ping <config.toml> | launch <config.toml> <absolute-guest-executable> [args...] | screenshot <config.toml> <new-output.ppm> | send-key <config.toml> <qcode> [qcodes...] | click <config.toml> <x:0..32767> <y:0..32767> | benchmark <frame_ms.csv> | validate <config.toml> | plan <config.toml> | start <config.toml> | status|pause|resume|shutdown|stop <config.toml>".into())),
    }
}
fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}
