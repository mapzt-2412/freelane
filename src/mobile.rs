//! Official Android Emulator/ADB and Apple Xcode Simulator adapters, not iPhone emulation.
use crate::{Error, Result};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Android,
    Ios,
}
#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Acceleration {
    #[default]
    Hardware,
    Software,
}
#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Gpu {
    #[default]
    Auto,
    Host,
    Software,
    Swiftshader,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Android {
    pub sdk_root: PathBuf,
    pub avd: String,
    pub avd_home: Option<PathBuf>,
    pub user_home: Option<PathBuf>,
    #[serde(default = "port")]
    pub console_port: u16,
    #[serde(default = "cpus")]
    pub cpus: u16,
    #[serde(default = "memory")]
    pub memory_mib: u64,
    #[serde(default)]
    pub acceleration: Acceleration,
    #[serde(default)]
    pub gpu: Gpu,
    #[serde(default)]
    pub headless: bool,
}
fn port() -> u16 {
    5554
}
fn cpus() -> u16 {
    2
}
fn memory() -> u64 {
    2048
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Ios {
    pub device_udid: String,
    pub developer_dir: Option<PathBuf>,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub name: String,
    pub platform: Platform,
    pub android: Option<Android>,
    pub ios: Option<Ios>,
}
#[derive(Debug, Serialize)]
pub struct Plan {
    pub program: PathBuf,
    pub arguments: Vec<String>,
    pub environment: BTreeMap<String, String>,
}
impl Profile {
    pub fn parse(text: &str) -> Result<Self> {
        let p: Self = toml::from_str(text).map_err(|e| Error(e.to_string()))?;
        p.validate()?;
        Ok(p)
    }
    pub fn validate(&self) -> Result<()> {
        if self.name.trim().is_empty()
            || self.name.len() > 128
            || self.name.chars().any(char::is_control)
        {
            return Err(Error(
                "Mobile name must be 1–128 bytes without control characters".into(),
            ));
        }
        match (self.platform, &self.android, &self.ios) {
            (Platform::Android, Some(a), None) => {
                if a.sdk_root.as_os_str().is_empty()
                    || a.avd.is_empty()
                    || a.avd.len() > 64
                    || !a
                        .avd
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || "_.-".contains(c))
                    || !a.avd.chars().next().unwrap().is_ascii_alphanumeric()
                {
                    return Err(Error("Specify an SDK root and a valid AVD name".into()));
                }
                if !(5554..=5682).contains(&a.console_port)
                    || a.console_port % 2 != 0
                    || !(1..=32).contains(&a.cpus)
                    || !(512..=65536).contains(&a.memory_mib)
                {
                    return Err(Error("Android needs an even console port 5554–5682, 1–32 CPUs and 512–65536 MiB RAM".into()));
                }
                if a.avd_home
                    .iter()
                    .chain(a.user_home.iter())
                    .any(|p| p.as_os_str().is_empty())
                {
                    return Err(Error(
                        "Optional Android home paths must not be empty".into(),
                    ));
                }
            }
            (Platform::Ios, None, Some(i)) => {
                let b = i.device_udid.as_bytes();
                if b.len() != 36
                    || b.iter().enumerate().any(|(n, c)| {
                        if [8, 13, 18, 23].contains(&n) {
                            *c != b'-'
                        } else {
                            !c.is_ascii_hexdigit()
                        }
                    })
                {
                    return Err(Error("Choose an exact Simulator device UUID; booted/all aliases are not accepted".into()));
                }
                if i.developer_dir
                    .as_ref()
                    .is_some_and(|p| p.as_os_str().is_empty())
                {
                    return Err(Error("Developer directory must not be empty".into()));
                }
            }
            _ => {
                return Err(Error(
                    "Mobile platform must have exactly its matching [android] or [ios] section"
                        .into(),
                ))
            }
        }
        Ok(())
    }
    fn plan(&self, base: &Path, program: PathBuf, args: Vec<String>) -> Result<Plan> {
        self.validate()?;
        let mut environment = BTreeMap::new();
        if let Some(a) = &self.android {
            environment.insert(
                "ANDROID_HOME".into(),
                base.join(&a.sdk_root).display().to_string(),
            );
            if let Some(p) = &a.avd_home {
                environment.insert(
                    "ANDROID_AVD_HOME".into(),
                    base.join(p).display().to_string(),
                );
            }
            if let Some(p) = &a.user_home {
                for key in ["ANDROID_USER_HOME", "ANDROID_EMULATOR_HOME"] {
                    environment.insert(key.into(), base.join(p).display().to_string());
                }
            }
        }
        if let Some(i) = &self.ios {
            if let Some(p) = &i.developer_dir {
                environment.insert("DEVELOPER_DIR".into(), base.join(p).display().to_string());
            }
        }
        Ok(Plan {
            program,
            arguments: args,
            environment,
        })
    }
    pub fn start_plan(&self, base: &Path) -> Result<Plan> {
        self.validate()?;
        match self.platform {
            Platform::Android => {
                let a = self.android.as_ref().unwrap();
                let mut args = vec![
                    "-avd".into(),
                    a.avd.clone(),
                    "-port".into(),
                    a.console_port.to_string(),
                    "-accel".into(),
                    if a.acceleration == Acceleration::Hardware {
                        "on".into()
                    } else {
                        "off".into()
                    },
                    "-gpu".into(),
                    match a.gpu {
                        Gpu::Auto => "auto",
                        Gpu::Host => "host",
                        Gpu::Software => "software",
                        Gpu::Swiftshader => "swiftshader",
                    }
                    .into(),
                    "-cores".into(),
                    a.cpus.to_string(),
                    "-memory".into(),
                    a.memory_mib.to_string(),
                    "-no-snapshot".into(),
                    "-no-boot-anim".into(),
                ];
                if a.headless {
                    args.push("-no-window".into());
                }
                self.plan(base, base.join(&a.sdk_root).join("emulator/emulator"), args)
            }
            Platform::Ios => self.ios_plan(base, &["boot"]),
        }
    }
    fn adb(&self, base: &Path, args: &[&str]) -> Result<Plan> {
        let a = self
            .android
            .as_ref()
            .ok_or_else(|| Error("Android profile required".into()))?;
        let mut arguments = vec!["-s".into(), format!("emulator-{}", a.console_port)];
        arguments.extend(args.iter().map(|v| (*v).to_owned()));
        self.plan(
            base,
            base.join(&a.sdk_root).join("platform-tools/adb"),
            arguments,
        )
    }
    fn ios_plan(&self, base: &Path, args: &[&str]) -> Result<Plan> {
        let i = self
            .ios
            .as_ref()
            .ok_or_else(|| Error("iOS Simulator profile required".into()))?;
        let mut arguments = vec!["simctl".into(), args[0].into(), i.device_udid.clone()];
        arguments.extend(args[1..].iter().map(|s| (*s).to_owned()));
        self.plan(base, "xcrun".into(), arguments)
    }
    pub fn install_plan(&self, base: &Path, application: &Path) -> Result<Plan> {
        let application = fs::canonicalize(application).map_err(|e| Error(e.to_string()))?;
        match self.platform {
            Platform::Android => {
                if !application.is_file()
                    || application.extension().and_then(|s| s.to_str()) != Some("apk")
                {
                    return Err(Error(
                        "Android install requires an existing .apk file".into(),
                    ));
                }
                // One streamed install also works on older Android guests; do not
                // let recent ADB attempt incremental installation then replay it.
                self.adb(
                    base,
                    &["install", "--no-incremental", "-r", text(&application)?],
                )
            }
            Platform::Ios => {
                if !application.is_dir()
                    || application.extension().and_then(|s| s.to_str()) != Some("app")
                {
                    return Err(Error("iOS Simulator requires a Simulator-target .app directory, not an App Store IPA".into()));
                }
                self.ios_plan(base, &["install", text(&application)?])
            }
        }
    }
    pub fn launch_plan(&self, base: &Path, application: &str) -> Result<Plan> {
        let valid = !application.is_empty()
            && application.len() <= 256
            && application
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic())
            && application.chars().all(|c| {
                c.is_ascii_alphanumeric()
                    || if self.platform == Platform::Ios {
                        ".-"
                    } else {
                        "._/"
                    }
                    .contains(c)
            });
        if !valid {
            return Err(Error(
                "Use a literal Android package/activity or iOS bundle identifier".into(),
            ));
        }
        match self.platform {
            Platform::Android => {
                if application.matches('/').count() != 1 || application.ends_with('/') {
                    return Err(Error("Android launch needs package/activity, e.g. com.android.settings/.Settings".into()));
                }
                self.adb(base, &["shell", "am", "start", "-W", "-n", application])
            }
            Platform::Ios => {
                if application.contains('/') {
                    return Err(Error("iOS launch needs a bundle identifier".into()));
                }
                self.ios_plan(base, &["launch", application])
            }
        }
    }
    fn require_host(&self) -> Result<()> {
        if self.platform == Platform::Ios && !cfg!(target_os = "macos") {
            return Err(Error("iOS Simulator requires macOS with Xcode and an installed Simulator runtime; it cannot run on this Linux host".into()));
        }
        Ok(())
    }
    pub fn devices(&self, base: &Path) -> Result<String> {
        self.require_host()?;
        let plan = match self.platform {
            Platform::Android => self.plan(
                base,
                base.join(&self.android.as_ref().unwrap().sdk_root)
                    .join("emulator/emulator"),
                vec!["-list-avds".into()],
            )?,
            Platform::Ios => self.plan(
                base,
                "xcrun".into(),
                vec![
                    "simctl".into(),
                    "list".into(),
                    "devices".into(),
                    "available".into(),
                    "--json".into(),
                ],
            )?,
        };
        Ok(
            String::from_utf8_lossy(&execute(&plan, Duration::from_secs(15), 1024 * 1024)?)
                .into_owned(),
        )
    }
    fn verify_android(&self, base: &Path) -> Result<()> {
        if self.platform != Platform::Android {
            return Ok(());
        }
        let reply = execute(
            &self.adb(base, &["emu", "avd", "name"])?,
            Duration::from_secs(10),
            64 * 1024,
        )?;
        let reply = String::from_utf8_lossy(&reply);
        if reply.lines().next().map(str::trim) != Some(self.android.as_ref().unwrap().avd.as_str())
        {
            return Err(Error(
                "Emulator port belongs to a different AVD; operation refused".into(),
            ));
        }
        Ok(())
    }
    pub fn status(&self, base: &Path) -> Result<serde_json::Value> {
        self.require_host()?;
        self.verify_android(base)?;
        match self.platform {
            Platform::Android => {
                let state = execute(
                    &self.adb(base, &["get-state"])?,
                    Duration::from_secs(10),
                    64 * 1024,
                )?;
                let boot = execute(
                    &self.adb(base, &["shell", "getprop", "sys.boot_completed"])?,
                    Duration::from_secs(10),
                    64 * 1024,
                )?;
                Ok(
                    json!({"platform":"android","device":format!("emulator-{}",self.android.as_ref().unwrap().console_port),"state":String::from_utf8_lossy(&state).trim(),"boot_completed":String::from_utf8_lossy(&boot).trim()=="1"}),
                )
            }
            Platform::Ios => {
                let devices: serde_json::Value =
                    serde_json::from_str(&self.devices(base)?).map_err(|e| Error(e.to_string()))?;
                let udid = &self.ios.as_ref().unwrap().device_udid;
                let device = devices["devices"]
                    .as_object()
                    .into_iter()
                    .flat_map(|r| r.values())
                    .filter_map(|v| v.as_array())
                    .flatten()
                    .find(|d| {
                        d["udid"]
                            .as_str()
                            .is_some_and(|value| value.eq_ignore_ascii_case(udid))
                    });
                let d = device.ok_or_else(|| {
                    Error("Configured Simulator is not available in Xcode".into())
                })?;
                Ok(
                    json!({"platform":"ios","device":udid,"state":d["state"],"boot_completed":false,"readiness":"Use mobile-wait; Booted state alone is not app readiness"}),
                )
            }
        }
    }
    pub fn control(&self, base: &Path, action: &str) -> Result<String> {
        self.require_host()?;
        self.verify_android(base)?;
        let plan = match action {
            "stop" => match self.platform {
                Platform::Android => self.adb(base, &["emu", "kill"])?,
                Platform::Ios => self.ios_plan(base, &["shutdown"])?,
            },
            "window" if self.platform == Platform::Ios => self.plan(
                base,
                "open".into(),
                vec![
                    "-a".into(),
                    "Simulator".into(),
                    "--args".into(),
                    "-CurrentDeviceUDID".into(),
                    self.ios.as_ref().unwrap().device_udid.clone(),
                ],
            )?,
            _ => return Err(Error(
                "Mobile control supports stop, or window for iOS; pause/resume are not implemented"
                    .into(),
            )),
        };
        Ok(
            String::from_utf8_lossy(&execute(&plan, Duration::from_secs(30), 64 * 1024)?)
                .into_owned(),
        )
    }
    pub fn install(&self, base: &Path, path: &Path) -> Result<String> {
        self.require_host()?;
        self.verify_android(base)?;
        let output = String::from_utf8_lossy(&execute(
            &self.install_plan(base, path)?,
            Duration::from_secs(120),
            64 * 1024,
        )?)
        .into_owned();
        if self.platform == Platform::Android
            && !output.lines().any(|line| line.trim() == "Success")
        {
            return Err(Error(format!(
                "APK installation did not report Success: {output}"
            )));
        }
        Ok(output)
    }
    pub fn launch(&self, base: &Path, app: &str) -> Result<String> {
        self.require_host()?;
        self.verify_android(base)?;
        let output = String::from_utf8_lossy(&execute(
            &self.launch_plan(base, app)?,
            Duration::from_secs(45),
            64 * 1024,
        )?)
        .into_owned();
        if self.platform == Platform::Android
            && !output.lines().any(|line| line.trim() == "Status: ok")
        {
            return Err(Error(format!(
                "Android activity did not report Status: ok: {output}"
            )));
        }
        Ok(output)
    }
    pub fn capture(&self, base: &Path, path: &Path) -> Result<()> {
        self.require_host()?;
        self.verify_android(base)?;
        use std::os::unix::fs::OpenOptionsExt;
        if path.extension().and_then(|s| s.to_str()) != Some("png") {
            return Err(Error("Mobile screenshots require a new .png path".into()));
        }
        let path = std::env::current_dir()
            .map_err(|e| Error(e.to_string()))?
            .join(path);
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .map_err(|e| Error(e.to_string()))?;
        let result = (|| {
            match self.platform {
                Platform::Android => {
                    let bytes = execute(
                        &self.adb(base, &["exec-out", "screencap", "-p"])?,
                        Duration::from_secs(30),
                        64 * 1024 * 1024,
                    )?;
                    if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
                        return Err(Error("ADB returned no PNG frame".into()));
                    }
                    file.write_all(&bytes).map_err(|e| Error(e.to_string()))?;
                    file.sync_all().map_err(|e| Error(e.to_string()))?;
                }
                Platform::Ios => {
                    execute(
                        &self.ios_plan(base, &["io", "screenshot", text(&path)?])?,
                        Duration::from_secs(30),
                        64 * 1024,
                    )?;
                    if fs::metadata(&path).map_err(|e| Error(e.to_string()))?.len()
                        > 64 * 1024 * 1024
                    {
                        return Err(Error("Simulator PNG exceeds 64 MiB".into()));
                    }
                    let mut signature = [0u8; 8];
                    fs::File::open(&path)
                        .and_then(|mut f| f.read_exact(&mut signature))
                        .map_err(|e| Error(e.to_string()))?;
                    if signature != *b"\x89PNG\r\n\x1a\n" {
                        return Err(Error("Simulator returned no PNG frame".into()));
                    }
                }
            }
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&path);
        }
        result
    }
    pub fn wait(&self, base: &Path) -> Result<()> {
        self.require_host()?;
        if self.platform == Platform::Ios {
            execute(
                &self.ios_plan(base, &["bootstatus", "-b"])?,
                Duration::from_secs(600),
                64 * 1024,
            )?;
            return Ok(());
        }
        let deadline = Instant::now() + Duration::from_secs(600);
        loop {
            if self.status(base).is_ok_and(|s| s["boot_completed"] == true) {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(Error(
                    "Android did not report sys.boot_completed=1 within 600 seconds".into(),
                ));
            }
            std::thread::sleep(Duration::from_secs(1));
        }
    }
}
fn text(path: &Path) -> Result<&str> {
    path.to_str()
        .ok_or_else(|| Error("Mobile SDK paths must be UTF-8".into()))
}
fn command(plan: &Plan) -> Command {
    let mut c = Command::new(&plan.program);
    c.args(&plan.arguments)
        .envs(&plan.environment)
        .stdin(Stdio::null());
    c
}
fn execute(plan: &Plan, timeout: Duration, limit: usize) -> Result<Vec<u8>> {
    let mut child = command(plan)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| Error(format!("{}: {e}", plan.program.display())))?;
    let output = Arc::new(Mutex::new(Vec::new()));
    let errors = Arc::new(Mutex::new(Vec::new()));
    let overflow = Arc::new(AtomicBool::new(false));
    let mut threads = Vec::new();
    for (reader, buffer, maximum) in [
        (
            Box::new(child.stdout.take().unwrap()) as Box<dyn Read + Send>,
            Arc::clone(&output),
            limit,
        ),
        (
            Box::new(child.stderr.take().unwrap()) as Box<dyn Read + Send>,
            Arc::clone(&errors),
            64 * 1024,
        ),
    ] {
        let overflow = Arc::clone(&overflow);
        threads.push(std::thread::spawn(move || {
            let mut reader = reader;
            let mut bytes = [0u8; 8192];
            while let Ok(count) = reader.read(&mut bytes) {
                if count == 0 {
                    break;
                }
                if let Ok(mut stored) = buffer.lock() {
                    let keep = count.min(maximum.saturating_sub(stored.len()));
                    stored.extend_from_slice(&bytes[..keep]);
                    if keep < count {
                        overflow.store(true, Ordering::SeqCst);
                    }
                }
            }
        }));
    }
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break Ok(s),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                break Err(Error("Mobile tool deadline exceeded; mutating operation outcome may be unknown, inspect before retrying".into()));
            }
            Err(e) => break Err(Error(e.to_string())),
        }
    };
    let stream_deadline = Instant::now() + Duration::from_secs(1);
    for thread in threads {
        while !thread.is_finished() && Instant::now() < stream_deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        if !thread.is_finished() {
            return Err(Error(
                "Mobile tool output stream stayed open; inspect before retrying".into(),
            ));
        }
        let _ = thread.join();
    }
    let bytes = output.lock().unwrap().clone();
    let diagnostic = errors.lock().unwrap().clone();
    let status = status?;
    if !status.success() {
        return Err(Error(format!(
            "Mobile tool exited {status}: {} {}",
            String::from_utf8_lossy(&bytes),
            String::from_utf8_lossy(&diagnostic)
        )));
    }
    if overflow.load(Ordering::SeqCst) {
        return Err(Error(
            "Mobile tool output exceeds bounds; inspect before retrying".into(),
        ));
    }
    Ok(bytes)
}
pub fn start(profile: &Profile, base: &Path, path: &Path) -> Result<()> {
    profile.require_host()?;
    if profile.platform == Platform::Ios {
        execute(
            &profile.start_plan(base)?,
            Duration::from_secs(60),
            64 * 1024,
        )?;
        return Ok(());
    }
    use std::os::unix::fs::OpenOptionsExt;
    let lock_path = path.with_extension("mobile.lock");
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(lock_path)
        .map_err(|e| Error(e.to_string()))?;
    lock.try_lock_exclusive()
        .map_err(|_| Error("This mobile emulator supervisor is already running".into()))?;
    if execute(
        &profile.adb(base, &["emu", "avd", "name"])?,
        Duration::from_secs(5),
        64 * 1024,
    )
    .is_ok()
    {
        return Err(Error(
            "The configured emulator port already has a device; inspect it before starting".into(),
        ));
    }
    let interrupted = Arc::new(AtomicBool::new(false));
    let signal = Arc::clone(&interrupted);
    ctrlc::set_handler(move || signal.store(true, Ordering::SeqCst))
        .map_err(|e| Error(e.to_string()))?;
    let mut child = command(&profile.start_plan(base)?)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| Error(e.to_string()))?;
    loop {
        if let Some(status) = child.try_wait().map_err(|e| Error(e.to_string()))? {
            return if status.success() {
                Ok(())
            } else {
                Err(Error(format!("Android Emulator exited {status}")))
            };
        }
        if interrupted.load(Ordering::SeqCst) {
            let _ = profile.control(base, "stop");
            // Give the SDK time to flush retained userdata after emu kill.
            let deadline = Instant::now() + Duration::from_secs(10);
            while child.try_wait().ok().flatten().is_none() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(100));
            }
            if child.try_wait().ok().flatten().is_none() {
                let _ = child.kill();
            }
            let _ = child.wait();
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}
pub fn load(path: &Path) -> Result<Profile> {
    if fs::metadata(path).map_err(|e| Error(e.to_string()))?.len() > 1024 * 1024 {
        return Err(Error("Mobile profile exceeds 1 MiB".into()));
    }
    Profile::parse(&fs::read_to_string(path).map_err(|e| Error(e.to_string()))?)
}
