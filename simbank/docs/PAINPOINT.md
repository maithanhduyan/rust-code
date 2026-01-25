Bài toán FinTech liên quan đến sự sai khác trong số liệu tiền gửi của KH từ góc độ IT BA

1. BA FinTech có nỗi sợ gì không?
Nếu bạn là một IT BA trong lĩnh vực FinTech, chắc hẳn bạn đã từng nghe câu than:
“Số liệu tiền gửi không khớp!”
Nghe tưởng đơn giản, nhưng đó là cơn ác mộng với BA mới: hàng triệu USD trong hệ thống custody, khách hàng gửi tiền đi đâu, số liệu ngân hàng đối chiếu, phân bổ tiền đầu tư… tất cả đều phải khớp hoàn toàn.
BA mới thường mất ngủ với Excel và báo cáo. BA lâu năm lại nhìn thấy pattern, workflow, và giải pháp hệ thống trước khi hoảng loạn.
Câu chuyện dưới đây là minh họa điển hình cho sự khác biệt đó.

2. Bối cảnh dự án – Hệ thống custody và kiểm toán tiền gửi
Một ngân hàng đầu tư triển khai hệ thống custody & quản lý tiền gửi, với các mục tiêu:
Theo dõi tiền gửi khách hàng
Phân bổ tiền đi đầu tư: trái phiếu, chứng khoán, quỹ liên kết
Đảm bảo dữ liệu luôn khớp giữa ledger nội bộ, ngân hàng đối tác và báo cáo quản lý
Hỗ trợ kiểm toán định kỳ
Dữ liệu:
Hàng nghìn khách hàng, mỗi khách 5–10 tài khoản tiền gửi
Mỗi ngày phát sinh hàng nghìn giao dịch gửi, rút, chuyển đầu tư
Hệ thống gồm: core banking, trading system, reporting database
Một ngày, team kiểm toán báo cáo:
“Số dư tiền gửi của một số khách hàng không khớp giữa ledger và báo cáo đầu tư!”
BA mới thấy hoảng: “Mình phải check từng giao dịch bằng tay sao đây?”

3. BA mới tiếp cận – “Chiến đấu với Excel & báo cáo”
BA mới thường làm theo quy trình:
Tải tất cả report từ hệ thống
Ledger, báo cáo đầu tư, hệ thống ngân hàng đối tác
Xuất ra Excel, ghép sheet thủ công
So sánh từng dòng
Filter theo customer_id, account_id
Check từng transaction amount, ngày, type
Tìm lỗi bằng mắt thường hoặc pivot table
Kết quả? BA mới:
Tốn nhiều ngày, nhiều giờ ngồi tính toán
Dễ bỏ sót các transaction phức tạp (chuyển tiền giữa các quỹ, trừ phí quản lý)
Stress cao, báo cáo chưa chắc hoàn toàn chính xác
Ví dụ: BA mới thấy khách hàng A có 100,000 USD trong ledger, nhưng báo cáo đầu tư chỉ 95,000 USD. Họ dành cả ngày để so sánh từng transaction, nhưng chưa phát hiện ra 5,000 USD bị trừ phí quản lý tự động chưa được phản ánh đúng trong báo cáo.

4. BA lâu năm – “Nhìn workflow, dùng logic, tìm gốc rễ”
BA senior không lao vào Excel trước. Họ hỏi:
Các giao dịch được generate từ đâu? (core banking, trading, manual entry?)
Quy tắc phân bổ tiền đầu tư và trừ phí như thế nào?
Hệ thống reporting có đồng bộ real-time không, hay batch overnight?
Sau đó họ đi theo workflow:
Map dữ liệu giữa các hệ thống
Ledger nội bộ ↔ Core banking ↔ Trading system ↔ Reporting DB
Xác định rule trừ phí, chuyển đầu tư
Các khoản phí, interest, dividend được booking tự động
Dùng công cụ AI / scripting
Python + Pandas + SQL để check batch transaction
Highlight discrepancy thay vì check thủ công từng row
Ví dụ: Python script lọc ra các giao dịch khách hàng A với discrepancy > 0, tự động gợi ý nguyên nhân: phí quản lý 5% chưa tính trong reporting system.
import pandas as pd
ledger = pd.read_csv('ledger.csv')
invest_report = pd.read_csv('invest_report.csv')
# Merge theo customer & account
df = ledger.merge(invest_report, on=['customer_id','account_id'], suffixes=('_ledger','_report'))
# Tính discrepancy
df['diff'] = df['balance_ledger'] - df['balance_report']
# Highlight các discrepancy > 0
discrepancy = df[df['diff'] != 0]
print(discrepancy)
Chỉ trong vài phút, BA senior xác định nguồn gốc lệch số, thay vì mất cả tuần so Excel.

