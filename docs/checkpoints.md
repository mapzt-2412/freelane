# Offline checkpoint and recovery-copy foundation

The CLI can create a full-copy checkpoint of a **stopped FreeLane workspace** and restore its verified files into a **new directory**. This is a first P1/P4 foundation, not a complete snapshot/backup product or evidence of successful Windows/BitLocker recovery.

```sh
freelane checkpoint-create /absolute/path/to/vm.toml /absolute/path/to/new-checkpoint 'Before app test'
freelane checkpoint-restore /absolute/path/to/checkpoint /absolute/path/to/new-recovery-workspace
freelane doctor-json /absolute/path/to/new-recovery-workspace/vm.toml
```

Creation holds the managed VM's backend lock and refuses running/paused VMs. Stop other tools using these files as well; the managed lock does not govern external processes. Only clean standalone qcow2 disks are currently supported; backing chains, external-data images and shared/hard-linked source files are refused. Sources must be inside the VM workspace. External installer media is not copied.

The copy includes the logical guest disk, firmware, NVRAM, configuration and bounded TPM files. qemu-img converts/compares disk data; copied files have SHA256 hashes. Checkpoint folders are private, files read-only and a marker prevents FreeLane from booting them directly. A verified recovery copy removes that marker only after publication. It is not an independent clone with regenerated guest identities; do not run it alongside the original until identity/network implications are resolved.

Files are staged in an owned private directory, then published with atomic no-replace rename. Existing directories, including empty ones created during a race, are never intentionally replaced. Failed operations discard their owned staging data; abrupt process/power interruption can leave a protected `.pending` directory. Automatic interruption repair/cleanup, full power-loss recovery, live/memory snapshots, UI management, incremental backup, archive import/export and clone identity handling remain outstanding.

Creation checks destination capacity conservatively against virtual disk size plus firmware/state margin; it does not reserve disk space against other writers. Restore verifies file entries/checksums and rejects traversal, filename collisions, malformed manifests and existing output. It does not prove filesystem consistency, TPM sealing, driver compatibility or a successful guest boot. Run those acceptance checks before depending on the recovery copy.

## Verification scope

- Unit tests: active VM lock refusal, immutable startup marker, path traversal, overwrite protection and atomic publication against an existing empty directory.
- Real tool test: `python3 tests/checkpoint_real.py` uses an 8 MiB qcow2 file with known bytes, fake nonbootable firmware and fake TPM state. It checks qemu-img conversion/compare, qemu-io data, checksums, corruption/overwrite refusal, source preservation and fresh-workspace restore. No guest OS or real TPM recovery runs.
- CI runs the file test on Debian/Linux and macOS with real qemu-img/qemu-io tools. Native file operations do not establish physical-Mac guest acceptance.
- The existing cloud Windows disk cannot currently be copied: the cloud filesystem is about 32 GiB with less than 1 GiB free, while the conservative checkpoint needs over 64 GiB. The check refused the operation before creating output. Use a sufficiently large destination for real Windows/Ubuntu restore acceptance; do not delete the existing guests or installation media to force this test.
