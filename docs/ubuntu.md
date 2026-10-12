# Ubuntu ARM64 guest support

FreeLane now has Windows and Ubuntu VM profiles. `guest_os = "ubuntu"` selects Ubuntu naming, an `ubuntu.qcow2` disk, app-path hints and Linux executable validation in MCP. The underlying ARM QEMU/HVF backend is shared. Existing configs without `guest_os` remain Windows configs. Ubuntu is intended to run on Apple Silicon with HVF; Linux cloud uses TCG for correctness tests.

## Create a separate Ubuntu VM

Obtain an official **ARM64** installer from https://cdimage.ubuntu.com/releases/24.04/release/ and verify the selected ISO against the official SHA256SUMS. Verify SHA256SUMS.gpg against a trusted Ubuntu CD-image signing key when available; do not disable TLS or change a checksum to accept an image. Ubuntu Server ARM64 is a baseline, not a desktop image. For a graphical guest, choose a supported generic ARM64 desktop installer or install the Ubuntu desktop packages in your own server guest. A Raspberry Pi-specific image is not a generic QEMU UEFI installer.

Use separate VM directories, disks and NVRAM. Do not reuse the Windows state:

```sh
cargo build --locked --workspace
target/debug/freelane init /absolute/path/to/new-ubuntu-vm /absolute/path/to/arm-code.fd /absolute/path/to/arm-vars.fd ubuntu
```

The final `ubuntu` selects the new profile. Omitting it preserves the original Windows behavior. Each VM gets a new sparse 64 GiB disk, private firmware/NVRAM and 4 CPUs/4096 MiB defaults. Ubuntu has no TPM attached by default; Windows keeps its existing TPM default. A private unused TPM directory may exist in the Ubuntu state directory. There is no automatic change to any Windows VM.

Edit the generated config's top-level `installer` to your verified ARM64 ISO path; see [ubuntu.toml](ubuntu.toml). On Mac use `accelerator = "hvf"` and a signed/working ARM QEMU executable. In Linux cloud use `accelerator = "tcg"` and `headless = true`. Ubuntu can use the regular matching ARM UEFI code/variable pair; Windows' firmware requirements remain documented separately.

```sh
target/debug/freelane validate /absolute/path/to/new-ubuntu-vm/vm.toml
target/debug/freelane plan /absolute/path/to/new-ubuntu-vm/vm.toml
target/debug/freelane start /absolute/path/to/new-ubuntu-vm/vm.toml
```

Complete Ubuntu's normal installation and user setup in the guest. After a clean shutdown, remove the installer entry to boot the installed disk. Networking remains off by default. Ubuntu normally supplies virtio kernel drivers; set `runtime.network=true` only when needed and verify actual connectivity. There are no forwarded host ports by default. Native QEMU's display handles interactive keyboard use; the manager displays snapshots and primary clicks. `ramfb` is basic display, not an accelerated Linux GPU implementation.

## Linux ARM64 guest agent

Build a static ARM64 Linux executable using Rust's bundled linker and musl target:

```sh
bash scripts/build-linux-agent.sh
```

Output: `target/guest-agent-linux-arm64/freelane-guest-agent`, with FreeLane and Rust runtime notices alongside it. The build script works without a separate cross-GCC/sysroot or pip package. Preserve its notices when distributing the binary. For an agent built inside Ubuntu itself, use `cargo build --locked -p freelane-guest-agent --release`.

Copy the ARM64 binary into the Ubuntu VM using a transfer method you control. Set `runtime.guest_agent=true` in the VM config **after a clean guest shutdown**, then cold-start it. The expected device is `/dev/virtio-ports/org.freelane.agent.0`. Confirm the actual device and its permissions. Linux uses the same bounded JSON protocol as Windows and launches absolute executable files with literal arguments; it never adds a shell interpreter implicitly. Completed children are reaped asynchronously.

Run the agent from a terminal in the signed-in desktop session, with its inherited display/Wayland and session-bus environment. Do not run it as root or a system service to get graphical windows. If the serial device is restricted to root, an administrator can grant a dedicated group access inside the guest, for example:

```sh
sudo groupadd -f freelane
sudo usermod -aG freelane "$USER"
echo 'SUBSYSTEM=="virtio-ports", ATTR{name}=="org.freelane.agent.0", GROUP="freelane", MODE="0660"' | sudo tee /etc/udev/rules.d/70-freelane-agent.rules
sudo udevadm control --reload-rules
```

Log out/in for the new group and reboot the guest or trigger rules for the intended device. Verify group access before starting the agent as the normal user. These are setup instructions, not operations already verified in the Ubuntu guest. No permission change or service is applied automatically by FreeLane.

```sh
# Inside Ubuntu, from the user's session:
/absolute/path/to/freelane-guest-agent
# Host:
target/debug/freelane agent-ping /absolute/path/to/ubuntu/vm.toml
target/debug/freelane launch /absolute/path/to/ubuntu/vm.toml /usr/bin/gedit '/home/user/Documents/a & b.txt'
```

Choose an application actually installed in the guest. `/usr/bin/gedit` is an example, not an included dependency. Console stdin/stdout/stderr are disconnected from RPC; terminal sessions are not streamed. A PID means process creation, not verified app behavior. Inspect the real window and never automatically repeat an uncertain launch. Save and reopen a file, restart the app, reboot and re-check data as separate acceptance steps.

## Windows and Ubuntu in one MCP server

Copy [mcp.multi.example.toml](../configs/mcp.multi.example.toml) to `configs/mcp.multi.local.toml`, replace both VM paths with existing configs, and generate client settings as described in [mcp.md](mcp.md). Use `vm="windows"` or `vm="ubuntu"` with the same tools. `guest_launch` reads that registered config's `guest_os`: Windows requires an absolute `.exe`; Ubuntu requires an absolute Linux path. Missing `guest_os` still means Windows. `dev_run` now includes `build_linux_agent`.

The existing cloud-only example is unchanged and does not require a nonexistent Ubuntu VM. Register Ubuntu only after its VM config exists. Neither registering nor changing a client configuration starts a guest automatically.

## Verification and remaining gates

The Linux host's real agent `--stdio` test launches an executable, observes its actual written file, preserves literal spaces/shell metacharacters, prevents child output corrupting RPC and verifies child reaping. This is Linux host evidence, not an ARM Ubuntu VM transport test. The static ARM64 agent was built and inspected as a statically linked AArch64 ELF; runtime on Ubuntu ARM64 remains unverified.

The new Ubuntu profile executed the local EFI installer fixture through the real QEMU backend, with a private `ubuntu.qcow2`, no TPM attachment and a read-only installer. That validates profile/device wiring; it does **not** boot Ubuntu's kernel. The existing Windows/UEFI regression suite remains separate.

The current cloud proxy denied CONNECT to `cloud-images.ubuntu.com` with HTTP 403. Both that host and `cdimage.ubuntu.com` have been added to the saved draft's allowed domains, preserving prior hosts. Saving the draft does not apply network settings. An official Ubuntu image download, installed Ubuntu boot, virtual serial-agent runtime, networking, graphical apps, persistence and Mac/HVF are pending until those prerequisites can be exercised. Do not count a firmware screenshot as an Ubuntu desktop.
