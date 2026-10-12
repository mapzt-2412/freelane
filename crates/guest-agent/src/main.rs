#[cfg(any(windows, target_os = "linux"))]
fn main() {
    use std::{fs::OpenOptions, io::BufReader, time::Duration};
    let args: Vec<String> = std::env::args().skip(1).collect();
    #[cfg(windows)]
    let mut launcher = freelane_guest_agent::WindowsLauncher;
    #[cfg(target_os = "linux")]
    let mut launcher = freelane_guest_agent::LinuxLauncher;
    if args == ["--stdio"] {
        let result = freelane_guest_agent::serve(
            &mut std::io::stdin().lock(),
            &mut std::io::stdout().lock(),
            &mut launcher,
        );
        if let Err(error) = result {
            eprintln!("Guest protocol: {error}");
            std::process::exit(1);
        }
        return;
    }
    let path = match args.as_slice() {
        [] => default_device().into(),
        [path] if !path.starts_with("--") => path.clone(),
        _ => {
            eprintln!("Usage: freelane-guest-agent [device-path | --stdio]");
            std::process::exit(2);
        }
    };
    loop {
        match OpenOptions::new().read(true).write(true).open(&path) {
            Ok(file) => match file.try_clone() {
                Ok(reader) => {
                    let _ = freelane_guest_agent::serve(
                        &mut BufReader::new(reader),
                        &mut { file },
                        &mut launcher,
                    );
                }
                Err(_) => eprintln!("Cannot clone guest channel handle"),
            },
            Err(_) => {
                eprintln!("Guest channel unavailable; check virtio serial and device permissions")
            }
        }
        std::thread::sleep(Duration::from_secs(1));
    }
}
#[cfg(windows)]
fn default_device() -> &'static str {
    r"\\.\Global\org.freelane.agent.0"
}
#[cfg(target_os = "linux")]
fn default_device() -> &'static str {
    "/dev/virtio-ports/org.freelane.agent.0"
}
#[cfg(not(any(windows, target_os = "linux")))]
fn main() {
    eprintln!("FreeLane guest agent requires Windows or Linux; this is a guest-only binary.");
    std::process::exit(1);
}
