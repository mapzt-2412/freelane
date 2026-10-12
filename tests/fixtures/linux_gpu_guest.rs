//! Static ARM64 DRM scanout fixture. Draws real virtio GPU pixels; no 3D claim.
use std::{
    ffi::{c_char, c_void},
    fs, io,
    os::fd::AsRawFd,
    time::Duration,
};
unsafe extern "C" {
    fn mount(
        source: *const c_char,
        target: *const c_char,
        kind: *const c_char,
        flags: usize,
        data: *const c_void,
    ) -> i32;
    fn init_module(bytes: *const c_void, length: usize, args: *const c_char) -> i32;
    fn ioctl(fd: i32, request: usize, arg: *mut c_void) -> i32;
    fn mmap(
        address: *mut c_void,
        length: usize,
        protection: i32,
        flags: i32,
        fd: i32,
        offset: i64,
    ) -> *mut c_void;
}
#[repr(C)]
#[derive(Default)]
struct Resources {
    fbs: u64,
    crtcs: u64,
    connectors: u64,
    encoders: u64,
    nfbs: u32,
    ncrtcs: u32,
    nconnectors: u32,
    nencoders: u32,
    min_width: u32,
    max_width: u32,
    min_height: u32,
    max_height: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Mode {
    clock: u32,
    hdisplay: u16,
    hsync_start: u16,
    hsync_end: u16,
    htotal: u16,
    hskew: u16,
    vdisplay: u16,
    vsync_start: u16,
    vsync_end: u16,
    vtotal: u16,
    vscan: u16,
    vrefresh: u32,
    flags: u32,
    kind: u32,
    name: [u8; 32],
}
#[repr(C)]
#[derive(Default)]
struct Connector {
    encoders: u64,
    modes: u64,
    props: u64,
    values: u64,
    nmodes: u32,
    nprops: u32,
    nencoders: u32,
    encoder_id: u32,
    id: u32,
    kind: u32,
    type_id: u32,
    connection: u32,
    width_mm: u32,
    height_mm: u32,
    subpixel: u32,
    pad: u32,
}
#[repr(C)]
#[derive(Default)]
struct Dumb {
    height: u32,
    width: u32,
    bpp: u32,
    flags: u32,
    handle: u32,
    pitch: u32,
    size: u64,
}
#[repr(C)]
#[derive(Default)]
struct Map {
    handle: u32,
    pad: u32,
    offset: u64,
}
#[repr(C)]
#[derive(Default)]
struct Framebuffer {
    id: u32,
    width: u32,
    height: u32,
    pitch: u32,
    bpp: u32,
    depth: u32,
    handle: u32,
}
#[repr(C)]
#[derive(Default)]
struct Crtc {
    connectors: u64,
    count: u32,
    id: u32,
    fb: u32,
    x: u32,
    y: u32,
    gamma: u32,
    valid: u32,
    mode: Mode,
}
fn request<T>(fd: i32, number: usize, argument: &mut T) {
    let code = 0xc0000000 | (std::mem::size_of::<T>() << 16) | (usize::from(b'd') << 8) | number;
    // SAFETY: Linux's DRM ioctl struct matches repr(C), and each pointer is valid through the call.
    assert_eq!(
        unsafe { ioctl(fd, code, std::ptr::from_mut(argument).cast()) },
        0,
        "DRM ioctl {number:x}: {}",
        io::Error::last_os_error()
    );
}
fn main() {
    fs::create_dir_all("/dev").unwrap();
    // SAFETY: strings are static, NUL-terminated; devtmpfs supplies actual DRM device nodes.
    assert_eq!(
        unsafe {
            mount(
                c"devtmpfs".as_ptr(),
                c"/dev".as_ptr(),
                c"devtmpfs".as_ptr(),
                0,
                std::ptr::null(),
            )
        },
        0
    );
    for name in ["virtio_dma_buf", "virtio-gpu"] {
        let bytes = fs::read(format!("/modules/{name}.ko")).unwrap();
        // SAFETY: module buffers are valid through the syscall.
        assert_eq!(
            unsafe { init_module(bytes.as_ptr().cast(), bytes.len(), c"".as_ptr()) },
            0,
            "load {name}: {}",
            io::Error::last_os_error()
        );
    }
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/dri/card0")
        .unwrap();
    let fd = file.as_raw_fd();
    let mut resources = Resources::default();
    request(fd, 0xa0, &mut resources);
    assert!(
        resources.ncrtcs > 0
            && resources.ncrtcs < 32
            && resources.nconnectors > 0
            && resources.nconnectors < 32
    );
    let mut crtcs = vec![0u32; resources.ncrtcs as usize];
    let mut connectors = vec![0u32; resources.nconnectors as usize];
    resources.nfbs = 0;
    resources.nencoders = 0;
    resources.crtcs = crtcs.as_mut_ptr() as u64;
    resources.connectors = connectors.as_mut_ptr() as u64;
    request(fd, 0xa0, &mut resources);
    let mut connector = Connector {
        id: connectors[0],
        ..Default::default()
    };
    request(fd, 0xa7, &mut connector);
    assert_eq!(connector.connection, 1);
    assert!(connector.nmodes > 0 && connector.nmodes < 256);
    let mut modes = vec![Mode::default(); connector.nmodes as usize];
    connector.nprops = 0;
    connector.nencoders = 0;
    connector.modes = modes.as_mut_ptr() as u64;
    request(fd, 0xa7, &mut connector);
    let mode = modes[0];
    let mut dumb = Dumb {
        width: mode.hdisplay.into(),
        height: mode.vdisplay.into(),
        bpp: 32,
        ..Default::default()
    };
    request(fd, 0xb2, &mut dumb);
    assert!(dumb.pitch >= dumb.width * 4 && dumb.pitch % 4 == 0);
    assert!(
        dumb.size <= 64 * 1024 * 1024
            && u64::from(dumb.pitch) * u64::from(dumb.height) <= dumb.size
    );
    let mut mapping = Map {
        handle: dumb.handle,
        ..Default::default()
    };
    request(fd, 0xb3, &mut mapping);
    // SAFETY: DRM reports the mapped size/offset; MAP_SHARED and PROT_READ|WRITE map that buffer.
    let pixels = unsafe {
        mmap(
            std::ptr::null_mut(),
            dumb.size as usize,
            3,
            1,
            fd,
            mapping.offset.try_into().unwrap(),
        )
    };
    assert_ne!(pixels as isize, -1);
    for y in 0..dumb.height {
        for x in 0..dumb.width {
            let color = if x < dumb.width / 3 {
                0x00ed5e65u32
            } else if x < 2 * dumb.width / 3 {
                0x005984ff
            } else {
                0x003ebc85
            };
            // SAFETY: every byte offset is within the validated DRM mapping; pitch is 32-bit pixel aligned.
            unsafe {
                pixels
                    .cast::<u8>()
                    .add((y * dumb.pitch + x * 4) as usize)
                    .cast::<u32>()
                    .write(color);
            }
        }
    }
    let mut fb = Framebuffer {
        width: dumb.width,
        height: dumb.height,
        pitch: dumb.pitch,
        bpp: 32,
        depth: 24,
        handle: dumb.handle,
        ..Default::default()
    };
    request(fd, 0xae, &mut fb);
    let mut crtc = Crtc {
        connectors: connectors.as_mut_ptr() as u64,
        count: 1,
        id: crtcs[0],
        fb: fb.id,
        valid: 1,
        mode,
        ..Default::default()
    };
    request(fd, 0xa2, &mut crtc);
    println!(
        "FREELANE_VIRTIO_GPU_SCANOUT_OK {} {}",
        dumb.width, dumb.height
    );
    loop {
        std::thread::sleep(Duration::from_secs(1));
    }
}
