mod index;
mod output;
mod grep;
mod query;
mod language;
mod util;

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process;

#[derive(Parser)]
#[command(name = "cx", about = "Semantic code navigation for AI agents")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Project root (default: git root from cwd, then cwd)
    #[arg(long, global = true)]
    project: Option<PathBuf>,

    /// Emit JSON instead of TOON
    #[arg(long, global = true)]
    json: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// File table of contents — all symbols + signatures
    Overview {
        /// File to summarize
        file: PathBuf,
    },
    /// Search symbols across project
    Symbols {
        /// Filter to a specific file
        #[arg(long)]
        file: Option<PathBuf>,
        /// Glob pattern to match symbol names
        #[arg(long)]
        name: Option<String>,
        /// Filter by symbol kind (fn, struct, enum, trait, type, const, class, interface, method, module)
        #[arg(long)]
        kind: Option<String>,
    },
    /// Get a function/type body without reading the whole file
    Definition {
        /// Symbol name to look up
        #[arg(long)]
        name: String,
        /// Disambiguate by source file
        #[arg(long)]
        from: Option<PathBuf>,
        /// Max lines for body output (default 200)
        #[arg(long, default_value = "200")]
        max_lines: usize,
    },
    /// Full file read with session cache
    Read {
        /// File to read
        file: PathBuf,
        /// Always return full content, bypass cache
        #[arg(long)]
        fresh: bool,
    },
    /// grep-compatible passthrough to rg
    Grep {
        /// Arguments passed through to rg/grep
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}

fn main() {
    let cli = Cli::parse();

    let exit_code = match cli.command {
        Commands::Overview { file } => {
            eprintln!("cx overview: not implemented yet");
            let _ = file;
            1
        }
        Commands::Symbols { file, name, kind } => {
            eprintln!("cx symbols: not implemented yet");
            let _ = (file, name, kind);
            1
        }
        Commands::Definition { name, from, max_lines } => {
            eprintln!("cx definition: not implemented yet");
            let _ = (name, from, max_lines);
            1
        }
        Commands::Read { file, fresh } => {
            eprintln!("cx read: not implemented yet");
            let _ = (file, fresh);
            1
        }
        Commands::Grep { args } => {
            grep::run(&args)
        }
    };

    process::exit(exit_code);
}
