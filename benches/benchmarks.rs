//! Benchmarks for agent-precommit.

#![allow(missing_docs)]
#![allow(let_underscore_drop)]

use agent_precommit::{Config, Detector, Mode};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

/// Benchmark the full mode detection pipeline.
///
/// This tests the `Detector::detect()` method which checks:
/// - APC_MODE environment variable
/// - AGENT_MODE flag
/// - Known agent environment variables (20+ vars)
/// - Custom agent environment variables from config
/// - CI environment variables (20+ vars)
/// - TTY detection
fn benchmark_mode_detection(c: &mut Criterion) {
    let config = Config::default();

    c.bench_function("detector_detect", |b| {
        b.iter(|| {
            let detector = Detector::new(black_box(&config));
            black_box(detector.detect())
        });
    });
}

/// Benchmark mode detection with custom agent environment variables configured.
fn benchmark_mode_detection_with_custom_vars(c: &mut Criterion) {
    let mut config = Config::default();
    // Add custom agent environment variables to simulate real-world config
    config.detection.agent_env_vars = vec![
        "MY_CUSTOM_AGENT".to_string(),
        "INTERNAL_BOT_TOKEN".to_string(),
        "AUTOMATION_SERVICE".to_string(),
    ];

    c.bench_function("detector_detect_with_custom_vars", |b| {
        b.iter(|| {
            let detector = Detector::new(black_box(&config));
            black_box(detector.detect())
        });
    });
}

/// Benchmark Mode::from_str parsing for different inputs.
fn benchmark_mode_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("mode_parsing");

    let test_cases = ["human", "agent", "ci", "HUMAN", "Agent", "CI"];

    for input in test_cases {
        group.bench_with_input(BenchmarkId::from_parameter(input), input, |b, input| {
            b.iter(|| {
                let result: Result<Mode, _> = black_box(input).parse();
                black_box(result)
            });
        });
    }

    group.finish();
}

/// Benchmark Mode helper methods.
fn benchmark_mode_methods(c: &mut Criterion) {
    let mut group = c.benchmark_group("mode_methods");

    let modes = [Mode::Human, Mode::Agent, Mode::Ci];

    for mode in modes {
        group.bench_with_input(BenchmarkId::new("name", mode.name()), &mode, |b, mode| {
            b.iter(|| black_box(mode.name()));
        });

        group.bench_with_input(
            BenchmarkId::new("is_thorough", mode.name()),
            &mode,
            |b, mode| {
                b.iter(|| black_box(mode.is_thorough()));
            },
        );

        group.bench_with_input(
            BenchmarkId::new("display", mode.name()),
            &mode,
            |b, mode| {
                b.iter(|| black_box(mode.to_string()));
            },
        );
    }

    group.finish();
}

/// Benchmark config parsing with the actual Config struct.
fn benchmark_config_parsing(c: &mut Criterion) {
    let toml_content = r#"
[detection]
agent_env_vars = ["MY_AGENT", "CUSTOM_BOT"]

[human]
checks = ["pre-commit"]
timeout = "30s"
fail_fast = true

[agent]
checks = ["pre-commit-all", "test-unit", "build-verify"]
timeout = "15m"
fail_fast = false

[checks.custom-lint]
run = "npm run lint"
description = "Run ESLint"

[checks.custom-lint.enabled_if]
file_exists = "package.json"
"#;

    c.bench_function("config_parsing_full", |b| {
        b.iter(|| {
            let result: Config = toml::from_str(black_box(toml_content)).expect("parse config");
            black_box(result)
        });
    });
}

/// Benchmark config parsing for minimal config.
fn benchmark_config_parsing_minimal(c: &mut Criterion) {
    let toml_content = r#"
[human]
checks = ["pre-commit"]
timeout = "30s"

[agent]
checks = ["pre-commit-all"]
timeout = "15m"
"#;

    c.bench_function("config_parsing_minimal", |b| {
        b.iter(|| {
            let result: Config = toml::from_str(black_box(toml_content)).expect("parse config");
            black_box(result)
        });
    });
}

/// Benchmark config default generation.
fn benchmark_config_default(c: &mut Criterion) {
    c.bench_function("config_default", |b| {
        b.iter(|| black_box(Config::default()));
    });
}

/// Benchmark config preset generation.
fn benchmark_config_presets(c: &mut Criterion) {
    let mut group = c.benchmark_group("config_presets");

    let presets = ["python", "rust", "node", "go"];

    for preset in presets {
        group.bench_with_input(BenchmarkId::from_parameter(preset), preset, |b, preset| {
            b.iter(|| black_box(Config::for_preset(black_box(preset))));
        });
    }

    group.finish();
}

/// Benchmark config validation.
fn benchmark_config_validation(c: &mut Criterion) {
    let config = Config::default();

    c.bench_function("config_validation", |b| {
        b.iter(|| black_box(config.validate()));
    });
}

criterion_group!(
    benches,
    benchmark_mode_detection,
    benchmark_mode_detection_with_custom_vars,
    benchmark_mode_parsing,
    benchmark_mode_methods,
    benchmark_config_parsing,
    benchmark_config_parsing_minimal,
    benchmark_config_default,
    benchmark_config_presets,
    benchmark_config_validation,
);
criterion_main!(benches);
