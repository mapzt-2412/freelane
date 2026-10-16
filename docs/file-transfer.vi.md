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

## Nhận file từ máy ảo

Chạy agent cùng phiên bản trong phiên người dùng của máy ảo, chọn một thư mục xuất có sẵn:

```text
freelane-guest-agent --export-root <thu-muc-xuat-co-san>
```

Thư mục nhận `--transfer-root` và thư mục xuất `--export-root` độc lập, mặc định đều tắt. Đặt file trực tiếp trong thư mục xuất. Tại tab **Màn hình**, nhập **Tên file trong thư mục xuất của máy ảo**, bấm **Nhận file từ máy ảo** rồi chọn thư mục nhận trên máy, có thể chọn ổ ngoài. CLI:

```text
freelane file-receive vm.toml "Ket qua.txt" /duong-dan/thu-muc-nhan
```

Cần agent hỗ trợ `file_export` v11; các tính năng cũ vẫn dùng phiên bản giao thức cũ. Chỉ nhận tên file, không nhận đường dẫn/thư mục con hoặc duyệt toàn bộ máy ảo. Mỗi lần đọc một file thường, từng khối tối đa 16 KiB, giới hạn chuyển mỗi file 64 GiB (không phải giới hạn đĩa máy ảo). Agent tính checksum từng khối và trả SHA-256 ở bước kết thúc sau khi kiểm tra kích thước/thời gian sửa file nguồn. App so khớp kích thước và checksum cuối trước khi công bố file hoàn chỉnh; hủy, sai checksum hoặc trùng tên đều giữ nguyên file cũ, kể cả symlink hỏng. File tạm riêng tư nằm trong thư mục nhận; cần dùng thư mục cục bộ tin cậy, tránh tiến trình khác thay thế file tạm. Không sửa file nguồn khi đang chuyển; đây là bản sao chủ động, chưa phải đồng bộ trực tiếp hoặc snapshot nguyên tử.

Mỗi agent có một phiên đọc, hết hạn sau năm phút không hoạt động và bị thu hồi khi mất kết nối. Không tự thử lại, mở file hay xóa file nguồn trong guest. Bộ cài tự khởi động của preview.18 chạy agent không có thư mục xuất; hãy dừng agent đó rồi chạy lệnh trên để bật riêng cho phiên dùng. Kiểm thử tương tác thực trên Mac và Windows ARM vẫn còn chờ.

Agent Windows mới có thể lưu riêng thư mục nhận/xuất bằng `-InboxDirectory`, `-OutboxDirectory` khi cài, hoặc đổi/tắt bằng `-Action Configure`. Ví dụ, sau khi dừng agent đang chạy:

```powershell
.\install-windows-agent.ps1 -Action Configure -OutboxDirectory 'D:\Guest Exports'
# Tắt thư mục xuất; thư mục nhận giữ nguyên:
.\install-windows-agent.ps1 -Action Configure -OutboxDirectory ''
```

Chạy lại agent hoặc đăng nhập lại để áp dụng. Cài mới không chọn thư mục thì cả hai chiều đều tắt. Update/rollback giữ lựa chọn hiện tại, không khôi phục quyền xuất đã tắt. Thư mục/ổ bị mất kết nối không được tự tạo lại; cấu hình lỗi sẽ từ chối khởi động agent. Linux vẫn dùng cờ CLI. Cần agent cùng bản mới; preview.18 vẫn dùng cờ cho từng phiên như phần trên.

## Chọn file từ danh sách máy ảo

Với agent mới có hỗ trợ v12, bấm **Chọn file trong máy ảo** ở tab Màn hình, rồi chọn tên trong cửa sổ thư mục xuất. App điền tên vào ô nhận; bấm **Nhận file từ máy ảo** để chọn nơi lưu và xác minh dữ liệu như trước. **Tải lại danh sách** chỉ đọc lại khi anh bấm. CLI: `freelane file-list vm.toml` trả tên và dung lượng dạng JSON.

Tính năng `file_export_catalog` v12 chỉ liệt kê thư mục xuất đã được bật, đọc thông tin file và không mở nội dung hay chạy chương trình. Mỗi lượt xét tối đa512 mục, trả tối đa128 file thường hợp lệ theo thứ tự tên. Không hiện thư mục con, symlink, tên ẩn/không hợp lệ hoặc file vượt giới hạn chuyển. Danh sách bị giới hạn có thông báo; vẫn có thể nhập tên hoặc chọn thư mục xuất ít file hơn. Đây là danh sách tại thời điểm đọc; tên/nội dung có thể đổi trước lúc nhận, không phải snapshot. Bước nhận vẫn kiểm tra kích thước/SHA-256. Agent cũ vẫn nhận bằng tên qua v11; nút chọn danh sách sẽ báo cần cập nhật. Danh sách/phản hồi cũ không được áp dụng sang VM khác.
