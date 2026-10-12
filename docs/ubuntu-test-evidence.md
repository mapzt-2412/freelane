# Installed Ubuntu Server ARM64 evidence

On 2026-10-12, Canonical's Ubuntu Server 24.04.5 LTS ARM64 cloud image was deployed into FreeLane's separate Ubuntu VM and booted with kernel `6.8.0-142-generic`. This is deployment of a preinstalled cloud image with NoCloud first-boot provisioning, not execution of the interactive desktop/ISO installer.

## Image and VM

- Image: `https://cloud-images.ubuntu.com/noble/current/noble-server-cloudimg-arm64.img`, 620224512 bytes.
- SHA-256: `1d6bffe64b848468ac97f821d369a4846d983de1800ccf6b5ec8853e85cefc55`, matching Canonical's SHA256SUMS.
- Detached manifest signature returned OpenPGP VALIDSIG for `D2EB44626FDDC30B513D5BB71A5D6C4C7DB87C81`. The cloud-image public keyring was extracted from `ubuntu-keyring_2023.11.28.1_all.deb` obtained over verified HTTPS from Ubuntu's official archive. Key trust was bootstrapped through that publisher's HTTPS endpoint; no web-of-trust certification is claimed.
- QEMU 10.0.13, `virt-10.0`, TCG, 4 CPUs/4096 MiB, modern virtio block storage, private matching generic AAVMF code/NVRAM, no TPM and networking disabled.
- The image was converted offline to a standalone qcow2 disk and expanded to 64 GiB. Guest-reported root filesystem capacity was 65445814272 bytes after cloud-init growth. Windows state was not reused or modified.

Generic firmware initially failed to enumerate the NVMe boot disk. The same Ubuntu disk booted with modern virtio block storage; new Ubuntu initialization now selects that device. Legacy/Windows configs keep their NVMe default. This observed workaround does not identify every firmware/NVMe compatibility issue.

## Actual console capture

![Ubuntu Server 24.04.5 ARM64 login console from the real VM](assets/ubuntu-server-login.png)

The PNG is an unchanged MCP capture of the real basic console. It does not depict a graphical Ubuntu desktop or a native Mac run. PNG SHA-256: `ce7f5b00bd2478803aab50df63520ff78c2e52d257991a329ff88609382e6d85`.

## Guest account and transport

NoCloud created a password-locked `freelane` test user, UID/GID 1000, with a dedicated virtio-port group rule. A private serial-console autologin started the static ARM64 agent from that user's profile. It ran as an ordinary user, not a privileged agent service. Only native `systemctl reboot` and `poweroff` were delegated through narrowly scoped sudoers rules for lifecycle acceptance. Networking had no interface/forwarded host port.

Early ping requests timed out. After a normal console session was connected and observed, fresh virtio requests returned correlated Linux/aarch64 replies and actual application results. Require those observations after startup; a shell/agent PID or old readiness banner is insufficient. The opt-in UART socket is inside the VM's 0700 runtime directory and was held open during the acceptance run.

## Saved data and native reboot

The opt-in [acceptance driver](../tests/ubuntu_guest_acceptance.py) uses real MCP, pings the registered guest, and sends exactly one explicit `/usr/bin/python3` launch with literal arguments. It observes a nonce-correlated guest result through UART, independently verifies the payload's SHA-256 and records the actual launch/ping replies. It does not automatically retry an uncertain launch.

The normal user wrote UTF-8 Vietnamese text to `/home/freelane/FreeLane a & b.txt`, fsynced/read it, copied it, round-tripped gzip and created/read/CRC-tested a ZIP. The path containing spaces and `&` was passed as a separate literal argument. After native reboot, the second phase read the existing original/copy/archive without recreating them.

| Observation | Baseline | After native reboot |
| --- | --- | --- |
| Guest OS/architecture | Ubuntu 24.04.5 / aarch64 | Ubuntu 24.04.5 / aarch64 |
| Effective UID | 1000 | 1000 |
| Boot ID | `163775dc-9643-4a24-a439-c3c486a91551` | `2e6f511f-2ab5-454f-9c93-d06b7530e48d` |
| File SHA-256 | `551af47c618bbd3f6376895d859a8a65cdff4a73588668ede00355ff796ac137` | Same |
| Copy bytes / ZIP CRC | Passed | Passed |

Recorded machine-readable replies: [baseline](evidence/ubuntu-baseline.json), [after reboot](evidence/ubuntu-after-reboot.json). Those test nonces, process/request IDs and boot IDs identify this run; they are not authentication credentials.

Reproduce only against the intended Ubuntu test guest/account, after a successful actual ping and console connection:

```sh
python3 tests/ubuntu_guest_acceptance.py --config configs/mcp.multi.local.toml --serial-log /absolute/runtime/serial.log --phase first --output /new/path/baseline.json
# Request a native guest reboot separately, observe completion and agent readiness.
python3 tests/ubuntu_guest_acceptance.py --config configs/mcp.multi.local.toml --serial-log /absolute/runtime/serial.log --phase reboot --previous /new/path/baseline.json --output /new/path/after-reboot.json
```

MCP pause/status/resume changed the actual QMP state, and a fresh guest ping succeeded after resume. The final native poweroff reached Linux's “Power down”; the actual QMP socket disappeared afterward. No hard-stop was used for the installed Ubuntu shutdowns. Preserve disk, private firmware/NVRAM and provisioning media together before taking a stopped filesystem snapshot.

## Limits

These are finite headless workloads under Linux software emulation. Network readiness timed out with networking disabled; no guest-network functionality is claimed. Startup was slow, including processing the test's large NoCloud payload. Graphical Ubuntu apps, a normal desktop installer/user setup, accelerated graphics, Mac/HVF, long-term stability and Parallels performance comparisons remain unverified. No VM image, provisioning binary or third-party executable is bundled in this repository.
