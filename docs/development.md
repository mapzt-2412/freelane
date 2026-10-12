# Build and run

## Rust checks

Use the pinned Rust 1.99.0 toolchain from the repository root:

```sh
cargo build --locked --workspace
cargo test --locked --workspace
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
```

Linux can exercise the portable code and software ARM emulation. The native target is macOS Apple Silicon. `doctor` intentionally exits 1 on unsupported native hosts, even when software testing is available.

## Mac baseline VM

On a real Apple Silicon Mac, the separate native framework probe needs a signed host executable with the hypervisor entitlement:

```sh
cargo build --locked --workspace
codesign --force --sign - --entitlements scripts/macos-hypervisor.entitlements.plist target/debug/freelane
codesign --verify --strict target/debug/freelane
target/debug/freelane probe-hypervisor
```

This uses local ad-hoc signing of the development binary. The probe creates and destroys an empty VM; it does not boot Windows. These native signing/runtime steps have not run in the Linux cloud environment. QEMU HVF acceleration depends on the QEMU executable's own signing and hardware support.

Install QEMU 10.0+ and swtpm from a trusted package manager (for example `brew install qemu swtpm`). Locate the installed AArch64 EDK2 **code** and **variable template** files; the two are different artifacts and must match. Names/locations vary by package. For Windows, use a Secure Boot capable code image and its matching variable template with Microsoft keys enrolled. On Debian trixie these are `AAVMF_CODE.ms.fd` and `AAVMF_VARS.ms.fd`. The generic `AAVMF_CODE.fd` links to a build without Secure Boot: our Windows Setup test rejected that pair for both TPM and Secure Boot. Merely attaching swtpm is insufficient. Do not substitute x86 OVMF firmware. Signed distribution firmware is preferable to an unverified download.

Create a new directory outside the source checkout; `init` refuses to overwrite an existing directory:

```sh
cargo run --locked -- init /absolute/path/to/new-vm /absolute/path/to/arm-code.fd /absolute/path/to/arm-vars.fd
```

This creates a sparse 64 GiB qcow2 disk, private firmware/NVRAM copies, private TPM state and `vm.toml`. Edit `vm.toml` to add your licensed Windows ARM ISO as the top-level `installer` field. Do not put the installer field under `[runtime]`. Microsoft provides the official download at https://www.microsoft.com/en-us/software-download/windows11arm64 ; verify its published SHA-256 before use.

Use at least the Windows-required RAM/CPU/storage resources. Defaults are 4 GiB RAM, 4 vCPUs and 64 GiB disk. `[runtime]` defaults to `accelerator = "hvf"`; `headless = false` uses QEMU's native Cocoa display on Mac. The selected `ramfb` display is a firmware/basic display experiment, **not accelerated gaming graphics**. The signed Debian Microsoft-key firmware pair passed Windows Setup security checks under TCG. The installed Audit desktop is observed under TCG; normal user setup, reboot stability and native Mac behavior remain unverified.

```sh
cargo run --locked -- validate /absolute/path/to/new-vm/vm.toml
cargo run --locked -- plan /absolute/path/to/new-vm/vm.toml
cargo run --locked -- start /absolute/path/to/new-vm/vm.toml
```

`plan` prints the exact argument array without executing a shell. `start` owns the QEMU/TPM processes in the foreground. Use another terminal for:

```sh
cargo run --locked -- status /absolute/path/to/new-vm/vm.toml
cargo run --locked -- pause /absolute/path/to/new-vm/vm.toml
cargo run --locked -- resume /absolute/path/to/new-vm/vm.toml
cargo run --locked -- shutdown /absolute/path/to/new-vm/vm.toml
```

`shutdown` requests guest powerdown; it does not claim the guest has shut down. Prefer it before `stop` or Ctrl-C, which end the host process without saving guest memory and can interrupt guest disk writes. Pausing does not return all guest RAM.

Networking is off by default. `network = true` enables user-mode networking with no forwarded host ports. Virtio PCI serial and network devices explicitly disable legacy port I/O on ARM. The default `network_device = "virtio"` requires a compatible ARM64 virtio network driver. `network_device = "usb"` selects QEMU USB networking as a compatibility experiment; its attachment is tested under UEFI, while Windows driver and connectivity validation are separate checks. An opt-in [guest agent](guest-agent.md) supplies ping/launch requests. The ARM64 agent answered ping and opened actual Notepad/7-Zip windows through virtio serial in WinPE and the installed Audit session; normal user-session setup remains unverified. Opt-in [read-only host-folder transfer](sharing.md) uses a virtual USB FAT disk. Clipboard, two-way shared-folder integration and individual application windows are not implemented yet.

