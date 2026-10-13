# VM creation and persisted library

The manager now has **Create virtual machine / Tạo máy ảo mới**. Select Windows or Ubuntu, a new directory, installer media, firmware code and a distinct variable template, then choose CPU/RAM/disk size and HVF or TCG explicitly. Small/Standard/Large select visible resource values; they are allocation presets, not measured performance claims. Enable networking and the guest-agent channel independently, and optionally select a host folder (read-only USB, or read/write 9p for Ubuntu). Use Browse / Chọn… to select files and the parent folder for a new workspace. The folder picker appends `FreeLane VM`; edit the destination name before creating. macOS uses its native dialog; Linux requires zenity or manual path entry. Cancellation preserves the form. Confirm the media only after checking its origin yourself.

![Creation form on Linux Xvfb](assets/vm-create-wizard-en.png)

This is the first P1 creation path: it creates a sparse disk, private firmware/NVRAM/TPM state and a configuration. It does not install an OS, start a VM or infer that a guest is ready. The wizard verifies the PE architecture of `EFI/BOOT/BOOTAA64.EFI` before writing. ISO9660 is inspected directly; UDF/bridge media requires 7-Zip (`brew install sevenzip` on macOS, `7zz` or `7z` in PATH). Inspection is bounded to an 8 MiB loader and 30 seconds for the external reader. This verifies the bootloader only, not the entire installer, authenticity or successful installation. Cloud disk images, guided installation and setup recovery are still outstanding. QEMU/firmware/media must be supplied separately. Before writing, creation checks the nearest existing destination filesystem for the full virtual disk size, firmware/NVRAM and 64 MiB of headroom. This conservative check prevents knowingly overcommitting sparse disks; it does not reserve space against other writers. Checkpoints are available separately through the [offline checkpoint CLI](checkpoints.md).

The CLI equivalent uses [the creation-request example](../configs/create.windows.example.toml):

```sh
freelane media-check /absolute/path/to/installer.iso
freelane vm-create /absolute/path/to/create-request.toml
freelane doctor-json /absolute/path/to/new-vm/vm.toml
```

Relative file paths in a request resolve against its directory; qemu-img may be a PATH executable or an explicit path. Windows requests require at least two CPUs, 4096 MiB RAM and 64 GiB disk. Ubuntu disks may start at 8 GiB. Existing VM directories and source firmware/templates are preserved. Creation prepares a private `.freelane-create-*.pending` directory beside the destination, then atomically publishes without replacing anything. The parent must already exist, be owned by you and have no group/world write access. An incomplete creation marker prevents backend startup. Failures retain the pending directory for inspection, never erase source templates or an existing destination.

## Library

Opening/creating a valid VM saves a bookmark. The manager reloads saved bookmarks at startup, including missing paths so they can be repaired explicitly. A settings rename updates the bookmark. Saving uses a private directory, 0600 file, lock, read/merge and atomic file replacement. It does not copy or modify guest disks/configurations; corrupt or unsupported library files are preserved and reported rather than overwritten. Runtime readiness is still queried independently.

Default paths:

- macOS: `~/Library/Application Support/FreeLane/library.toml`.
- Linux: `$XDG_DATA_HOME/freelane/library.toml`, or `~/.local/share/freelane/library.toml`.
- Tests/portable overrides: absolute `FREELANE_LIBRARY_PATH` in a private directory.

![Saved Ubuntu bookmark after restarting the actual manager without arguments](assets/vm-library-reopened.png)

Recorded checks before the conservative storage gate was introduced: real qemu-img created a 64 GiB virtual Windows disk, preflight passed, repeat creation refused overwrite and original NVRAM template bytes stayed unchanged. Actual Linux UI saved an Ubuntu bookmark and reloaded it after restart without configuration arguments. No new guest boot or installation was performed. Physical-Mac wizard/runtime acceptance is pending.

## Observed runtime state

`freelane vm-readiness /absolute/path/to/vm.toml` queries QMP and, only while the backend is observed running and its agent channel is enabled, sends a bounded agent ping. It reports backend and agent independently. Missing/unreachable QMP is `unknown`, never assumed stopped or installed. A running backend or responding agent does not prove installation completion; `installation_complete` remains null. JSON command success means observation completed, not guest acceptance. Desktop Refresh status uses the same observer; Details shows the agent state.

## Interrupted creation

A successfully prepared workspace has a versioned creation marker with SHA-256 digests for configuration, disk, firmware and NVRAM. If publication fails (for example another process creates the destination), recover it into a fresh directory:

```sh
freelane creation-recover /absolute/path/to/.freelane-create-ID.pending /absolute/path/to/recovered-vm
```

Recovery validates private ownership, the manifest, contained regular files/checksums, empty initial TPM state and the device plan before an atomic no-replace move. This is completion of a never-started workspace, not guest recovery or OS-installation resumption. A changed/missing component, existing destination, unsupported manifest or earlier `creating` stage is refused with the source preserved. For early failures, keep the pending files for inspection and retry creation with a new destination. The command returns JSON: `published` records committed publication, `startup_allowed` records successful marker removal, and `warning` reports a retained-marker finalization problem. Check both fields before starting. Pre-publication failures leave the source in place; an unexpected finalization failure after publication returns the committed destination with startup protected. Read-only source directories are refused before publication. Cross-filesystem moves are refused rather than silently copying. Do not remove markers or run QEMU directly against pending directories. Power-loss durability and automatic cleanup of abandoned workspaces are not guaranteed.

## Creation progress and cloud UI evidence

The wizard displays actual completed/active boundaries: input validation, media inspection, workspace preparation, configuration, file verification and publication. There is no estimated percentage or fabricated installation progress. Failed validation never emits completion. Workspace completion does not mean the OS is installed.

The actual Linux UI created a new Ubuntu workspace from the official Ubuntu 24.04.5 ARM64 installer after Ubuntu's checksum signature and ISO SHA-256 were verified. The Small preset produced 2 CPUs, 2048 MiB RAM and a 16 GiB sparse qcow2; network and agent-channel choices persisted, source firmware/variable templates matched the copies and a bookmark was saved. The reopened manager loaded this configuration. No guest was started or OS installed during this creation test.

![Created Ubuntu workspace in English](assets/wizard-created-ubuntu-en.png)

[Evidence](evidence/wizard-options.json) records these limits. Physical-Mac dialog and guest installation acceptance remain deferred.
