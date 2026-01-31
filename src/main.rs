// File: src/main.rs
use clap::{Parser, ValueEnum};
use std::path::PathBuf;

use bevy_i18n_lint::{run, CliOptions};

#[derive(Clone, Debug, ValueEnum)]
enum OutputFormat {
    Text,
    Json,
    Github,
}

#[derive(Parser, Debug)]
#[command(name = "bevy-i18n-lint")]
#[command(version)]
#[command(
    about = "Lint Bevy localization files (json/ron): missing keys, extra keys, placeholder mismatches."
)]
struct Cli {
    #[arg(long, default_value = "assets/i18n")]
    dir: PathBuf,

    #[arg(long, default_value = "en")]
    base: String,

    #[arg(long)]
    strict: bool,

    #[arg(long, value_enum, default_value = "text")]
    format: OutputFormat,

    #[arg(long)]
    fail_on_extra: bool,

    #[arg(long)]
    fail_on_placeholder: bool,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let format = match cli.format {
        OutputFormat::Text => "text",
        OutputFormat::Json => "json",
        OutputFormat::Github => "github",
    };

    let code = run(CliOptions {
        dir: cli.dir,
        base: cli.base,
        strict: cli.strict,
        format: format.to_string(),
        fail_on_extra: cli.fail_on_extra,
        fail_on_placeholder: cli.fail_on_placeholder,
    })?;

    std::process::exit(code);
}
