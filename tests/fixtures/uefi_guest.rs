//! Freestanding ARM64 EFI test application. Not Windows or a Windows installer.
#![no_std]
#![no_main]
use core::ffi::c_void;

#[repr(C)]
struct Header {
    signature: u64,
    revision: u32,
    size: u32,
    crc32: u32,
    reserved: u32,
}
#[repr(C)]
struct SystemTable {
    header: Header,
    firmware_vendor: *mut u16,
    firmware_revision: u32,
    console_in_handle: *mut c_void,
    console_in: *mut c_void,
    console_out_handle: *mut c_void,
    console_out: *mut TextOutput,
}
#[repr(C)]
struct TextOutput {
    reset: usize,
    output_string: unsafe extern "efiapi" fn(*mut TextOutput, *const u16) -> usize,
}
#[no_mangle]
unsafe extern "efiapi" fn efi_main(_handle: *mut c_void, table: *mut SystemTable) -> usize {
    if table.is_null() {
        return 0x8000_0000_0000_0002;
    }
    // SAFETY: EFI firmware passes a valid SystemTable; repr(C) matches the UEFI ABI prefix.
    let console = unsafe { (*table).console_out };
    if console.is_null() {
        return 0x8000_0000_0000_0002;
    }
    let mut message = [0u16; 40];
    for (index, byte) in b"FREELANE_INSTALL_MEDIA_OK\r\n".iter().enumerate() {
        message[index] = u16::from(*byte);
    }
    // SAFETY: the UTF-16 buffer is NUL-terminated and lives through the firmware call.
    unsafe { ((*console).output_string)(console, message.as_ptr()) }
}
#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
