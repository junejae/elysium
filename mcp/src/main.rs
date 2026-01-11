mod commands;
mod core;
#[cfg(feature = "mcp")]
mod mcp;
mod search;
mod tags;

use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(name = "elysium")]
#[command(about = "MCP server for Obsidian-based Second Brain with AI-powered semantic search", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    // ===== MCP Server (also default) =====
    /// Start MCP server for Claude integration
    #[cfg(feature = "mcp")]
    Mcp {
        #[arg(long, help = "Show Claude configuration instructions")]
        install: bool,
    },

    // ===== Core Commands =====
    Init {
        #[arg(long, help = "Generate .elysium.json config file")]
        config: bool,
        #[arg(long, help = "Path to inbox file (default: inbox.md)")]
        inbox: Option<String>,
    },
    Validate {
        #[arg(long, help = "Check YAML schema only")]
        schema: bool,
        #[arg(long, help = "Check wikilinks only")]
        wikilinks: bool,
        #[arg(long, help = "JSON output")]
        json: bool,
    },
    Audit {
        #[arg(short, long, help = "Quick mode (schema + wikilinks only)")]
        quick: bool,
        #[arg(long, help = "JSON output")]
        json: bool,
        #[arg(long, help = "Exit 1 on violations")]
        strict: bool,
    },
    Status {
        #[arg(short, long, help = "Brief output")]
        brief: bool,
        #[arg(long, help = "JSON output")]
        json: bool,
    },
    Health {
        #[arg(short, long, help = "Show detailed breakdown")]
        details: bool,
        #[arg(long, help = "JSON output")]
        json: bool,
    },
    Search {
        query: String,
        #[arg(long, help = "Search in gist only")]
        gist: bool,
        #[arg(long, help = "Limit results")]
        limit: Option<usize>,
    },
    Related {
        note: String,
        #[arg(long, short, help = "Use semantic search (gist-based AI similarity)")]
        semantic: bool,
        #[arg(long, help = "Minimum shared tags (tag mode only)")]
        min_tags: Option<usize>,
        #[arg(long, short, help = "Limit results (semantic mode)")]
        limit: Option<usize>,
        #[arg(long, help = "Boost notes with same type (semantic mode)")]
        boost_type: bool,
        #[arg(long, help = "Boost notes with same area (semantic mode)")]
        boost_area: bool,
        #[arg(long, help = "JSON output")]
        json: bool,
    },
    Tags {
        #[command(subcommand)]
        action: Option<TagsAction>,
    },
    Fix {
        #[arg(long, help = "Fix broken wikilinks")]
        wikilinks: bool,
        #[arg(long, help = "Actually apply fixes (default: dry-run)")]
        execute: bool,
        #[arg(long, help = "JSON output")]
        json: bool,
    },
    /// Connect orphan notes to related notes via wikilinks
    Connect {
        #[arg(long, help = "Actually apply connections (default: dry-run)")]
        execute: bool,
        #[arg(long, help = "Minimum shared tags for connection (default: 1)")]
        min_tags: Option<usize>,
        #[arg(short, long, help = "Use semantic search instead of tags")]
        semantic: bool,
        #[arg(short, long, help = "Max connections per orphan (default: 5)")]
        limit: Option<usize>,
        #[arg(long, help = "JSON output")]
        json: bool,
    },

    // ===== Semantic Search =====
    /// Build semantic search index
    Index {
        #[arg(long, help = "Show index status only")]
        status: bool,
        #[arg(long, help = "Force rebuild index")]
        rebuild: bool,
        #[arg(long, help = "JSON output")]
        json: bool,
    },
    /// Semantic search using AI embeddings
    #[command(name = "semantic-search", alias = "ss")]
    SemanticSearch {
        query: String,
        #[arg(long, short, help = "Limit results")]
        limit: Option<usize>,
        #[arg(long, help = "JSON output")]
        json: bool,
        #[arg(long, help = "Use simple string search (no AI)")]
        fallback: bool,
    },

    // ===== Model Management =====
    /// Manage Model2Vec models for advanced semantic search
    Model {
        /// Subcommand: download, status
        #[arg(default_value = "status")]
        action: String,
        #[arg(long, help = "JSON output")]
        json: bool,
    },
}