5. Case study – Phân bổ tiền gửi đi đầu tư
Problem: Ngân hàng A muốn đảm bảo tiền gửi của khách hàng được phân bổ đúng giữa các quỹ, nhưng báo cáo cuối ngày cho thấy discrepancy 0.5–1% mỗi ngày.
BA mới:
Check từng giao dịch bằng mắt
So sánh số dư từng tài khoản
Tốn nhiều giờ nhưng vẫn chưa chắc chắn nguyên nhân
BA senior:
Tìm hiểu workflow đầu tư: mỗi ngày hệ thống auto trừ 0.1% phí quản lý trên tất cả tài khoản
Mapped các giao dịch batch giữa ledger và reporting
Dùng script check batch discrepancy, highlight các khoản chưa trừ phí hoặc chưa booking
Đưa ra insight: discrepancy 0.5% do batch overnight chưa được cập nhật trong report real-time
Kết quả:
BA senior giải thích cho kiểm toán: “Discrepancy không phải lỗi, mà do timing của batch update.”
Kiểm toán hiểu, vấn đề không nghiêm trọng
BA mới chưa chắc nhận ra timing này, dễ báo động false alarm

6. Mini-story qua một ví dụ vui sau đây:
Một ví dụ vui:
BA mới: “Ôi không, số dư khách hàng B lệch 20,000 USD, khách hàng nổi giận!”
BA senior: “Hãy xem rule auto-reinvest dividend… aha, số tiền này đã được booking vào quỹ mới, nhưng report ledger chưa update.”
Lesson: workflow + hiểu business rule quan trọng hơn việc soi dữ liệu từng dòng.

7. Bài học rút ra cho IT BA trong FinTech
Số liệu tiền gửi chưa chắc là lỗi, có thể là workflow & business rule
BA mới: tập trung vào “làm sạch dữ liệu”
BA lâu năm: tập trung vào “hiểu business workflow + rules + timing”
Automation & scripting / AI tools giúp kiểm soát discrepancy nhanh hơn
Insight > dữ liệu sạch, vì stakeholders cần giải thích discrepancy, không chỉ nhìn con số

8. “KEY TAKEAWAYS” cho các bạn IT BA làm trong lĩnh vực Fintech
Trong FinTech, BA không chỉ là người ghi nhận bug hay tạo report đẹp.
BA giỏi là người dịch workflow nghiệp vụ (custody, đầu tư, phân bổ tiền) thành giải pháp IT, đảm bảo số liệu khớp, insight chính xác, stakeholder yên tâm.
Case study này chứng minh: cùng một vấn đề – số liệu tiền gửi không khớp – BA mới sẽ hoảng loạn, BA lâu năm sẽ dùng workflow + automation + business insight để giải quyết nhanh, chính xác, giảm stress, tăng giá trị cho tổ chức.
Thông điệp: hiểu nghiệp vụ + workflow + business rule quan trọng hơn việc kiểm tra từng số liệu, AI và automation là công cụ hỗ trợ, không thay thế tư duy BA.
Với mindset này, mọi discrepancy dù lớn hay nhỏ, dù liên quan đến custody hay phân bổ đầu tư, đều có thể xử lý hiệu quả và minh bạch – đúng tiêu chuẩn FinTech.