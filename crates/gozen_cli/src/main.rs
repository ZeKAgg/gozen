use clap::{Parser, Subcommand};
use std::path::PathBuf;

pub mod cache;
mod commands;
mod discovery;
mod reporters;

use reporters::Reporter;

#[derive(Parser)]
#[command(
    name = "gozen",
    version,
    about = "Code quality for GDScript and GDShader. Lint, format, analyze. From the terminal."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Format(FormatArgs),
    Lint(LintArgs),
    Check(CheckArgs),
    Ci(CiArgs),
    Init(InitArgs),
    Explain(ExplainArgs),
    Baseline(BaselineArgs),
    Lsp(LspArgs),
    Migrate(MigrateArgs),
}

#[derive(clap::Args)]
struct FormatArgs {
    #[arg(default_value = ".")]
    paths: Vec<PathBuf>,
    /// Check if files are formatted without writing changes.
    #[arg(long)]
    check: bool,
    /// Show a unified diff of formatting changes without writing.
    #[arg(long)]
    diff: bool,
    /// List each file and whether it was changed.
    #[arg(long)]
    verbose: bool,
    /// Suppress all output except errors.
    #[arg(long)]
    quiet: bool,
    #[arg(long)]
    stdin_filepath: Option<PathBuf>,
    #[arg(long)]
    line_width: Option<usize>,
    #[arg(long, default_value = "text")]
    reporter: Reporter,
    #[arg(long)]
    config: Option<PathBuf>,
}

#[derive(clap::Args)]
struct LintArgs {
    #[arg(default_value = ".")]
    paths: Vec<PathBuf>,
    #[arg(long)]
    write: bool,
    /// Suppress all output except errors.
    #[arg(long)]
    quiet: bool,
    #[arg(long, default_value = "text")]
    reporter: Reporter,
    #[arg(long)]
    config: Option<PathBuf>,
    #[arg(long, default_value = "50")]
    max_diagnostics: usize,
}

#[derive(clap::Args)]
struct CheckArgs {
    #[arg(default_value = ".")]
    paths: Vec<PathBuf>,
    #[arg(long)]
    write: bool,
    /// Suppress all output except errors.
    #[arg(long)]
    quiet: bool,
    #[arg(long, default_value = "text")]
    reporter: Reporter,
    #[arg(long)]
    config: Option<PathBuf>,
    #[arg(long, default_value = "50")]
    max_diagnostics: usize,
}

#[derive(clap::Args)]
struct CiArgs {
    #[arg(default_value = ".")]
    paths: Vec<PathBuf>,
    #[arg(long)]
    changed: bool,
    #[arg(long)]
    baseline: Option<PathBuf>,
    #[arg(long, default_value = "text")]
    reporter: Reporter,
    #[arg(long)]
    config: Option<PathBuf>,
    #[arg(long, default_value = "50")]
    max_diagnostics: usize,
}

#[derive(clap::Args)]
struct InitArgs {
    #[arg(long)]
    force: bool,
}

#[derive(clap::Args)]
struct ExplainArgs {
    rule: String,
}

#[derive(clap::Args)]
struct BaselineArgs {
    #[arg(long)]
    create: bool,
    #[arg(long, default_value = "gozen-baseline.json")]
    output: PathBuf,
    #[arg(default_value = ".")]
    paths: Vec<PathBuf>,
    #[arg(long)]
    config: Option<PathBuf>,
}

#[derive(clap::Args)]
struct LspArgs {}

#[derive(clap::Args)]
struct MigrateArgs {
    /// Source config format to migrate from (e.g., "gdlintrc").
    #[arg(long)]
    from: String,
}

fn main() {
    if let Err(e) = run() {
        eprintln!("{}", e);
        std::process::exit(2);
    }
}

fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let start_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let config = match cli.config_file() {
        Some(p) => gozen_config::load_config_from_path(&p).unwrap_or_else(|e| {
            eprintln!("warning: could not load config {}: {}", p.display(), e);
            Default::default()
        }),
        None => gozen_config::load_config(&start_dir).unwrap_or_else(|e| {
            eprintln!("warning: could not load config: {}", e);
            Default::default()
        }),
    };

    let exit_ok = match cli.command {
        Command::Format(a) => {
            commands::format::run(
                &a.paths,
                a.check || a.diff,
                &config,
                &start_dir,
                a.stdin_filepath,
                a.verbose,
                a.diff,
                a.quiet,
            )?
            .0
        }
        Command::Lint(a) => commands::lint::run(
            &a.paths,
            a.write,
            &config,
            a.max_diagnostics,
            a.reporter,
            0,
            &start_dir,
            a.quiet,
        )?,
        Command::Check(a) => commands::check::run(
            &a.paths,
            a.write,
            &config,
            &start_dir,
            a.max_diagnostics,
            a.reporter,
            a.quiet,
        )?,
        Command::Ci(a) => commands::ci::run(
            &a.paths,
            a.changed,
            a.baseline.clone(),
            &config,
            &start_dir,
            a.max_diagnostics,
            a.reporter,
        )?,
        Command::Init(a) => {
            commands::init::run(a.force, &start_dir)?;
            true
        }
        Command::Explain(a) => {
            commands::explain::run(&a.rule)?;
            true
        }
        Command::Baseline(a) => {
            commands::baseline::run(&a.paths, a.create, &a.output, &config, &start_dir)?
        }
        Command::Lsp(_) => {
            let rt = tokio::runtime::Runtime::new()
                .map_err(|e| anyhow::anyhow!("Failed to create async runtime: {}", e))?;
            rt.block_on(gozen_lsp::run_stdio(config));
            true
        }
        Command::Migrate(a) => {
            commands::migrate::run(&a.from, &start_dir)?;
            true
        }
    };

    if !exit_ok {
        std::process::exit(1);
    }
    Ok(())
}

trait ConfigFile {
    fn config_file(&self) -> Option<PathBuf>;
}

impl ConfigFile for Cli {
    fn config_file(&self) -> Option<PathBuf> {
        match &self.command {
            Command::Format(a) => a.config.clone(),
            Command::Lint(a) => a.config.clone(),
            Command::Check(a) => a.config.clone(),
            Command::Ci(a) => a.config.clone(),
            Command::Baseline(a) => a.config.clone(),
            _ => None,
        }
    }
}
