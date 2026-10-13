# VM creation and persisted library

The manager now has **Create virtual machine / Tạo máy ảo mới**. Select Windows or Ubuntu, a new directory, installer media, firmware code and a distinct variable template, then choose CPU/RAM/disk size and HVF or TCG explicitly. Use Browse / Chọn… to select files and the parent folder for a new workspace. The folder picker appends `FreeLane VM`; edit the destination name before creating. macOS uses its native dialog; Linux requires zenity or manual path entry. Cancellation preserves the form. Confirm the media only after checking its origin yourself.

![Creation form on Linux Xvfb](assets/vm-create-wizard-en.png)

This is the first P1 creation path: it creates a sparse disk, private firmware/NVRAM/TPM state and a configuration. It does not install an OS, start a VM or infer that a guest is ready. The wizard verifies the PE architecture of `EFI/BOOT/BOOTAA64.EFI` before writing. ISO9660 is inspected directly; UDF/bridge media requires 7-Zip (`brew install sevenzip` on macOS, `7zz` or `7z` in PATH). Inspection is bounded to an 8 MiB loader and 30 seconds for the external reader. This verifies the bootloader only, not the entire installer, authenticity or successful installation. Cloud disk images, guided installation and setup recovery are still outstanding. QEMU/firmware/media must be supplied separately. Before writing, creation checks the nearest existing destination filesystem for the full virtual disk size, firmware/NVRAM and 64 MiB of headroom. This conservative check prevents knowingly overcommitting sparse disks; it does not reserve space against other writers. Checkpoints are available separately through the [offline checkpoint CLI](checkpoints.md).

The CLI equivalent uses [the creation-request example](../configs/create.windows.example.toml):

```sh
freelane media-check /absolute/path/to/installer.iso
freelane vm-create /absolute/path/to/create-request.toml
freelane doctor-json /absolute/path/to/new-vm/vm.toml
```

Relative file paths in a request resolve against its directory; qemu-img may be a PATH executable or an explicit path. Windows requests require at least two CPUs, 4096 MiB RAM and 64 GiB disk. Ubuntu disks may start at 8 GiB. Existing VM directories and source firmware/templates are preserved. Backend failures after directory creation retain the incomplete directory for inspection; do not retry with that same path or delete user data automatically.

## Library

Opening/creating a valid VM saves a bookmark. The manager reloads saved bookmarks at startup, including missing paths so they can be repaired explicitly. A settings rename updates the bookmark. Saving uses a private directory, 0600 file, lock, read/merge and atomic file replacement. It does not copy or modify guest disks/configurations; corrupt or unsupported library files are preserved and reported rather than overwritten. Runtime readiness is still queried independently.

Default paths:

- macOS: `~/Library/Application Support/FreeLane/library.toml`.
- Linux: `$XDG_DATA_HOME/freelane/library.toml`, or `~/.local/share/freelane/library.toml`.
- Tests/portable overrides: absolute `FREELANE_LIBRARY_PATH` in a private directory.

![Saved Ubuntu bookmark after restarting the actual manager without arguments](assets/vm-library-reopened.png)

Recorded checks before the conservative storage gate was introduced: real qemu-img created a 64 GiB virtual Windows disk, preflight passed, repeat creation refused overwrite and original NVRAM template bytes stayed unchanged. Actual Linux UI saved an Ubuntu bookmark and reloaded it after restart without configuration arguments. No new guest boot or installation was performed. Physical-Mac wizard/runtime acceptance is pending.
