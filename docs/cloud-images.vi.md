# Ubuntu từ ảnh cloud

Trong **Tạo máy ảo mới**, chọn **Ubuntu ARM64 → Ảnh Ubuntu cloud**, chọn tệp qcow2 ARM64, firmware/NVRAM và CPU/RAM/ổ đĩa. Xác nhận kiến trúc rồi bấm tạo. FreeLane giữ nguyên ảnh nguồn, tạo bản đĩa độc lập và tăng dung lượng trong thư mục VM mới.

Tải ảnh từ [Ubuntu chính thức](https://cloud-images.ubuntu.com/noble/current/) và kiểm tra hash với `SHA256SUMS` đã xác minh chữ ký. Header qcow2 chỉ xác minh định dạng/lưu trữ, không chứng minh kiến trúc hay nguồn gốc; phần ARM64 vẫn cần người dùng xác nhận. Không hỗ trợ backing file, dữ liệu ngoài, ảnh mã hóa hoặc dirty. Tệp nguồn tối đa 16 GiB; từng lệnh chuyển đổi/tăng dung lượng giới hạn năm phút.

Ảnh cloud đã chứa Ubuntu Server, nhưng tạo VM chưa chứng minh boot xong, có tài khoản đăng nhập, agent hay Ubuntu Desktop. **ISO NoCloud seed** tùy chọn giúp cấu hình tài khoản/khóa SSH và mạng cho lần boot đầu. Nếu thiếu seed phù hợp, ảnh mặc định có thể chưa có tài khoản đăng nhập dùng được. Tệp seed phải là tệp thường có dữ liệu, tối đa 64 MiB; cấu trúc ISO và nội dung seed hiện chưa được xác minh. Chỉ dùng nội dung anh muốn chạy trong guest và không đưa khóa SSH riêng tư vào seed.

Giữ đường dẫn seed còn truy cập được tới khi provisioning hoàn tất. Cloud-init có thể chờ lâu nếu mạng tắt hoặc cấu hình NIC không đúng; QEMU đang chạy chưa có nghĩa là guest sẵn sàng. Kênh guest agent không tự cài agent.

Xem [mẫu cấu hình CLI và NoCloud network-config](cloud-images.md). Kiểm thử Mac thực tế và Ubuntu Desktop vẫn còn chờ.

Xem [thiết lập nơi lưu và đĩa tăng dần](setup.vi.md). Không yêu cầu chỗ trống bằng dung lượng tối đa của đĩa mới; khi nhập ảnh vẫn tính chỗ cho snapshot nguồn và chuyển đổi.
