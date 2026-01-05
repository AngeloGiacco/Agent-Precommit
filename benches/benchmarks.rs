//! Benchmarks for agent-precommit.

#![allow(missing_docs)]
#![allow(let_underscore_drop)]

use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn benchmark_mode_detection(c: &mut Criterion) {
    c.bench_function("mode_detection", |b| {
        b.iter(|| {
            // Simple benchmark placeholder
            // In a real benchmark, we'd test the detector
            black_box(1 + 1)
        });
    });
}

fn benchmark_config_parsing(c: &mut Criterion) {
    let toml_content = r#"
[human]
checks = ["pre-commit"]
timeout = "30s"

[agent]
checks = ["pre-commit-all", "test-unit"]
timeout = "15m"
"#;

    c.bench_function("config_parsing", |b| {
        b.iter(|| {
            let result: toml::Value =
                toml::from_str(black_box(toml_content)).expect("parse config");
            black_box(result)
        });
    });
}

criterion_group!(benches, benchmark_mode_detection, benchmark_config_parsing);
criterion_main!(benches);
