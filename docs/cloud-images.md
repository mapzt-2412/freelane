# Ubuntu cloud images

The VM wizard supports **Ubuntu cloud image** alongside the existing ISO path. Choose an official ARM64 Ubuntu **qcow2** image, confirm its architecture, select firmware/NVRAM templates and choose resources. The source is read only; FreeLane converts a private snapshot into an independent VM disk and grows it to the requested size. It does not download images or silently replace an existing workspace.

Use an official source such as <https://cloud-images.ubuntu.com/noble/current/>. Verify the ARM64 image against the signed `SHA256SUMS` before use. A qcow2 header describes storage, not CPU architecture or image authenticity: FreeLane validates format and capacity, while ARM64 confirmation remains the user's responsibility. It rejects backing chains, external-data/encrypted/dirty images and unsupported qcow2 features. The input file is limited to 16 GiB; each QEMU conversion/resize process has a five-minute deadline. Cloud import budgets source virtual capacity, the temporary source-file copy and metadata. Enlarging the target virtual maximum does not reserve that maximum. Available-space checks are not reservations. See [storage setup](setup.md).

A cloud image already contains Ubuntu Server. Creating the workspace does not establish first-boot completion, a usable login, a guest agent or Ubuntu Desktop. Supply an optional **NoCloud seed ISO** to configure your account/SSH key and networking. Without a suitable seed, the default image may have no usable login. The seed must be a nonempty regular file ≤64 MiB; ISO structure and seed contents are currently unverified. Its external path must remain accessible until provisioning completes.

Example request, usable with `freelane vm-create /absolute/path/create.toml`:

```toml
name = "Ubuntu cloud"
guest_os = "ubuntu"
source = "ubuntu_cloud_image"
directory = "new-ubuntu-vm"
installer = "noble-server-cloudimg-arm64.img"
cloud_init_iso = "seed.iso" # optional; user-created NoCloud media
firmware = "/absolute/path/AAVMF_CODE.fd"
variable_template = "/absolute/path/AAVMF_VARS.fd"
qemu_img = "qemu-img"
accelerator = "hvf" # Apple Silicon; explicitly use tcg for portable Linux tests
cpus = 2
memory_mib = 2048
disk_gib = 16
arm64_media_confirmed = true
network = true
guest_agent = false
```

Paths resolve beside the request. Existing requests omit `source` and retain ISO behavior. Guest-agent channel configuration does not install an agent.

For a user-created NoCloud seed, include `meta-data`, `user-data` and optionally `network-config` in an ISO volume labeled `cidata`. Configure authentication explicitly using your public SSH key; never put a private SSH key in a seed. NoCloud can execute privileged first-boot commands, so use seed contents you intend to run. The application does not inspect those commands.

An explicit `network-config` for QEMU's default Virtio interface can look like:

```yaml
version: 2
ethernets:
  ethernet:
    match:
      name: "en*"
    dhcp4: true
    dhcp6: false
    optional: true
```

Adapt it to the guest. A disabled/misconfigured NIC may leave cloud-init waiting for networking; QMP `running` does not mean provisioning finished. `vm-readiness` intentionally keeps installation completion unverified. Virtio networking for disk/ISO boot disables the optional PXE ROM; PXE/network boot is not provided by this flow.


## Repeatable guest test

After separately verifying the signed ARM64 manifest, run the opt-in test with the expected checksum and local firmware paths:

```sh
python3 tests/cloud_guest_acceptance.py --media /absolute/path/noble-server-cloudimg-arm64.img --sha256 EXPECTED_SHA256 --firmware /absolute/path/AAVMF_CODE.fd --variables /absolute/path/AAVMF_VARS.fd
```

It creates and retains a fresh private test workspace, uses a fixed NoCloud test seed with explicit networking, observes architecture/OS inside the guest, writes a random marker, checks its exact contents after reboot and waits for clean shutdowns. It neither modifies the supplied source image nor deletes a user VM. Requires `qemu-img`, `qemu-system-aarch64`, `xorriso` and a built host CLI. Default acceleration is explicit TCG; this does not establish native Mac acceptance. The five-minute conversion bound and per-boot test timeout are separate.
