//! Module chứa định nghĩa tài khoản tiết kiệm
//! 
//! Đây là đối tượng lõi của DSL ngân hàng, đại diện cho một tài khoản
//! tiền gửi với các thao tác nghiệp vụ cơ bản.

/// Tài khoản tiết kiệm
/// 
/// # Thuộc tính
/// - `balance`: Số dư hiện tại của tài khoản
/// 
/// # Ví dụ
/// ```
/// use banking_dsl::SavingsAccount;
/// 
/// let mut account = SavingsAccount::new(100.0);
/// account.subtract_fee(1.0);
/// account.add_interest(0.002);
/// println!("Số dư: {:.2}", account.get_balance());
/// ```
#[derive(Debug, Clone)]
pub struct SavingsAccount {
    balance: f64,
}

impl SavingsAccount {
    /// Tạo tài khoản mới với số tiền gửi ban đầu
    /// 
    /// # Tham số
    /// - `initial_deposit`: Số tiền gửi ban đầu
    pub fn new(initial_deposit: f64) -> Self {
        println!("🏦 Mở tài khoản tiết kiệm với số tiền: {:.2}", initial_deposit);
        SavingsAccount { balance: initial_deposit }
    }

    /// Trừ phí quản lý hàng năm
    /// 
    /// # Tham số
    /// - `fee`: Số tiền phí cần trừ
    /// 
    /// # Lưu ý
    /// Phí sẽ được trừ trực tiếp vào số dư tài khoản
    pub fn subtract_fee(&mut self, fee: f64) {
        self.balance -= fee;
        println!("✅ Đã trừ phí quản lý: {:.2}. Số dư còn: {:.2}", fee, self.balance);
    }

    /// Cộng lãi suất vào tài khoản
    /// 
    /// # Tham số
    /// - `annual_rate`: Lãi suất năm (ví dụ: 0.002 = 0.2%)
    /// 
    /// # Công thức
    /// `lãi = số_dư * lãi_suất`
    pub fn add_interest(&mut self, annual_rate: f64) {
        let interest = self.balance * annual_rate;
        self.balance += interest;
        println!("💰 Đã cộng lãi: {:.2} (lãi suất {:.2}%). Số dư mới: {:.2}", 
                 interest, annual_rate * 100.0, self.balance);
    }

    /// Lấy số dư hiện tại
    pub fn get_balance(&self) -> f64 {
        self.balance
    }

    /// Gửi thêm tiền vào tài khoản
    /// 
    /// # Tham số
    /// - `amount`: Số tiền cần gửi thêm
    pub fn deposit(&mut self, amount: f64) {
        self.balance += amount;
        println!("📥 Đã gửi thêm: {:.2}. Số dư mới: {:.2}", amount, self.balance);
    }

    /// Rút tiền từ tài khoản
    /// 
    /// # Tham số
    /// - `amount`: Số tiền cần rút
    /// 
    /// # Trả về
    /// - `Ok(amount)`: Nếu rút thành công
    /// - `Err(message)`: Nếu số dư không đủ
    pub fn withdraw(&mut self, amount: f64) -> Result<f64, String> {
        if self.balance >= amount {
            self.balance -= amount;
            println!("📤 Đã rút: {:.2}. Số dư còn: {:.2}", amount, self.balance);
            Ok(amount)
        } else {
            let msg = format!("❌ Số dư không đủ. Yêu cầu: {:.2}, Hiện có: {:.2}", 
                             amount, self.balance);
            println!("{}", msg);
            Err(msg)
        }
    }

    /// Hiển thị thông tin tài khoản
    pub fn display(&self) {
        println!("═══════════════════════════════════");
        println!("📊 THÔNG TIN TÀI KHOẢN TIẾT KIỆM");
        println!("═══════════════════════════════════");
        println!("   Số dư hiện tại: {:.2} VND", self.balance);
        println!("═══════════════════════════════════");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_account() {
        let account = SavingsAccount::new(100.0);
        assert_eq!(account.get_balance(), 100.0);
    }

    #[test]
    fn test_subtract_fee() {
        let mut account = SavingsAccount::new(100.0);
        account.subtract_fee(1.0);
        assert_eq!(account.get_balance(), 99.0);
    }

    #[test]
    fn test_add_interest() {
        let mut account = SavingsAccount::new(100.0);
        account.add_interest(0.002);
        assert!((account.get_balance() - 100.2).abs() < 0.001);
    }

    #[test]
    fn test_deposit() {
        let mut account = SavingsAccount::new(100.0);
        account.deposit(50.0);
        assert_eq!(account.get_balance(), 150.0);
    }

    #[test]
    fn test_withdraw_success() {
        let mut account = SavingsAccount::new(100.0);
        let result = account.withdraw(30.0);
        assert!(result.is_ok());
        assert_eq!(account.get_balance(), 70.0);
    }

    #[test]
    fn test_withdraw_insufficient() {
        let mut account = SavingsAccount::new(100.0);
        let result = account.withdraw(150.0);
        assert!(result.is_err());
        assert_eq!(account.get_balance(), 100.0);
    }
}
