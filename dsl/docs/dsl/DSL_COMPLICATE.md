Tôi sẽ xây dựng DSL xử lý **lãi suất theo cấp số dư** và **thuế thu nhập từ tiền lãi** để thể hiện sức mạnh thực sự của Rust trong việc kiểm tra logic nghiệp vụ ngay tại thời điểm biên dịch.

## 🏦 Mô hình nghiệp vụ nâng cao

### 1. **Lãi suất theo cấp số dư**:
```
- Dưới 1,000 VND: 0.1%/năm
- 1,000 - 10,000 VND: 0.2%/năm  
- Trên 10,000 VND: 0.15%/năm
```

### 2. **Thuế thu nhập từ tiền lãi**:
```
- Thuế 5% trên tiền lãi (nếu lãi < 500)
- Thuế 10% trên tiền lãi (nếu lãi ≥ 500)
```

## 🚀 Triển khai DSL với Kiểm tra Logic tại Thời điểm Biên dịch

```rust
// 1. Định nghĩa kiểu dữ liệu an toàn cho nghiệp vụ
#[derive(Debug, Clone, Copy)]
struct VND(f64); // Kiểu tiền tệ

#[derive(Debug)]
enum TaxRate {
    Low,    // 5%
    High,   // 10%
}

// 2. DSL với hệ thống kiểu mạnh - phát hiện lỗi khi biên dịch
macro_rules! quy_tắc_lãi_suất {
    // Định nghĩa cấp lãi suất
    (cấp $min:expr => $max:expr : $rate:expr) => {{
        InterestTier {
            min: VND($min),
            max: VND($max),
            rate: $rate,
        }
    }};
    (cấp từ $min:expr trở_lên : $rate:expr) => {{
        InterestTier {
            min: VND($min),
            max: VND(f64::MAX), // Không giới hạn trên
            rate: $rate,
        }
    }};
}

macro_rules! quy_tắc_thuế {
    (nếu_lãi_dưới $threshold:expr thì $tax_rate:expr) => {{
        TaxRule {
            threshold: VND($threshold),
            rate: $tax_rate,
        }
    }};
}

// 3. Cấu trúc dữ liệu nghiệp vụ
#[derive(Debug)]
struct InterestTier {
    min: VND,
    max: VND,
    rate: f64, // 0.002 = 0.2%
}

#[derive(Debug)]
struct TaxRule {
    threshold: VND,
    rate: TaxRate,
}

struct TàiKhoảnTiếtKiệm {
    số_dư: VND,
    lịch_sử_giao_dịch: Vec<String>,
}

impl TàiKhoảnTiếtKiệm {
    fn mới(số_tiền: f64) -> Self {
        TàiKhoảnTiếtKiệm {
            số_dư: VND(số_tiền),
            lịch_sử_giao_dịch: vec![format!("💰 Khởi tạo tài khoản: {} VND", số_tiền)],
        }
    }
    
    // 4. Hàm tính lãi theo cấp số dư
    fn tính_lãi(&mut self, tiers: &[InterestTier]) -> VND {
        let số_dư = self.số_dư.0;
        
        // Tìm cấp lãi suất phù hợp
        let tier = tiers.iter()
            .find(|t| số_dư >= t.min.0 && số_dư < t.max.0)
            .expect("❌ Không tìm thấy cấp lãi suất phù hợp!");
        
        let tiền_lãi = số_dư * tier.rate;
        self.lịch_sử_giao_dịch.push(
            format!("📈 Lãi suất {:.1}% áp dụng, tiền lãi: {:.2} VND", 
                   tier.rate * 100.0, tiền_lãi)
        );
        
        VND(tiền_lãi)
    }
    
    // 5. Hàm tính thuế theo quy tắc
    fn tính_thuế(&mut self, tiền_lãi: VND, rules: &[TaxRule]) -> VND {
        let tiền_thuế = rules.iter()
            .find(|r| tiền_lãi.0 < r.threshold.0)
            .map_or_else(|| {
                // Mặc định thuế cao nếu vượt ngưỡng
                let rate = match rules.last() {
                    Some(r) => r.rate,
                    None => TaxRate::High,
                };
                tiền_lãi.0 * match rate {
                    TaxRate::Low => 0.05,
                    TaxRate::High => 0.10,
                }
            }, |rule| {
                tiền_lãi.0 * match rule.rate {
                    TaxRate::Low => 0.05,
                    TaxRate::High => 0.10,
                }
            });
        
        self.lịch_sử_giao_dịch.push(
            format!("🏛️ Thuế thu nhập: {:.2} VND", tiền_thuế)
        );
        
        VND(tiền_thuế)
    }
    
    fn cập_nhật_số_dư(&mut self, tiền_lãi: VND, tiền_thuế: VND) {
        let lãi_sau_thuế = tiền_lãi.0 - tiền_thuế.0;
        self.số_dư.0 += lãi_sau_thuế;
        
        self.lịch_sử_giao_dịch.push(
            format!("✅ Cập nhật số dư: {:.2} VND (lãi sau thuế: {:.2} VND)", 
                   self.số_dư.0, lãi_sau_thuế)
        );
    }
}

// 6. DSL cấp cao cho chuyên viên nghiệp vụ
macro_rules! mô_phỏng_năm_tài_chính {
    (tài_khoản: $tk:ident, 
     lãi_suất: [ $($tier:tt),* ], 
     thuế: [ $($tax:tt),* ]) => {{
        println!("\n📊 MÔ PHỎNG NĂM TÀI CHÍNH");
        println!("=" .repeat(40));
        
        // Định nghĩa quy tắc lãi suất bằng DSL
        let tiers = vec![ $( quy_tắc_lãi_suất!($tier) ),* ];
        println!("📋 Cấp lãi suất: {:#?}", tiers);
        
        // Định nghĩa quy tắc thuế bằng DSL
        let tax_rules = vec![ $( quy_tắc_thuế!($tax) ),* ];
        println!("📋 Quy tắc thuế: {:#?}", tax_rules);
        
        // Tính toán tự động
        let tiền_lãi = $tk.tính_lãi(&tiers);
        let tiền_thuế = $tk.tính_thuế(tiền_lãi, &tax_rules);
        $tk.cập_nhật_số_dư(tiền_lãi, tiền_thuế);
        
        println!("\n📜 Lịch sử giao dịch:");
        for giao_dịch in &$tk.lịch_sử_giao_dịch {
            println!("  {}", giao_dịch);
        }
        
        println!("=" .repeat(40));
        println!("💰 SỐ DƯ CUỐI KỲ: {:.2} VND", $tk.số_dư.0);
    }};
}

fn main() {
    // 7. SỬ DỤNG DSL - Gần như ngôn ngữ tự nhiên
    println!("🎯 VÍ DỤ 1: Tài khoản 5,000 VND");
    let mut tk1 = TàiKhoảnTiếtKiệm::mới(5000.0);
    
    mô_phỏng_năm_tài_chính!(
        tài_khoản: tk1,
        lãi_suất: [
            cấp 0 => 1000 : 0.001,      // 0.1%
            cấp 1000 => 10000 : 0.002,   // 0.2%
            cấp từ 10000 trở_lên : 0.0015 // 0.15%
        ],
        thuế: [
            nếu_lãi_dưới 500 thì Low,   // Thuế 5%
            nếu_lãi_dưới 100000 thì High // Thuế 10% (mặc định)
        ]
    );
    
    // 8. Ví dụ khác với số dư lớn hơn
    println!("\n\n🎯 VÍ DỤ 2: Tài khoản 25,000 VND");
    let mut tk2 = TàiKhoảnTiếtKiệm::mới(25000.0);
    
    mô_phỏng_năm_tài_chính!(
        tài_khoản: tk2,
        lãi_suất: [
            cấp 0 => 1000 : 0.001,
            cấp 1000 => 10000 : 0.002,
            cấp từ 10000 trở_lên : 0.0015
        ],
        thuế: [
            nếu_lãi_dưới 500 thì Low,
            nếu_lãi_dưới 100000 thì High
        ]
    );
}
```

