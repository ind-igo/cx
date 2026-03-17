mod index;
mod output;
mod grep;
mod query;
mod language;
mod util;

use clap::{Parser, Subcommand};
use std::env;
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

fn resolve_root(project: Option<PathBuf>) -> PathBuf {
    match project {
        Some(p) => p,
        None => {
            let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            util::git::find_project_root(&cwd)
        }
    }
}

fn main() {
    let cli = Cli::parse();
    let root = resolve_root(cli.project);

    let exit_code = match cli.command {
        Commands::Overview { file } => {
            let idx = index::Index::load_or_build(&root);
            query::symbols(&idx, Some(&file), None, None, true, cli.json)
        }
        Commands::Symbols { file, name, kind } => {
            let idx = index::Index::load_or_build(&root);
            let kind_filter = kind.as_deref().and_then(index::SymbolKind::from_str);
            query::symbols(&idx, file.as_deref(), name.as_deref(), kind_filter, file.is_some(), cli.json)
        }
        Commands::Definition { name, from, max_lines } => {
            let _index = index::Index::load_or_build(&root);
            eprintln!("cx definition: not implemented yet");
            let _ = (name, from, max_lines);
            1
        }
        Commands::Read { file, fresh } => {
            let _index = index::Index::load_or_build(&root);
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
