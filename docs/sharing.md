# Host files and physical USB

VM settings now offers three folder-sharing modes and explicit USB device selectors. Changes apply on the next start; save while the VM is shut down. Clearing the folder path disables its share. Relative paths resolve from the configuration directory.

## Read-only USB folder: Windows and Ubuntu

This remains the default and preserves existing configurations. QEMU exposes the folder as a removable, read-only FAT16 USB storage device, without a special guest sharing agent.

```toml
[runtime]
shared_directory = "/absolute/path/to/files-to-share"
share_mode = "readonly_usb"
```

Add these fields to your existing runtime table. Use a dedicated folder with content comfortably below the approximately 504 MiB virtual volume capacity. QEMU's virtual FAT backend has filesystem/name constraints and may reject incompatible contents at startup. Keep source files unchanged while the VM runs; restart to expose changed content. Guest writes are disabled. Copy files into the guest disk to edit them.

Windows should see ordinary USB storage in File Explorer. Ubuntu can identify its partition with `lsblk -f` and mount it read-only. Installed guest automount remains unverified. Real Windows-profile and Ubuntu-profile EFI tests attached this volume, QMP reported it as removable/read-only, and mtools extracted the exact fixture text from its FAT image.

## Live read-only folder: Ubuntu

Select **Live read-only · Ubuntu**, or set `share_mode = "virtio9p_readonly"`. QEMU enforces `readonly=on` at its filesystem backend, so even a writable guest mount and guest root cannot modify the host share. Host changes remain visible. Unlike USB FAT media, this is a live mount and has no504MiB FAT-volume limit.

```sh
sudo mkdir -p /mnt/freelane
sudo mount -t 9p -o ro,trans=virtio,version=9p2000.L,cache=none freelane /mnt/freelane
```

Guest mount permissions/driver setup remain explicit; FreeLane does not grant guest sudo access. Read-only9p is Ubuntu-only, like read/write9p.

## Folder availability and reconnect

Files settings includes Choose shared folder and Check folder. Checks inspect the current typed path and configured mode asynchronously without creating directories or touching contents. Status means host availability, not an attached device or a successful guest mount. Relative paths use the VM configuration directory.

```sh
freelane share-status /path/vm.toml
```

The JSON always marks `guest_mount` unknown; guest mount telemetry remains pending. If a folder/drive disappears, reconnect it or explicitly choose a replacement. Shut down the VM before saving; settings apply on next start. On Mac, both the absolute selected `/Volumes` path and its canonical target are validated, refusing ordinary internal-disk fallbacks or a missing-volume path replaced by an internal symlink. Actual external-drive detach/reconnect on Mac remains unverified.

After importing settings, click Check folder again: cached availability can otherwise refer to the previous selection. The current Ubuntu-only validation error wording still says read/write for both9p modes.

## Live read/write folder: Ubuntu

Select **Live read/write · Ubuntu** in Settings:

```toml
[runtime]
shared_directory = "/absolute/path/to/shared-work"
share_mode = "virtio9p"
```

This exports a live virtio-9p filesystem. Both sides access the same files; guest edits change host files immediately. This is not a background copying/synchronization service and does not resolve concurrent editing conflicts. It does not use the FAT volume or its capacity limit. Use a dedicated folder; the host filesystem must support the xattrs required by QEMU's `mapped-xattr` security model.

Inside Ubuntu, create and mount the share:

```sh
sudo mkdir -p /mnt/freelane
sudo mount -t 9p -o trans=virtio,version=9p2000.L,cache=none freelane /mnt/freelane
```

Guest and host user IDs may differ, especially on macOS. If the guest user cannot write, set the guest-visible owner through the mounted filesystem:

```sh
sudo chown "$(id -u):$(id -g)" /mnt/freelane
```

The mapped-xattr backend stores guest ownership in extended attributes while host filesystem operations use the QEMU host account. Guest root provisioning/mount permissions must be available; FreeLane does not silently grant its agent sudo access. The Ubuntu cloud lab's agent account has only narrow lifecycle sudo permissions, so this mount requires a provisioning change or an authorized administrative session. Unmount before guest shutdown if necessary. File contents are ordinary host files; POSIX guest metadata is represented in xattrs.

