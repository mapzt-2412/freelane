# Host files and virtual hardware

Open **VM settings** in the desktop manager, enter a host folder path, then save while the VM is shut down. On the next start, QEMU exposes that folder as a removable, read-only USB storage device containing a FAT16 filesystem. This uses ordinary USB storage rather than a special shared-folder guest agent.

The equivalent TOML setting is optional and off by default:

```toml
[runtime]
shared_directory = "/absolute/path/to/files-to-share"
```

Add this field to your existing `[runtime]` table; keep its other required fields. Relative paths resolve from the VM configuration directory. Removing the field or clearing the GUI textbox disables the share. The source must be an existing directory.

Use a small dedicated transfer folder with total content comfortably below the virtual volume's approximately **504 MiB** capacity. QEMU's virtual FAT backend has filesystem/name constraints and may reject incompatible contents at startup. It is not suitable for large installers, disk images or a whole home directory. Do not modify the folder or its files while the VM is running; shut down and restart to expose a changed set of files. Guest writes are disabled; copy files from the volume into the guest's own disk to edit them.

Windows should see the device through its USB storage driver and File Explorer. Ubuntu can inspect it with `lsblk -f` and mount its partition read-only, using the device identified by that command (device names are not fixed). A desktop environment may mount removable media automatically. Mounting and reading the volume inside installed Windows/Ubuntu have not yet been verified.

Validation currently covers real QEMU attachment during Windows-profile and Ubuntu-profile EFI boot tests, QMP reporting the volume as removable/read-only, unchanged host fixture bytes, and extracting the exact fixture text from the virtual FAT image with mtools. These checks do not establish installed guest automount or filesystem compatibility for arbitrary host folders.

The installer textbox separately attaches an existing ISO as a read-only CD device. Clearing it detaches that media on the next start; this also affects NoCloud seed media if that is your configured installer path.

CPU count and RAM are guest resource allocations. NAT networking, network adapter selection, display and guest-agent settings control existing virtual devices. FreeLane does not currently forward physical USB devices, share the host GPU, synchronize folders in both directions, or share the clipboard. These require separate backend and guest integration work.
