//! Doctor command - check environment and dependencies

use anyhow::Result;
use clap::Args;
use console::{style, Emoji};
use std::path::PathBuf;
use std::process::Command;

use crate::OutputFormat;

#[derive(Args, Debug)]
pub struct DoctorArgs {
    /// Run checks silently and only report failures
    #[arg(long)]
    pub quiet: bool,
}

static CHECK: Emoji<'_, '_> = Emoji("✓ ", "[OK] ");
static CROSS: Emoji<'_, '_> = Emoji("✗ ", "[FAIL] ");
static WARN: Emoji<'_, '_> = Emoji("⚠ ", "[WARN] ");

struct CheckResult {
    name: String,
    status: CheckStatus,
    details: Option<String>,
}

enum CheckStatus {
    Ok,
    Warning,
    Error,
}

fn check_perry_version() -> CheckResult {
    CheckResult {
        name: "perry version".to_string(),
        status: CheckStatus::Ok,
        details: Some(env!("CARGO_PKG_VERSION").to_string()),
    }
}

fn check_system_linker() -> CheckResult {
    let output = Command::new("cc").arg("--version").output();

    match output {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout);
            let first_line = version.lines().next().unwrap_or("unknown");
            CheckResult {
                name: "system linker (cc)".to_string(),
                status: CheckStatus::Ok,
                details: Some(first_line.to_string()),
            }
        }
        Ok(_) => CheckResult {
            name: "system linker (cc)".to_string(),
            status: CheckStatus::Error,
            details: Some("cc command failed".to_string()),
        },
        Err(e) => CheckResult {
            name: "system linker (cc)".to_string(),
            status: CheckStatus::Error,
            details: Some(format!("cc not found: {}", e)),
        },
    }
}

fn check_runtime_library() -> CheckResult {
    let candidates = [
        PathBuf::from("target/release/libperry_runtime.a"),
        PathBuf::from("target/debug/libperry_runtime.a"),
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.join("libperry_runtime.a")))
            .unwrap_or_default(),
        PathBuf::from("/usr/local/lib/libperry_runtime.a"),
    ];

    for path in &candidates {
        if path.exists() {
            return CheckResult {
                name: "runtime library".to_string(),
                status: CheckStatus::Ok,
                details: Some(path.display().to_string()),
            };
        }
    }

    CheckResult {
        name: "runtime library".to_string(),
        status: CheckStatus::Warning,
        details: Some("not found - run: cargo build --release -p perry-runtime".to_string()),
    }
}

fn check_project_config() -> CheckResult {
    let config_path = PathBuf::from("perry.toml");
    if config_path.exists() {
        CheckResult {
            name: "project config (perry.toml)".to_string(),
            status: CheckStatus::Ok,
            details: Some("found".to_string()),
        }
    } else {
        CheckResult {
            name: "project config (perry.toml)".to_string(),
            status: CheckStatus::Warning,
            details: Some("not found - run: perry init".to_string()),
        }
    }
}

pub fn run(args: DoctorArgs, format: OutputFormat, use_color: bool) -> Result<()> {
    let checks = vec![
        check_perry_version(),
        check_system_linker(),
        check_runtime_library(),
        check_project_config(),
    ];

    let mut has_errors = false;
    let mut has_warnings = false;

    match format {
        OutputFormat::Text => {
            if !args.quiet {
                println!("Perry Doctor\n");
                println!("Environment Checks");
                println!("──────────────────");
            }

            for check in &checks {
                let (emoji, color_fn): (_, fn(&str) -> console::StyledObject<&str>) = match check.status {
                    CheckStatus::Ok => (CHECK, |s| style(s).green()),
                    CheckStatus::Warning => {
                        has_warnings = true;
                        (WARN, |s| style(s).yellow())
                    }
                    CheckStatus::Error => {
                        has_errors = true;
                        (CROSS, |s| style(s).red())
                    }
                };

                let status_str = match check.status {
                    CheckStatus::Ok => "OK",
                    CheckStatus::Warning => "WARN",
                    CheckStatus::Error => "FAIL",
                };

                if args.quiet && matches!(check.status, CheckStatus::Ok) {
                    continue;
                }

                if use_color {
                    print!("  {}{}: ", emoji, check.name);
                    if let Some(ref details) = check.details {
                        println!("{}", color_fn(details));
                    } else {
                        println!("{}", color_fn(status_str));
                    }
                } else {
                    print!("  [{}] {}: ", status_str, check.name);
                    if let Some(ref details) = check.details {
                        println!("{}", details);
                    } else {
                        println!();
                    }
                }
            }

            if !args.quiet {
                println!();
                if has_errors {
                    println!("Some checks failed. Please fix the issues above.");
                } else if has_warnings {
                    println!("All critical checks passed with some warnings.");
                } else {
                    println!("All checks passed!");
                }
            }
        }
        OutputFormat::Json => {
            let results: Vec<_> = checks
                .iter()
                .map(|c| {
                    serde_json::json!({
                        "name": c.name,
                        "status": match c.status {
                            CheckStatus::Ok => "ok",
                            CheckStatus::Warning => "warning",
                            CheckStatus::Error => "error",
                        },
                        "details": c.details,
                    })
                })
                .collect();

            let output = serde_json::json!({
                "success": !has_errors,
                "checks": results,
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
    }

    if has_errors {
        std::process::exit(1);
    }

    Ok(())
}