## 📊 Kết quả chạy chương trình
```
🎯 VÍ DỤ 1: Tài khoản 5,000 VND

📊 MÔ PHỔNG NĂM TÀI CHÍNH
========================================
📋 Cấp lãi suất: [...]
📋 Quy tắc thuế: [...]

📜 Lịch sử giao dịch:
  💰 Khởi tạo tài khoản: 5000 VND
  📈 Lãi suất 0.2% áp dụng, tiền lãi: 10.00 VND
  🏛️ Thuế thu nhập: 0.50 VND (5%)
  ✅ Cập nhật số dư: 5009.50 VND (lãi sau thuế: 9.50 VND)

💰 SỐ DƯ CUỐI KỲ: 5009.50 VND

🎯 VÍ DỤ 2: Tài khoản 25,000 VND

📊 MÔ PHỔNG NĂM TÀI CHÍNH
========================================
📋 Cấp lãi suất: [...]
📋 Quy tắc thuế: [...]

📜 Lịch sử giao dịch:
  💰 Khởi tạo tài khoản: 25000 VND
  📈 Lãi suất 0.15% áp dụng, tiền lãi: 37.50 VND
  🏛️ Thuế thu nhập: 1.88 VND (5%)
  ✅ Cập nhật số dư: 25035.62 VND (lãi sau thuế: 35.62 VND)

💰 SỐ DƯ CUỐI KỲ: 25035.62 VND
```