/// Tag management subcommands
#[derive(Subcommand)]
enum TagsAction {
    /// Analyze tag usage (default)
    #[command(alias = "analyze")]
    List {
        #[arg(short, long, help = "Analyze tags and suggest improvements")]
        analyze: bool,
        #[arg(long, help = "JSON output")]
        json: bool,
    },
    /// Initialize tag database with seed tags
    Init {
        #[arg(long, help = "Force reinitialize (clear existing)")]
        force: bool,
    },
    /// Suggest tags for given text
    Suggest {
        /// Text to analyze (gist or title)
        text: String,
        #[arg(short, long, default_value = "5", help = "Number of suggestions")]
        limit: usize,
        #[arg(
            short,
            long,
            help = "Enable tag discovery from keywords (not just DB match)"
        )]
        discover: bool,
        #[arg(long, help = "JSON output")]
        json: bool,
    },
    /// Sync tags for all notes based on gist
    Sync {
        #[arg(long, help = "Actually apply changes (default: dry-run)")]
        execute: bool,
        #[arg(
            short,
            long,
            help = "Enable tag discovery from keywords (not just DB match)"
        )]
        discover: bool,
        #[arg(long, help = "JSON output")]
        json: bool,
    },
    /// Extract tags from existing notes to build tag database
    Extract {
        #[arg(long, default_value = "2", help = "Minimum usage count to include tag")]
        min_usage: usize,
        #[arg(long, help = "JSON output")]
        json: bool,
    },
    /// Extract keywords from text using Model2Vec tokenizer
    Keywords {
        /// Text to analyze
        text: String,
        #[arg(
            short,
            long,
            default_value = "10",
            help = "Number of keywords to extract"
        )]
        limit: usize,
        #[arg(long, help = "JSON output")]
        json: bool,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        // Default: run MCP server
        None => {
            #[cfg(feature = "mcp")]
            {
                run_mcp_server()
            }
            #[cfg(not(feature = "mcp"))]
            {
                eprintln!("MCP feature not enabled. Build with --features mcp");
                std::process::exit(1);
            }
        }

        // MCP Server
        #[cfg(feature = "mcp")]
        Some(Commands::Mcp { install }) => {
            if install {
                print_mcp_install_instructions();
                Ok(())
            } else {
                run_mcp_server()
            }
        }

        // Core commands
        Some(Commands::Init { config, inbox }) => commands::init::run(config, inbox),
        Some(Commands::Validate {
            schema,
            wikilinks,
            json,
        }) => commands::validate::run(schema, wikilinks, json),
        Some(Commands::Audit {
            quick,
            json,
            strict,
        }) => commands::audit::run(quick, json, strict),
        Some(Commands::Status { brief, json }) => commands::status::run(brief, json),
        Some(Commands::Health { details, json }) => commands::health::run(details, json),
        Some(Commands::Search { query, gist, limit }) => commands::search::run(&query, gist, limit),
        Some(Commands::Related {
            note,
            semantic,
            min_tags,
            limit,
            boost_type,
            boost_area,
            json,
        }) => commands::related::run(
            &note, min_tags, semantic, limit, boost_type, boost_area, json,
        ),
        Some(Commands::Tags { action }) => match action {
            None
            | Some(TagsAction::List {
                analyze: false,
                json: false,
            }) => commands::tags::run(false, false),
            Some(TagsAction::List { analyze, json }) => commands::tags::run(analyze, json),
            Some(TagsAction::Init { force }) => commands::tags::run_init(force),
            Some(TagsAction::Suggest {
                text,
                limit,
                discover,
                json,
            }) => commands::tags::run_suggest(&text, limit, discover, json),
            Some(TagsAction::Sync {
                execute,
                discover,
                json,
            }) => commands::tags::run_sync(execute, discover, json),
            Some(TagsAction::Extract { min_usage, json }) => {
                commands::tags::run_extract(min_usage, json)
            }
            Some(TagsAction::Keywords { text, limit, json }) => {
                commands::tags::run_keywords(&text, limit, json)
            }
        },
        Some(Commands::Fix {
            wikilinks,
            execute,
            json,
        }) => commands::fix::run(wikilinks, !execute, json),
        Some(Commands::Connect {
            execute,
            min_tags,
            semantic,
            limit,
            json,
        }) => commands::connect::run(!execute, min_tags, semantic, limit, json),

        // Semantic Search
        Some(Commands::Index {
            status,
            rebuild,
            json,
        }) => commands::index::run(status, rebuild, json),
        Some(Commands::SemanticSearch {
            query,
            limit,
            json,
            fallback,
        }) => commands::semantic_search::run(&query, limit, json, fallback),

        // Model Management
        Some(Commands::Model { action, json }) => commands::model::run(&action, json),
    }
}

#[cfg(feature = "mcp")]
fn run_mcp_server() -> anyhow::Result<()> {
    let vault_path = core::paths::get_vault_root();
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(mcp::run_mcp_server(vault_path))
}

#[cfg(feature = "mcp")]
fn print_mcp_install_instructions() {
    use colored::Colorize;
    use core::paths::VAULT_PATH_ENV;

    let vault_path = core::paths::get_vault_root().to_string_lossy().to_string();

    println!("{}", "Elysium MCP Server Installation Guide".bold().cyan());
    println!();
    println!("{}", "Configuration Priority:".bold());
    println!(
        "  1. {} environment variable (recommended)",
        VAULT_PATH_ENV.yellow()
    );
    println!("  2. Current working directory (fallback)");
    println!();
    println!(
        "{}",
        "For Claude Desktop (~/.config/claude/claude_desktop_config.json):".dimmed()
    );
    println!(
        r#"{{
  "mcpServers": {{
    "elysium": {{
      "command": "npx",
      "args": ["elysium-mcp"],
      "env": {{
        "{}": "{}"
      }}
    }}
  }}
}}"#,
        VAULT_PATH_ENV, vault_path
    );
    println!();
    println!("{}", "For Claude Code (.mcp.json in vault root):".dimmed());
    println!(
        r#"{{
  "mcpServers": {{
    "elysium": {{
      "command": "npx",
      "args": ["elysium-mcp"],
      "env": {{
        "{}": "{}"
      }}
    }}
  }}
}}"#,
        VAULT_PATH_ENV, vault_path
    );
    println!();
    println!("{}", "Available tools:".bold());
    println!(
        "  • {} - Semantic search using gist embeddings",
        "vault_search".green()
    );
    println!("  • {} - Get full note content", "vault_get_note".green());
    println!(
        "  • {} - List notes with filters",
        "vault_list_notes".green()
    );
    println!("  • {} - Get vault health score", "vault_health".green());
    println!("  • {} - Get vault status summary", "vault_status".green());
    println!(
        "  • {} - Run policy compliance audit",
        "vault_audit".green()
    );
}
