mod index;
mod lang;
mod output;
mod query;
mod language;
mod util;

use clap::{Parser, Subcommand};
use std::env;
use std::path::PathBuf;
use std::process;

#[derive(Parser)]
#[command(name = "cx", version, about = "Semantic code navigation for AI agents")]
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
        /// Filter by symbol kind
        #[arg(long)]
        kind: Option<index::SymbolKind>,
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
        /// Filter by symbol kind
        #[arg(long)]
        kind: Option<index::SymbolKind>,
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
    /// Manage language grammars
    Lang {
        #[command(subcommand)]
        action: LangAction,
    },
    /// Print the agent skill file to stdout
    Skill,
}

#[derive(Subcommand)]
enum LangAction {
    /// Download and install language grammars
    Add {
        /// Language names (e.g. rust typescript python)
        languages: Vec<String>,
    },
    /// Remove installed language grammars
    Remove {
        /// Language names to remove
        languages: Vec<String>,
    },
    /// List supported languages and their install status
    List,
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

pub fn grammar_cache_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from(".cache"))
        .join("cx")
        .join("grammars")
}

fn main() {
    // Use our own cache directory so grammar downloads survive crate version bumps.
    // See KNOWN_ISSUES.md for details.
    let config = tree_sitter_language_pack::PackConfig {
        cache_dir: Some(grammar_cache_dir()),
        ..Default::default()
    };
    if let Err(e) = tree_sitter_language_pack::configure(&config) {
        eprintln!("cx: failed to configure grammar cache: {}", e);
    }

    let cli = Cli::parse();
    let root = resolve_root(cli.root);

    let exit_code = match cli.command {
        Commands::Overview { file } => {
            let idx = index::Index::load_or_build(&root);
            query::symbols(&idx, Some(&file), None, None, cli.json)
        }
        Commands::Symbols { file, name, kind } => {
            let idx = index::Index::load_or_build(&root);
            query::symbols(&idx, file.as_deref(), name.as_deref(), kind, cli.json)
        }
        Commands::Definition { name, from, kind, max_lines } => {
            let idx = index::Index::load_or_build(&root);
            query::definition(&idx, &name, from.as_deref(), kind, max_lines, cli.json)
        }
        Commands::References { name, file } => {
            let idx = index::Index::load_or_build(&root);
            query::references(&idx, &name, file.as_deref(), cli.json)
        }
        Commands::Lang { action } => {
            match action {
                LangAction::Add { languages } => lang::add(&languages),
                LangAction::Remove { languages } => lang::remove(&languages),
                LangAction::List => lang::list(),
            }
        }
        Commands::Skill => {
            print!("{}", include_str!("skill.md"));
            0
        }
    };

    process::exit(exit_code);
}
