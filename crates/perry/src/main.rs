//! Perry - Native TypeScript Compiler
//!
//! CLI driver for compiling TypeScript to native executables.

mod commands;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};

/// Native TypeScript Compiler
#[derive(Parser, Debug)]
#[command(name = "perry")]
#[command(author, version, about = "Compile TypeScript to native executables")]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Output format
    #[arg(long, global = true, default_value = "text")]
    format: OutputFormat,

    /// Increase verbosity (-v, -vv, -vvv)
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    verbose: u8,

    /// Suppress non-error output
    #[arg(short, long, global = true)]
    quiet: bool,

    /// Disable colored output
    #[arg(long, global = true)]
    no_color: bool,
}

#[derive(Clone, Copy, Debug, Default, ValueEnum)]
pub enum OutputFormat {
    #[default]
    Text,
    Json,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Compile TypeScript file(s) to native executable
    Compile(commands::compile::CompileArgs),

    /// Check TypeScript compatibility without compiling
    Check(commands::check::CheckArgs),

    /// Initialize a new perry project
    Init(commands::init::InitArgs),

    /// Check environment and dependencies
    Doctor(commands::doctor::DoctorArgs),

    /// Explain an error code
    Explain(commands::explain::ExplainArgs),
}

/// Check if the first non-flag argument looks like a TypeScript file
fn is_legacy_invocation(args: &[String]) -> bool {
    for arg in args.iter().skip(1) {
        // Skip flags
        if arg.starts_with('-') {
            continue;
        }
        // Check if it looks like a .ts file (and not a subcommand)
        if arg.ends_with(".ts") {
            return true;
        }
        // If it's a known subcommand, not legacy
        if matches!(
            arg.as_str(),
            "compile" | "check" | "init" | "doctor" | "explain" | "help"
        ) {
            return false;
        }
        // First non-flag, non-subcommand arg
        break;
    }
    false
}

/// Transform legacy args (perry file.ts -o out) to subcommand form
fn transform_legacy_args(args: Vec<String>) -> Vec<String> {
    let mut new_args = vec![args[0].clone(), "compile".to_string()];
    new_args.extend(args.into_iter().skip(1));
    new_args
}

fn main() -> Result<()> {
    env_logger::init();

    // Handle legacy invocation (perry file.ts -o out)
    let args: Vec<String> = std::env::args().collect();
    let effective_args = if is_legacy_invocation(&args) {
        transform_legacy_args(args)
    } else {
        args
    };

    let cli = Cli::parse_from(effective_args);

    // Determine if colors should be used
    let use_color = !cli.no_color && !cli.quiet && atty::is(atty::Stream::Stdout);

    // Handle no command case
    if cli.command.is_none() {
        let mut cmd = <Cli as clap::CommandFactory>::command();
        cmd.print_help()?;
        println!();
        return Ok(());
    }

    match cli.command.unwrap() {
        Commands::Compile(args) => {
            commands::compile::run(args, cli.format, use_color, cli.verbose)
        }
        Commands::Check(args) => {
            commands::check::run(args, cli.format, use_color, cli.verbose)
        }
        Commands::Init(args) => {
            commands::init::run(args, cli.format, use_color)
        }
        Commands::Doctor(args) => {
            commands::doctor::run(args, cli.format, use_color)
        }
        Commands::Explain(args) => {
            commands::explain::run(args, cli.format, use_color)
        }
    }
}
