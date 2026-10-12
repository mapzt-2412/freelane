#[cfg(windows)]
fn main() {
    use std::{fs::OpenOptions, io::BufReader, time::Duration};
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args == ["--stdio"] {
        let result = freelane_guest_agent::serve(
            &mut std::io::stdin().lock(),
            &mut std::io::stdout().lock(),
            &mut freelane_guest_agent::WindowsLauncher,
        );
        if let Err(error) = result {
            eprintln!("Guest protocol: {error}");
            std::process::exit(1);
        }
        return;
    }
    let path = match args.as_slice() {
        [] => r"\\.\Global\org.freelane.agent.0".into(),
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
                        &mut freelane_guest_agent::WindowsLauncher,
                    );
                }
                Err(_) => eprintln!("Cannot clone guest channel handle"),
            },
            Err(_) => eprintln!("Guest channel unavailable; check the ARM64 virtio-serial driver"),
        }
        std::thread::sleep(Duration::from_secs(1));
    }
}
#[cfg(not(windows))]
fn main() {
    eprintln!("FreeLane guest agent requires Windows; protocol tests can run on this host.");
    std::process::exit(1);
}
