# Chuyển file

Trong **Hiển thị**, bấm **Gửi file vào máy ảo**, chọn file. Có tiến độ và **Hủy chuyển file**. Tên tiếng Việt và dấu cách được hỗ trợ. File trùng tên bị từ chối; file cũ được giữ nguyên.

Guest agent mới cần được chạy với thư mục nhận đã chọn:

```powershell
New-Item -ItemType Directory -Force "$env:USERPROFILE\Documents\FreeLane Inbox"
.\freelane-guest-agent.exe --transfer-root "$env:USERPROFILE\Documents\FreeLane Inbox"
```

```bash
mkdir -p "$HOME/FreeLane Inbox"
./freelane-guest-agent --transfer-root "$HOME/FreeLane Inbox"
```

Đây là chuyển bản sao theo yêu cầu, chưa phải đồng bộ nền hay thư mục mount. Chia sẻ 9p của Ubuntu vẫn dùng riêng. Trình cài agent Windows chưa tự thêm tham số thư mục nhận vào startup.

**Mở văn bản sau khi chuyển** hỗ trợ `.txt`/`.csv` tối đa 32 MiB, bằng Notepad hệ thống trên Windows hoặc gnome-text-editor/gedit đã cài trên Linux có giao diện. Ubuntu Server nhận file bình thường nhưng cần giao diện và trình đọc để mở văn bản. Chưa mở PDF/ảnh hoặc gọi file association.

File được kiểm tra dung lượng và SHA-256 trước khi hoàn tất, không tự ghi đè hay thử lại thao tác chưa rõ kết quả. Một phiên chuyển mỗi agent, tối đa 64 GiB/file; đây không phải giới hạn dung lượng đĩa VM. Tiến độ hiện bắt đầu sau bước tính checksum. Linux cần filesystem hỗ trợ tệp ẩn danh/hard link như ext4; Windows cần hard link như NTFS. Filesystem không hỗ trợ sẽ báo lỗi và giữ file cũ.

Nếu ngắt kết nối giữa chừng, chờ hết hạn năm phút rồi gửi yêu cầu mới hoặc khởi động lại agent. Agent bị dừng đột ngột trên Windows có thể để lại `.freelane-stage-*`; chỉ kiểm tra/dọn sau khi agent đã dừng. Kiểm thử Mac thật, guest Windows tương tác và startup tài khoản thường vẫn đang chờ.