Windows profiles reject `virtio9p`: this project does not have a compatible Windows ARM 9p sharing driver. Two-way Windows sharing remains outstanding and needs a separate supported backend, such as an authenticated SMB or virtio-fs integration. Existing read-only transfer is unchanged.

### Actual read/write verification

[The committed report](evidence/9p-bidirectional.json) records a real ARM64 Linux initramfs using Ubuntu's 6.8.0-142 kernel and matching 9p modules extracted from the previously verified Ubuntu cloud image. A static Rust fixture mounts the device, starts a worker with kernel-confirmed UID 1000, and tests:

- Host file read by guest and a subsequent host edit observed without reboot.
- Guest-created file read by host with exact bytes.
- Guest rename/delete reflected in the host directory.
- Native guest poweroff after completion.

The test obtains filesystem/device arguments from FreeLane's production planner, including a comma and option-like text in the host path. It is not a full installed Ubuntu session or a native Mac test. Reproduce with a trusted ARM64 kernel and its matching uncompressed modules (`netfs.ko`, `9pnet.ko`, `9pnet_virtio.ko`, `9p.ko`):

```sh
cargo build --locked --workspace
rustup target add aarch64-unknown-linux-musl
python3 tests/share_9p_real.py --kernel /path/to/kernel --modules /path/to/modules --output /path/to/result.json
```

The source image hash and signature verification are documented in [Ubuntu evidence](ubuntu.md). The report records kernel/module hashes. Fixtures and kernel inputs are not bundled into a release; the test is opt-in and requires QEMU. No signing or TLS verification was disabled to obtain its inputs.

## Physical USB device forwarding

Settings accepts bus/address and hexadecimal vendor/product IDs, creating exact QEMU `usb-host` selectors. Inspect devices on the actual host first:

```sh
python3 scripts/list-usb.py
# Optional if libusb is outside its normal search path:
python3 scripts/list-usb.py --library /path/to/libusb-1.0.dylib
```

The helper uses libusb without claiming, detaching or resetting devices. QEMU must also include libusb/usb-host support. On Mac, obtain QEMU/libusb from a trusted package manager; on Linux, give your user access to the specific USB device using your distribution's normal device rules. FreeLane does not change device permissions or run QEMU as root.

Example selectors (replace all values with an actual inventory entry):

```toml
[[runtime.usb_devices]]
bus = 2
address = 3
vendor_id = 0x1234
product_id = 0xabcd
```

IDs default to no attached devices; GUI entries with an unset vendor ID cannot be saved. Duplicate bus/address pairs, wildcard bus/address values and more than 16 selectors are rejected. QEMU requires all configured selectors to match. A device captured by QEMU is unavailable to host applications until released. Addresses may change after reconnection, so inspect again before starting. Guest device drivers and host access are required. A configured selector is not evidence that hardware was attached; a missing device may be awaited by QEMU.

The local libusb inventory returned an empty list when run with host inspection access: this cloud has no USB hardware to test. Argument validation and persistence are tested, but physical device transfer, unplug/replug, and native Mac behavior remain unverified. Do not infer successful hardware forwarding from a VM start response.

## GPU boundary

GPU passthrough is not implemented. The current Apple Silicon QEMU/HVF backend has no host GPU assignment path; HVF accelerates guest CPUs. QEMU's Linux VFIO PCI assignment requires a suitable Linux host, IOMMU setup and assignable hardware, and cannot be used as a macOS Apple GPU setting. The cloud machine supplies neither an Apple Silicon Mac nor an assignable test GPU.

A virtual GPU and graphics translation path is separate engineering work, including Windows ARM guest-driver support. FreeLane does not label its basic display, HVF, or USB forwarding as GPU passthrough.

The installer textbox separately attaches an ISO/NoCloud seed as read-only CD media. Clipboard and seamless application windows remain separate integration work.

## Read-only backend acceptance

[Read-only ARM Linux evidence](evidence/9p-readonly.json) requests a writable9p mount, then attempts create/overwrite/rename/delete as guest root. Every mutation was refused; a host update remained readable and native guest poweroff completed. Existing read/write create/rename/delete acceptance was rerun too. This is a disposable kernel/initramfs fixture, not installed Ubuntu or native Mac acceptance.

```sh
python3 tests/share_9p_real.py --readonly --kernel /trusted/kernel --modules /trusted/modules --output /path/readonly-result.json
```
