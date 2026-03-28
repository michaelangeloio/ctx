mod commands;
mod output;
mod parse;

use clap::Parser;
use commands::Commands;

#[derive(Parser)]
#[command(name = "ctx", about = "Local context graph for AI coding sessions")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Database file path
    #[arg(long, global = true, env = "CTX_DB")]
    db: Option<String>,

    /// Schema file path
    #[arg(long, global = true, env = "CTX_SCHEMA")]
    schema: Option<String>,
}

fn main() {
    let cli = Cli::parse();

    let schema = match &cli.schema {
        Some(path) => ctx_schema::load_schema(std::path::Path::new(path)),
        None => ctx_schema::load_default_schema(),
    };

    let schema = match schema {
        Ok(s) => s,
        Err(e) => {
            eprintln!("ctx: schema error: {e}");
            std::process::exit(1);
        }
    };

    let db = match &cli.db {
        Some(path) => ctx_db::Database::open(std::path::Path::new(path), schema),
        None => ctx_db::Database::open_default(schema),
    };

    let db = match db {
        Ok(d) => d,
        Err(e) => {
            eprintln!("ctx: database error: {e}");
            std::process::exit(1);
        }
    };

    if let Err(e) = commands::run(&db, cli.command) {
        eprintln!("ctx: {e}");
        std::process::exit(1);
    }
}
