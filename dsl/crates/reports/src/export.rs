//! Xuất báo cáo ra các định dạng khác nhau

use business::YearlySimulationResult;

/// Trait xuất báo cáo
pub trait ReportExporter {
    fn export(&self, results: &[YearlySimulationResult]) -> String;
}

/// Xuất CSV
pub struct CsvExporter;

impl ReportExporter for CsvExporter {
    fn export(&self, results: &[YearlySimulationResult]) -> String {
        let mut csv = String::new();
        csv.push_str("Năm,Số dư đầu kỳ,Phí,Lãi,Thuế,Lãi ròng,Số dư cuối kỳ\n");
        
        for r in results {
            csv.push_str(&format!(
                "{},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2}\n",
                r.year,
                r.opening_balance.value(),
                r.fee_charged.value(),
                r.interest_earned.value(),
                r.tax_paid.value(),
                r.net_interest.value(),
                r.closing_balance.value()
            ));
        }
        
        csv
    }
}

/// Xuất JSON
pub struct JsonExporter;

impl ReportExporter for JsonExporter {
    fn export(&self, results: &[YearlySimulationResult]) -> String {
        let mut json = String::from("[\n");
        
        for (i, r) in results.iter().enumerate() {
            json.push_str(&format!(
                r#"  {{
    "year": {},
    "opening_balance": {:.2},
    "fee_charged": {:.2},
    "interest_earned": {:.2},
    "tax_paid": {:.2},
    "net_interest": {:.2},
    "closing_balance": {:.2}
  }}"#,
                r.year,
                r.opening_balance.value(),
                r.fee_charged.value(),
                r.interest_earned.value(),
                r.tax_paid.value(),
                r.net_interest.value(),
                r.closing_balance.value()
            ));
            
            if i < results.len() - 1 {
                json.push_str(",\n");
            } else {
                json.push('\n');
            }
        }
        
        json.push(']');
        json
    }
}

/// Xuất Markdown
pub struct MarkdownExporter;

impl ReportExporter for MarkdownExporter {
    fn export(&self, results: &[YearlySimulationResult]) -> String {
        let mut md = String::new();
        md.push_str("# Báo cáo Mô phỏng Tài chính\n\n");
        md.push_str("| Năm | Số dư đầu kỳ | Phí | Lãi | Thuế | Lãi ròng | Số dư cuối kỳ |\n");
        md.push_str("|-----|--------------|-----|-----|------|----------|---------------|\n");
        
        for r in results {
            md.push_str(&format!(
                "| {} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2} |\n",
                r.year,
                r.opening_balance.value(),
                r.fee_charged.value(),
                r.interest_earned.value(),
                r.tax_paid.value(),
                r.net_interest.value(),
                r.closing_balance.value()
            ));
        }
        
        md
    }
}
