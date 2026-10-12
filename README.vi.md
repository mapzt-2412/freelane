# FreeLane

[English](README.md) · Tiếng Việt

Trình quản lý máy ảo mã nguồn mở viết bằng Rust, hướng tới **Mac Apple Silicon**, hỗ trợ cấu hình **Windows 11 ARM và Ubuntu ARM64** qua QEMU.

Dự án đang phát triển. Kiểm thử chạy guest hiện thực hiện trên Linux cloud với giả lập TCG; chưa xác minh chạy giao diện/HVF trên Mac thật. Chưa có bằng chứng nhanh hơn Parallels hoặc chạy được mọi ứng dụng Windows.

## Đã có

- UI sáng, thư viện máy ảo, thẻ tài nguyên và chuyển Việt/Anh.
- Cấu hình CPU, RAM, mạng, hiển thị, ISO, guest agent và USB.
- Chia sẻ file USB chỉ đọc; Ubuntu có thư mục đọc/ghi hai chiều qua 9p.
- GPU Virtio **2D cho Ubuntu**, đã kiểm tra khung hình thật từ guest Linux ARM.
- CLI và MCP stdio để build/test, điều khiển VM, chụp màn hình và gọi guest agent.

Windows ARM đã vào desktop Audit và thực hiện các bài thử 7-Zip/Notepad. Ubuntu Server đã kiểm tra file/ZIP và giữ đúng dữ liệu sau reboot. Đây là các thử nghiệm có phạm vi cụ thể, chưa phải sản phẩm dùng hằng ngày.

## Build và mở giao diện

Cần Rust **1.99.0**, QEMU 10.0+, firmware ARM64 phù hợp; Windows còn cần swtpm và bộ cài do người dùng cung cấp.

```sh
cargo build --locked --workspace
./target/debug/freelane-desktop /duong/dan/ubuntu/vm.toml /duong/dan/windows/vm.toml --lang vi
```

Chọn thẻ VM bên trái hoặc **Thêm máy ảo** để mở tệp cấu hình có sẵn. **Cấu hình** chia thành Phần cứng, Đồ họa, Tệp & chia sẻ, Thiết bị. Tắt VM trước khi lưu; có tạo backup. Giao diện hiển thị ảnh chụp, chưa truyền video trực tiếp. Dùng cửa sổ QEMU riêng để nhập bàn phím.

![UI FreeLane tiếng Việt thật](docs/assets/desktop-manager.png)

Ảnh ghi lại VM thử nghiệm EFI trên Linux, không phải Ubuntu Desktop đã cài.

## Chưa hoàn tất

Chưa có GPU 3D/Metal, driver GPU Windows ARM, chia sẻ hai chiều cho Windows hay kiểm thử USB vật lý. Chưa xác minh game/FPS hoặc hiệu năng Mac. GPU Virtio 2D không đồng nghĩa với tăng tốc 3D.

Xem [cách tạo VM](docs/development.md), [UI](docs/desktop.md), [chia sẻ file/USB](docs/sharing.md), [GPU](docs/graphics.md) và [bằng chứng kiểm thử](docs/test-evidence.md).

Mã FreeLane dùng [MIT](LICENSE); font và các thành phần bên ngoài giữ giấy phép riêng. Windows và ứng dụng thương mại do người dùng tự cung cấp. FreeLane độc lập với Apple, Microsoft, Canonical và Parallels.
