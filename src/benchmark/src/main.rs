extern crate criterion;
extern crate plotters;

use criterion::{criterion_group, criterion_main, Criterion};
use plotters::prelude::*;

// Hàm để tính tổng của một dãy số từ 0 đến n
fn calculate_sum(n: u64) -> u64 {
    (0..=n).sum()
}

// Hàm benchmark
fn bench_sum(_c: &mut Criterion) -> Result<(), Box<dyn std::error::Error>> {
    // Tạo một biểu đồ plotters để vẽ kết quả
    let root = BitMapBackend::new("benchmark.png", (800, 600)).into_drawing_area();
    root.fill(&WHITE)?;

    // Khởi tạo một biểu đồ thể hiện thời gian thực thi
    let mut chart = ChartBuilder::on(&root)
        .caption("Sum Benchmark", ("sans-serif", 40).into_font())
        .margin(10)
        .x_label_area_size(40)
        .y_label_area_size(40)
        .build_cartesian_2d(0u64..10u64, 0u64..10u64)?;

    // Đo thời gian và vẽ kết quả lên biểu đồ
    chart.draw_series(LineSeries::new(
        (0u64..10u64).map(|n| (n, calculate_sum(n))),
        &BLUE,
    ))?;

    // Lưu biểu đồ vào tệp PNG
    chart
        .configure_series_labels()
        .background_style(&WHITE.mix(0.8))
        .border_style(&BLACK)
        .draw()?;
    Ok(())
}

// Đăng ký benchmark cho chương trình
criterion_group!(benches, bench_sum);
criterion_main!(benches);