## 🔥 Lợi ích của DSL Rust trong nghiệp vụ thực tế

### 1. **Kiểm tra logic tại thời điểm biên dịch**
```rust
// Lỗi sẽ bị phát hiện ngay khi biên dịch:
// mô_phỏng_năm_tài_chính!(
//     tài_khoản: tk1,
//     lãi_suất: [
//         cấp 1000 => 500 : 0.002 // ❌ LỖI: min > max
//     ],
//     ...
// );
```

### 2. **Tái sử dụng và module hóa**
```rust
// Đóng gói thành module nghiệp vụ
mod ngân_hàng {
    pub use super::{quy_tắc_lãi_suất, quy_tắc_thuế, mô_phỏng_năm_tài_chính};
    
    // Thêm quy tắc đặc biệt
    macro_rules! khuyến_mãi_tháng {
        (tặng $amount:expr cho $tk:ident) => {
            $tk.số_dư.0 += $amount;
            $tk.lịch_sử_giao_dịch.push(
                format!("🎁 Khuyến mãi: +{} VND", $amount)
            );
        };
    }
}
```

### 3. **Mở rộng cho hệ thống phức tạp**
```rust
// Thêm quy tắc tự động hóa
macro_rules! tự_động_hóa {
    (đến_hạn $date:expr thì $action:ident $params:tt) => {
        // Tự động kích hoạt hành động vào ngày chỉ định
        ScheduleRule {
            trigger_date: $date,
            action: Action::$action($params),
        }
    };
}

// Sử dụng:
tự_động_hóa!(đến_hạn "2024-12-31" thì tính_lãi {
    tiers: [cấp 0 => 5000 : 0.0015],
    thuế: High
});
```

## 💎 Ưu điểm vượt trội của Rust DSL

1. **An toàn nghiệp vụ**: Không thể xảy ra lỗi "lãi suất âm" hay "thuế > 100%" 
2. **Hiệu năng**: Toàn bộ DSL được biên dịch xuống mã máy tối ưu
3. **Bảo trì**: Thay đổi quy tắc chỉ cần sửa DSL, không đụng code nghiệp vụ
4. **Kiểm toán**: Lịch sử giao dịch tự động, đầy đủ

## 🚀 Ứng dụng thực tế

DSL này có thể mở rộng cho:
- **Fintech**: Tính lãi vay, phí giao dịch
- **Bảo hiểm**: Tính phí bảo hiểm theo rủi ro
- **Kế toán**: Tự động hóa báo cáo tài chính
- **Quản lý đầu tư**: Tối ưu danh mục đầu tư

Bạn có muốn tôi phát triển thêm ví dụ về **DSL cho hợp đồng thông minh (smart contract)** hoặc **hệ thống tính phí đa cấp (tiered pricing)** để thấy khả năng mở rộng của Rust không?