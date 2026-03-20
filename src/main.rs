mod index;
mod output;
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
    root: Option<PathBuf>,

    /// Emit JSON instead of TOON
    #[arg(long, global = true)]
    json: bool,

}

#[derive(Subcommand)]
enum Commands {
    /// File table of contents — all symbols + signatures
    #[command(alias = "o")]
    Overview {
        /// File to summarize
        file: PathBuf,
    },
    /// Search symbols across project
    #[command(alias = "s")]
    Symbols {
        /// Filter to a specific file
        #[arg(long)]
        file: Option<PathBuf>,
        /// Glob pattern to match symbol names
        #[arg(long)]
        name: Option<String>,
        /// Filter by symbol kind (fn, struct, enum, trait, type, const, class, interface, method, module, event)
        #[arg(long)]
        kind: Option<String>,
    },
    /// Get a function/type body without reading the whole file
    #[command(alias = "d")]
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
    /// Find all usages of a symbol across the project
    #[command(alias = "r")]
    References {
        /// Symbol name to find
        #[arg(long)]
        name: String,
        /// Limit search to a specific file
        #[arg(long)]
        file: Option<PathBuf>,
    },
    /// Print the agent skill file to stdout
    Skill,
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
    let root = resolve_root(cli.root);

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
            let idx = index::Index::load_or_build(&root);
            query::definition(&idx, &name, from.as_deref(), max_lines, cli.json)
        }
        Commands::References { name, file } => {
            let idx = index::Index::load_or_build(&root);
            query::references(&idx, &name, file.as_deref(), cli.json)
        }
        Commands::Skill => {
            print!("{}", include_str!("skill.md"));
            0
        }
    };

    process::exit(exit_code);
}
