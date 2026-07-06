use clap::{Parser, ValueEnum};
use std::path::PathBuf;

use bevy_i18n_lint::{run, CliOptions, OutputFormat};

#[derive(Clone, Debug, ValueEnum)]
enum CliOutputFormat {
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
    #[arg(short = 'd', long, default_value = "assets/i18n")]
    dir: PathBuf,

    #[arg(short = 'b', long, default_value = "en")]
    base: String,

    #[arg(short = 's', long)]
    strict: bool,

    #[arg(short = 'f', long, value_enum, default_value = "text")]
    format: CliOutputFormat,

    #[arg(short = 'e', long)]
    fail_on_extra: bool,

    #[arg(short = 'p', long)]
    fail_on_placeholder: bool,

    #[arg(long)]
    config: Option<PathBuf>,

    #[arg(long)]
    no_ignore: bool,

    #[arg(long)]
    init: bool,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if cli.init {
        bevy_i18n_lint::generate_config()?;
        println!("Created bevy-i18n-lint.toml");
        return Ok(());
    }

    let format = match cli.format {
        CliOutputFormat::Text => OutputFormat::Text,
        CliOutputFormat::Json => OutputFormat::Json,
        CliOutputFormat::Github => OutputFormat::Github,
    };

    let code = run(CliOptions {
        dir: cli.dir,
        base: cli.base,
        strict: cli.strict,
        format,
        fail_on_extra: cli.fail_on_extra,
        fail_on_placeholder: cli.fail_on_placeholder,
        config_path: cli.config,
        no_ignore: cli.no_ignore,
    })?;

    std::process::exit(code);
}
