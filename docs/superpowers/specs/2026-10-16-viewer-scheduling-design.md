# Viewer scheduling — đề xuất đợt tiếp theo

## Intent

Tiếp tục P3.5 từ preview.21: người dùng nhập liên tục vẫn nhận được hình cập nhật, queue không tăng vô hạn và input không bị gửi lại khi có lỗi. Đây là cải thiện snapshot viewer hiện hữu, chưa phải video hoặc full Coherence.

## Hiện trạng

`crates/desktop/src/app_ui.rs` có một worker và queue32. Cuối vòng UI luôn lấy input trước, chỉ capture khi queue rỗng. Input hoàn tất còn đưa deadline về hiện tại. Vì vậy một nguồn input liên tục có thể làm frame đói lịch dù worker vẫn hoạt động. Capture dùng JPEG tối đa640×480 trên control RPC; giữ nguyên trong đợt này.

## Thiết kế đề xuất

Tách chính sách scheduling thuần sang `crates/desktop/src/viewer_schedule.rs`; Viewer tiếp tục giữ action queue và thực thi RPC. Scheduler chỉ chọn Idle/Input/Capture dựa trên trạng thái worker/target/live/queue/deadline, không tạo thread hay tự gửi request.

Giữ tối đa32 input chưa gửi và một RPC đang chạy. Nếu live bật, có target và frame đến hạn, sau tối đa8 input hoàn tất liên tiếp phải chọn một capture trước input tiếp theo. Không có input thì capture đến hạn ngay. Nếu frame chưa đến hạn thì tiếp tục input, không chặn UI để chờ. Frame thành công đặt deadline sau1giây và reset burst. Input thành công không đẩy deadline frame lùi; khi hàng đợi hết có thể yêu cầu capture sớm như hành vi cũ.

Manual refresh vẫn là yêu cầu riêng khi worker rảnh. Tắt live không tự chụp; không ảnh hưởng input/manual refresh. Đổi target hoặc refresh inventory xóa input/drag/texture như hiện tại và reset scheduler. Chỉ hiển thị frame có đúng WindowTarget hiện tại. Lỗi RPC dừng live, xóa input chưa gửi và không retry mutation. Không thêm retry vào capture; giữ nguyên một retry có điều kiện đã có cho timeout được guest xác nhận.

## Giới hạn và nghiệm thu

Burst8 là giới hạn công bằng theo số thao tác, không phải cam kết latency theo giây: một RPC chậm vẫn có thể chặn worker theo timeout hiện có. Giữ Rust1.99, dependency hiện tại, protocol hiện tại, EN/VI, snapshot1giây/640×480. Không tăng FPS mặc định hay thay input semantics.

Nghiệm thu bằng clock giả lập trong test và private guest socket trả lời có kiểm soát: queue32 liên tục; busy worker; live tắt; frame đến hạn/chưa đến hạn; đổi target khi frame đang chạy; input lỗi không replay. Capture/input được ghi theo loại và identity, không ghi text/clipboard/file contents. Native EN/VI UI kiểm tra bật/tắt live, manual refresh và đổi cửa sổ. Windows CI/native Mac giữ các acceptance riêng, không suy ra từ fixture.

Đây là tài liệu kế hoạch; chưa triển khai trong lần lập kế hoạch này.
