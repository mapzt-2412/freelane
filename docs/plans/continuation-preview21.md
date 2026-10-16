# Kế hoạch tiếp tục sau preview.21

Ngày: 2026-10-16. Mốc hiện tại: preview.21, source `985b8e9`; tài liệu phát hành `45a4411`. Đây là kế hoạch, không phải bằng chứng các tính năng dưới đây đã hoàn thành.

## Mục tiêu và cách triển khai

FreeLane là workspace miễn phí cho Mac Apple Silicon, ưu tiên Windows ARM và Ubuntu ARM64. Tiếp tục từ code hiện có, giữ dữ liệu người dùng, UI Anh/Việt nhất quán và cài đặt ít thao tác. Android Emulator/Xcode Simulator tiếp tục dùng backend SDK tương ứng.

Thực hiện tuần tự từng đợt có kết quả dùng được. Phần độc lập với phần cứng làm trên Linux cloud và native CI; Mac HVF/Dock/sleep và guest tương tác được giữ thành danh sách nghiệm thu riêng theo yêu cầu của chủ dự án. Không chờ Mac để làm phần độc lập, cũng không dùng cross-build thay bằng chứng chạy thật.

Chọn cách này thay vì làm tất cả song song: video, đồng bộ file và driver có các điều kiện khác nhau; một đợt hỏng phải có thể sửa và phát hành riêng. Chỉ viết implementation plan chi tiết cho đợt đã có giao diện kỹ thuật rõ; các nhánh nghiên cứu phải chốt kết quả trước khi cam kết triển khai.

## Thứ tự các đợt

| Đợt | Công việc | Kết quả và điều kiện đạt |
| --- | --- | --- |
| 1 — Viewer ổn định | Công bằng giữa input và chụp hình; giữ queue tối đa32; một worker; bỏ kết quả sai cửa sổ; không replay input | Khi queue liên tục có input, frame đến hạn vẫn được lên lịch sau tối đa8 input hoàn tất. Test thời gian giả lập, guest chậm/lỗi, đổi target và live tắt; không tuyên bố FPS hay Coherence video |
| 2 — Quyết định video | So sánh đường capture/encode riêng, SPICE và RDP/RemoteApp; kiểm tra Windows ARM, macOS, license/driver; đo baseline | Báo cáo có lệnh tái lập, tài nguyên, kích thước ảnh, throughput và latency; chọn go/no-go. Chỉ triển khai codec/channel sau quyết định; control RPC64KiB không biến thành video stream |
| 3 — Chia sẻ Windows | Đánh giá SMB so với đồng bộ qua agent hiện có; chọn một backend trước | Hai thư mục thử nghiệm, read-only/read-write, Unicode, mất kết nối, sửa hai phía và file trùng tên. Không tự ghi đè xung đột. Phân biệt mount trực tiếp và bản sao đồng bộ; physical hotplug là nghiệm thu riêng |
| 4 — Backup di chuyển | Export/import checkpoint offline, manifest/checksum và kiểm tra tương thích; sau đó thiết kế clone độc lập | Restore vào thư mục mới, dữ liệu/hash đúng, không đè source; thiếu dung lượng/corrupt/interruption đều giữ bản gốc. Clone chỉ nhận đạt sau kiểm tra identity/network/TPM và chạy đồng thời với source |
| 5 — Tài nguyên | Đo CPU/RAM/storage, cảnh báo pressure, phối hợp nhiều viewer và chính sách sleep | Quan sát khác cấu hình; không tự tắt guest vì áp lực. Đo trước/sau với cùng workload; quota/sleep Mac chỉ bật khi backend hỗ trợ và đã nghiệm thu |
| 6 — Audio/USB/đồ họa | Audio output trước, mic sau; danh sách USB dễ hiểu và attach/detach; đánh giá Ubuntu3D/Windows ARM graphics | Ma trận thiết bị/driver có kết quả chạy thật, permission/reconnect/conflict rõ. Không hứa Apple GPU passthrough nếu backend không hỗ trợ |
| 7 — Hoàn thiện sản phẩm | Recent apps, icon/Dock regeneration, MCP môi trường thử dùng một lần; updater xác thực và rollback | Job có exit code/artifact/ownership; cleanup không chạm VM người dùng. Update không thay binary đang chạy; signing trust/rotation/rollback có thiết kế trước. Developer ID/notarization phụ thuộc tài khoản/chứng chỉ bảo mật |
| Nghiệm thu Mac song hành | Setup Windows/Ubuntu HVF, reboot, Finder/Dock, clipboard/file dialogs, sleep/wake, normal-user Windows agent | Anh thực hiện khi có Mac; ghi model/macOS/RAM/backend và từng kết quả. Chưa chạy thì vẫn đánh dấu chưa nghiệm thu |

Các đợt 2/3/6 có thể kết luận một nhánh chưa khả thi; khi đó lưu bằng chứng và chọn nhánh được hỗ trợ, không thêm nút UI giả vờ đã chạy. P4/P5/P7 có phần cloud độc lập được chuyển lên nếu đợt trước đang chờ phần cứng; ghi rõ lý do thay thứ tự.

## Quy tắc hoàn tất mỗi đợt

1. Chốt phạm vi, dữ liệu chạm tới và bằng chứng cần có. Đọc spec/plan của đợt trước khi code.
2. Hành vi mới/lỗi logic có test thất bại đúng nguyên nhân trước khi sửa. UI thuần bố cục dùng ảnh chụp app thật; không thêm test chỉ mô phỏng styling.
3. Chạy test liên quan, full Rust/MCP khi đổi đường dùng chung, fmt/lint và build nền tảng bị ảnh hưởng. Phân biệt passed/ignored/chưa chạy.
4. Một review độc lập toàn đợt; xử lý Critical/Important rồi mới tích hợp. Commit chia theo phần có thể review, push main theo quyền đã cấp.
5. Chỉ tạo tag sau CI cần thiết đạt; kiểm tra payload public, checksum, kiến trúc và revision. Cập nhật README EN/VI, release notes và evidence theo hành vi cuối.
6. Giữ danh sách việc còn lại và hardware gates. Không đánh dấu toàn roadmap hoàn thành vì build hoặc ảnh UI đạt.

Không gán ngày hoàn thành cho video/driver/native acceptance trước khi có số đo và môi trường phù hợp. Không hứa chạy mọi app Windows hoặc nhanh hơn Parallels trước ma trận tương thích và phép đo tương đương.

## Đợt đầu đã tách riêng

- [Spec viewer scheduling](../superpowers/specs/2026-10-16-viewer-scheduling-design.md).
- [Implementation plan viewer scheduling](../superpowers/plans/2026-10-16-viewer-scheduling.md).
- Các đợt sau nhận spec/plan riêng sau khi đánh giá phụ thuộc; tài liệu này giữ thứ tự và tiêu chí nghiệm thu chung.

Đợt1 đã giao trong preview.22: chống đói lịch snapshot, queue32/một worker, kiểm thử EN/VI với peer giao thức sở hữu riêng. Native Mac/installed Windows acceptance vẫn chờ. Đợt kế tiếp là đánh giá video theo tiêu chí ở bảng trên, chưa cam kết codec hay FPS.
