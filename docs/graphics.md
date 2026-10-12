# Virtual graphics: current implementation

FreeLane now has an opt-in **Virtio GPU 2D** device for Ubuntu ARM64. In Settings → Graphics, select Virtio GPU · 2D and save while the VM is shut down. The equivalent field in the existing runtime table is:

```toml
[runtime]
graphics = "virtio2d"
```

The default remains `graphics = "ramfb"` for backward compatibility. Windows profiles reject Virtio 2D because FreeLane has no validated Windows ARM GPU driver. Windows retains its existing basic display.

QEMU attaches `virtio-gpu-pci` with modern virtio PCI and keeps RAMFB for firmware compatibility. Ubuntu needs its `virtio_gpu` DRM driver; the virtual GPU supplies 2D resource/scanout operations. This implementation does **not** enable virgl, OpenGL, Vulkan, Metal, DirectX acceleration, physical GPU passthrough or a Windows graphics driver. A virtual GPU device alone is not a demonstration of faster apps or games.

## Real rendering evidence

The [recorded report](evidence/virtio-gpu-2d.json) comes from an actual ARM Linux guest under QEMU TCG, using the verified Ubuntu cloud image's 6.8.0-142 kernel and matching modules. A static Rust initramfs fixture:

1. Loads `virtio_dma_buf` and `virtio-gpu`, then opens the real DRM device.
2. Queries connector modes, allocates/maps a DRM dumb buffer and draws three colored regions.
3. Adds a framebuffer and sets the guest CRTC/scanout.
4. Lets the host capture QEMU's default display, with the firmware RAMFB device still attached.

The host checked the actual 1280 × 800 frame's pixel samples and hash. The following PNG is a lossless encoding of that QEMU PPM output, not a mockup or a screenshot of a complete Ubuntu desktop:

![Actual guest-rendered Virtio GPU diagnostic scanout](assets/virtio-gpu-scanout.png)

A separate real Ubuntu-profile EFI test also booted with the Virtio GPU device. The redesigned desktop manager started a disposable Ubuntu-profile EFI VM, paused/resumed it with independently checked QMP states, and captured its actual firmware shell. These tests do not establish GNOME/compositor behavior, Windows GPU support or native Mac performance.

## Reproduce the opt-in Linux test

Provide a trusted ARM64 kernel and its matching, uncompressed `virtio_dma_buf.ko` and `virtio-gpu.ko` modules. Kernel/module inputs are not bundled into a release.

```sh
cargo build --locked --workspace
rustup target add aarch64-unknown-linux-musl
python3 tests/gpu_2d_real.py --kernel /path/to/kernel --modules /path/to/modules --output /path/to/report.json --frame /path/to/new-frame.ppm
```

The fixture obtains its GPU option from FreeLane's production planner. It uses an isolated initramfs with no installed guest disk. The test stops only its own QEMU process after capture. Existing frame paths are refused. The source image's checksum/signature provenance is recorded in [Ubuntu documentation](ubuntu.md); the report records kernel, module, fixture and frame hashes.

## Performance and next gates

The manager remains a snapshot viewer with optional five-second refresh; it is not a video streaming client. Use QEMU's native guest window for keyboard input and interactive use. No FPS, latency or advantage over another VM product has been measured.

Native Apple Silicon validation, Ubuntu desktop/compositor workloads, and a viable Windows ARM driver/rendering path are still required. A Metal/3D backend is separate engineering work requiring a real Mac and repeatable application/graphics tests. The current backend has no Apple GPU assignment path and does not silently substitute software rendering for a promised hardware acceleration mode.