For a headless guest, capture its display and send a keyboard chord directly:

```sh
cargo run --locked -- screenshot /absolute/path/to/vm.toml /absolute/path/to/new-frame.ppm
cargo run --locked -- send-key /absolute/path/to/vm.toml alt n
cargo run --locked -- send-key /absolute/path/to/vm.toml shift f10
```

Screenshots use PPM and refuse existing output paths. Keys use QEMU qcodes; each command sends one chord (up to eight keys), not text. The guest must be running; the sender allows the release timer to fire before returning so consecutive identical keys are not merged. A successful send acknowledges delivery, not the guest's action. These commands do not require the guest agent.

Startup reports a private runtime directory containing `qemu.log`, `serial.log`, `swtpm.log` and the QMP socket. Only one QMP client can connect at a time; monitor connections must be released between separate clients. Logs and sockets are temporary; disk, firmware variables and TPM state are persistent and must be backed up together.

## Linux ARM/UEFI integration tests

On Debian trixie, install trusted distribution packages `qemu-system-arm`, `qemu-utils`, `qemu-efi-aarch64` and `swtpm`, plus `mtools` and `xorriso` for the EFI installer-media fixture. Cloud setup installs/extracts these under `/workspace/.freelane-tools` without modifying system directories. Activate its wrappers:

```sh
export CARGO_HOME=/workspace/.freelane-tools/cargo
export RUSTUP_HOME=/workspace/.freelane-tools/rustup
export PATH=/workspace/.freelane-tools/bin:$CARGO_HOME/bin:$PATH
export FREELANE_TEST_FIRMWARE=/workspace/.freelane-tools/qemu-root/usr/share/AAVMF/AAVMF_CODE.fd
export FREELANE_TEST_NVRAM=/workspace/.freelane-tools/qemu-root/usr/share/AAVMF/AAVMF_VARS.fd
rustup target add aarch64-unknown-uefi
cargo test --locked --test qemu_smoke -- --ignored
```

Without these prerequisites, the six QEMU integration tests are explicitly ignored in the ordinary test command. They must be run separately to validate the backend. They execute an ARM guest, boot a locally built EFI guest from CD-ROM with Windows and Ubuntu VM profiles, boot UEFI with default and single-threaded TCG, and run a shell command through USB keyboard input under TCG. These tests do not boot Windows or Ubuntu kernels. Set `FREELANE_TEST_SCREENSHOT` to an output path to retain the UEFI test screenshot. See [Ubuntu setup](ubuntu.md) for actual installation and Linux guest-agent instructions.

For a software-emulation concurrency diagnostic, set `tcg_threading = "single"` or `"multi"` under `[runtime]` with `accelerator = "tcg"`. Omitting it preserves QEMU's default. This setting does not change guest CPU count or memory and is rejected with HVF. Single-threaded TCG is a diagnostic option, not a verified Windows crash fix or a performance improvement. Change it only after a clean guest shutdown.

The single-threaded UEFI test uses four guest CPUs and checks that QMP reports all four sharing one execution thread. It exercises the resource configuration used by the Windows diagnostic, without claiming that a UEFI test proves Windows stability.

For a manually launched software VM, set `accelerator = "tcg"` and `headless = true` in its config. TCG is a test mode, not a Mac gaming performance target.

## Frame measurements

Export real measured frame durations as a single-column CSV headed `frame_ms`. Use at least 100 frames; discard warm-up consistently before export:

```sh
cargo run --locked -- benchmark /absolute/path/to/measured-frames.csv
```

Average FPS is total frame count divided by total capture duration. The 1% low is 1000 divided by the mean duration of the slowest `ceil(N/100)` frames, not the average of instantaneous FPS values. Percentiles use linear interpolation at rank `(N-1)*p` (type 7). The count over 50 ms is a defined threshold metric, not an automatic diagnosis of stutter. Invalid, nonpositive, nonfinite or overflowing data is rejected.

Compare FreeLane and Parallels only with matched Mac/macOS/Windows versions, VM resources, game/app build, scene, settings and measurement method. Repeat runs and preserve raw captures. The synthetic data in unit tests is not a performance result.

For headless mouse input, `freelane click /absolute/path/to/vm.toml X Y` uses normalized coordinates 0–32767. It requires a running VM and sends a primary press/release through the USB tablet. QMP acceptance means delivery was requested; inspect the guest screen for the effect. GUI snapshots map clicks automatically.
