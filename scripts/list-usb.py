#!/usr/bin/env python3
"""List USB bus/address and vendor/product IDs using libusb without claiming devices."""
import argparse
import ctypes
import ctypes.util
import json
import sys


class Descriptor(ctypes.Structure):
    _fields_ = [
        ("length", ctypes.c_uint8), ("kind", ctypes.c_uint8),
        ("usb", ctypes.c_uint16), ("device_class", ctypes.c_uint8),
        ("subclass", ctypes.c_uint8), ("protocol", ctypes.c_uint8),
        ("packet_size", ctypes.c_uint8), ("vendor", ctypes.c_uint16),
        ("product", ctypes.c_uint16), ("release", ctypes.c_uint16),
        ("manufacturer", ctypes.c_uint8), ("product_string", ctypes.c_uint8),
        ("serial_string", ctypes.c_uint8), ("configurations", ctypes.c_uint8),
    ]


def inventory(library):
    usb = ctypes.CDLL(library)
    pointer = ctypes.c_void_p
    usb.libusb_init.argtypes = [ctypes.POINTER(pointer)]
    usb.libusb_init.restype = ctypes.c_int
    usb.libusb_exit.argtypes = [pointer]
    usb.libusb_exit.restype = None
    usb.libusb_get_device_list.argtypes = [pointer, ctypes.POINTER(ctypes.POINTER(pointer))]
    usb.libusb_get_device_list.restype = ctypes.c_ssize_t
    usb.libusb_free_device_list.argtypes = [ctypes.POINTER(pointer), ctypes.c_int]
    usb.libusb_free_device_list.restype = None
    usb.libusb_get_bus_number.argtypes = [pointer]
    usb.libusb_get_bus_number.restype = ctypes.c_uint8
    usb.libusb_get_device_address.argtypes = [pointer]
    usb.libusb_get_device_address.restype = ctypes.c_uint8
    usb.libusb_get_device_descriptor.argtypes = [pointer, ctypes.POINTER(Descriptor)]
    usb.libusb_get_device_descriptor.restype = ctypes.c_int
    context = pointer()
    code = usb.libusb_init(ctypes.byref(context))
    if code != 0:
        raise RuntimeError(f"libusb initialization failed ({code}); host USB access is unavailable")
    devices = ctypes.POINTER(pointer)()
    try:
        count = usb.libusb_get_device_list(context, ctypes.byref(devices))
        if count < 0:
            raise RuntimeError(f"libusb enumeration failed ({count})")
        try:
            result = []
            for index in range(count):
                descriptor = Descriptor()
                if usb.libusb_get_device_descriptor(devices[index], ctypes.byref(descriptor)) != 0:
                    continue
                result.append({"bus": usb.libusb_get_bus_number(devices[index]),
                               "address": usb.libusb_get_device_address(devices[index]),
                               "vendor_id": f"{descriptor.vendor:04x}",
                               "product_id": f"{descriptor.product:04x}"})
            return sorted(result, key=lambda item: (item["bus"], item["address"]))
        finally:
            usb.libusb_free_device_list(devices, 1)
    finally:
        usb.libusb_exit(context)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--library", help="Path to libusb-1.0 shared library (optional)")
    args = parser.parse_args()
    library = args.library or ctypes.util.find_library("usb-1.0")
    if not library:
        print("Install libusb or provide --library /path/to/libusb-1.0 library", file=sys.stderr)
        return 1
    try:
        print(json.dumps(inventory(library), indent=2))
        return 0
    except (OSError, RuntimeError) as error:
        print(f"USB inventory failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
