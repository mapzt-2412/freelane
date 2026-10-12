# Dependency decisions

This is an engineering baseline, not a license approval for a packaged release. FreeLane's own code is MIT licensed; this does not change the licenses of external components. No Windows images, proprietary drivers or games are bundled.

| Component | Role and platform | License/maintenance considerations | Windows limitations |
| --- | --- | --- | --- |
| QEMU 10.0+ | External VMM process; macOS ARM64 HVF, Linux TCG for tests | Actively maintained upstream. Primarily GPL-2.0 with component-specific terms; consult the installed package copyright notices and preserve source/notice obligations when redistributing. Invoked as a subprocess rather than linked into Rust. | Windows ARM installation still needs firmware and compatible devices/drivers. No accelerated Windows DirectX path provided here. |
| EDK2/AAVMF | Separately installed ARM UEFI firmware | Maintained TianoCore/Debian package. Primarily BSD-2-Clause-Patent with other component licenses; Debian's package copyright lists BSD, OpenSSL, Apache and GPL components. A full artifact-specific review is required before bundling. | Secure Boot configuration, keys and Windows compatibility must be verified; an initialized display is insufficient. |
| swtpm/libtpms | External TPM 2.0 emulator with persistent per-VM state | Maintained upstream. swtpm includes BSD-3-Clause and CPL-1.0 components; consult package notices for libtpms and its dependencies. | Attachment is verified under TCG. Windows use, BitLocker recovery and coordinated snapshot/restore are unverified. |
| serde, serde_json, toml | Rust configuration/protocol serialization | MIT/Apache-2.0 upstream licensing; exact resolved versions retained in Cargo.lock. | No OS-specific guest capability. |
| eframe/egui, image, tempfile | Native desktop UI, PPM decoding and private temporary frames | MIT/Apache-2.0 upstream; locked transitive dependencies include platform/rendering components with their own notices. Audit actual artifacts before release. | The UI presents snapshots, not a virtual GPU or individual guest windows. |
| LLVM-MinGW | Build-time ARM64 Windows cross-linker | Pinned upstream release and SHA-256; distribution and runtime notices included by the build script. Compiler/runtime licenses do not change FreeLane's MIT license. | An ARM64 binary was linked and run in WinPE and the installed Audit session; virtio drivers remain separate. |
| getrandom | Fresh nonzero host request IDs across processes | MIT/Apache-2.0; platform OS randomness, version locked. | Correlation IDs are not authentication or a driver capability. |
| fs2 | Host-side file locks | MIT/Apache-2.0. Small older crate; failures are propagated and real concurrent-launch tests exercise its use. | Host locks do not substitute for guest filesystem consistency. |
| ctrlc | SIGINT/SIGTERM/SIGHUP cleanup | MIT/Apache-2.0. Platform dependencies retained in Cargo.lock. | A forced host stop does not perform a guest shutdown. |

Sources: upstream QEMU documentation at https://www.qemu.org/docs/master/system/arm/virt.html and installed Debian `copyright` files for `qemu-system-arm`, `qemu-efi-aarch64`, `swtpm` and `libtpms0`. Signed Debian repository metadata and package checksums were used for the Linux installation. TLS, signature and checksum verification were not disabled.
