use clap::Subcommand;
use ctx_db::{CreateEdgeParams, CreateNodeParams, Database, DbError};
use ctx_schema;

use crate::output::{self, OutputMode};
use crate::parse;

#[derive(Subcommand)]
pub enum Commands {
    /// Create a node
    Add {
        kind: String,
        #[arg(trailing_var_arg = true)]
        props: Vec<String>,
    },
    /// Show a node and its edges
    Get { r#ref: String },
    /// Update node properties
    Set {
        r#ref: String,
        #[arg(trailing_var_arg = true)]
        props: Vec<String>,
    },
    /// Delete a node
    Rm { r#ref: String },
    /// Create an edge
    Link {
        from: String,
        edge_kind: String,
        to: String,
        #[arg(trailing_var_arg = true)]
        props: Vec<String>,
    },
    /// Update edge properties
    #[command(name = "edge-set")]
    EdgeSet {
        from: String,
        edge_kind: String,
        to: String,
        #[arg(trailing_var_arg = true)]
        props: Vec<String>,
    },
    /// Remove an edge
    Unlink {
        from: String,
        edge_kind: String,
        to: String,
    },
    /// Change a node's kind
    Rekind {
        r#ref: String,
        new_kind: String,
    },
    /// List edges on a node
    Edges {
        r#ref: String,
        #[arg(long)]
        r#in: bool,
        #[arg(long)]
        out: bool,
        #[arg(long)]
        kind: Option<String>,
    },
    /// Find nodes by kind and filters
    Find {
        kind: String,
        #[arg(trailing_var_arg = true)]
        filters: Vec<String>,
        #[arg(long, default_value = "30")]
        limit: usize,
        #[arg(long)]
        order: Option<String>,
        #[arg(long)]
        full: bool,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        quiet: bool,
    },
    /// Full-text search
    Search {
        query: String,
        #[arg(long)]
        kind: Option<String>,
        #[arg(long, default_value = "10")]
        limit: usize,
    },
    /// Count nodes
    Count {
        kind: String,
        #[arg(long)]
        by: Option<String>,
    },
    /// Traverse the graph along an edge path
    Walk {
        r#ref: String,
        edge_path: String,
        #[arg(long, short = 'w')]
        r#where: Option<String>,
        #[arg(long, default_value = "30")]
        limit: usize,
    },
    /// Find shortest path between two nodes
    Path {
        from: String,
        to: String,
        #[arg(long, default_value = "3")]
        depth: usize,
        #[arg(long)]
        via: Option<String>,
    },
    /// Show the schema
    Schema { kind: Option<String> },
    /// List node kinds with counts
    Kinds,
    /// Database statistics
    Stats,
}

pub fn run(db: &Database, cmd: Commands) -> Result<(), DbError> {
    match cmd {
        Commands::Add { kind, props } => {
            let properties = parse::key_value_pairs(&props);
            let node = db.create_node(CreateNodeParams { kind, properties })?;
            println!("{}", node.ref_str());
        }
        Commands::Get { r#ref } => {
            let (kind, id) = parse::node_ref(&r#ref)?;
            let node = db.resolve_ref(&kind, id)?;
            let edges = db.list_edges(node.id)?;
            output::print_node_full(&node, &edges);
        }
        Commands::Set { r#ref, props } => {
            let (kind, id) = parse::node_ref(&r#ref)?;
            let properties = parse::key_value_pairs(&props);
            db.update_node(&kind, id, properties)?;
        }
        Commands::Rm { r#ref } => {
            let (kind, id) = parse::node_ref(&r#ref)?;
            db.delete_node(&kind, id)?;
        }
        Commands::Link {
            from,
            edge_kind,
            to,
            props,
        } => {
            let (_, from_id) = parse::node_ref(&from)?;
            let (_, to_id) = parse::node_ref(&to)?;
            let properties = parse::key_value_pairs(&props);
            db.create_edge(CreateEdgeParams {
                kind: edge_kind,
                from_id,
                to_id,
                properties,
            })?;
        }
        Commands::EdgeSet {
            from,
            edge_kind,
            to,
            props,
        } => {
            let (_, from_id) = parse::node_ref(&from)?;
            let (_, to_id) = parse::node_ref(&to)?;
            let properties = parse::key_value_pairs(&props);
            db.update_edge(from_id, &edge_kind, to_id, properties)?;
        }
        Commands::Unlink {
            from,
            edge_kind,
            to,
        } => {
            let (_, from_id) = parse::node_ref(&from)?;
            let (_, to_id) = parse::node_ref(&to)?;
            db.delete_edge(from_id, &edge_kind, to_id)?;
        }
        Commands::Rekind { r#ref, new_kind } => {
            let (kind, id) = parse::node_ref(&r#ref)?;
            let node = db.change_node_kind(&kind, id, &new_kind)?;
            println!("{}", node.ref_str());
        }
        Commands::Edges {
            r#ref,
            r#in,
            out,
            kind,
        } => {
            let (_, id) = parse::node_ref(&r#ref)?;
            let edges = db.list_edges(id)?;
            output::print_edges(id, &edges, r#in, out, kind.as_deref());
        }
        Commands::Find {
            kind,
            filters: _,
            limit,
            order,
            full,
            json,
            quiet,
        } => {
            let nodes = db.list_nodes_ordered(&kind, limit, order.as_deref())?;
            let mode = if json {
                OutputMode::Json
            } else if quiet {
                OutputMode::Quiet
            } else if full {
                OutputMode::Full
            } else {
                OutputMode::Compact
            };
            output::print_nodes(&nodes, mode, db);
        }
        Commands::Search { query, kind, limit } => {
            let results = ctx_graph::search(db, &query, kind.as_deref(), limit)?;
            for r in results {
                println!("{}:{} \"{}\" ({:.2})", r.kind, r.id, r.label, r.score);
            }
        }
        Commands::Count { kind, by } => {
            if let Some(prop) = by {
                use ctx_db::sql::{Expr, Query, TableRef, require_ident};
                require_ident(&kind)?;
                require_ident(&prop)?;
                let node_def = db.schema().nodes.get(&kind)
                    .ok_or_else(|| ctx_db::DbError::Validation(
                        ctx_schema::ValidationError::UnknownKind(kind.clone()),
                    ))?;
                if !node_def.properties.contains_key(&prop) {
                    return Err(ctx_db::DbError::Validation(
                        ctx_schema::ValidationError::UnknownProperty {
                            kind: kind.clone(),
                            prop: prop.clone(),
                        },
                    ));
                }
                let view = format!("v_{}", kind.to_lowercase());
                let q = Query::from(TableRef::scan(&view, "v"))
                    .col("v", &prop)
                    .col_as(Expr::func("count", vec![Expr::Star]), "count")
                    .group(Expr::col("v", &prop))
                    .order("count", true)
                    .build();
                let mut stmt = db.conn().prepare(q.sql()).map_err(ctx_db::DbError::from)?;
                let mut rows = stmt.query(q.param_refs().as_slice()).map_err(ctx_db::DbError::from)?;
                while let Some(row) = rows.next().map_err(ctx_db::DbError::from)? {
                    let val: String = row.get(0).unwrap_or_default();
                    let count: i64 = row.get(1).unwrap_or(0);
                    println!("{val:<15} {count}");
                }
            } else {
                let count = db.count_by_kind(&kind)?;
                println!("{count}");
            }
        }
        Commands::Walk {
            r#ref,
            edge_path,
            limit,
            ..
        } => {
            let (_, id) = parse::node_ref(&r#ref)?;
            let nodes = ctx_graph::walk(db, id, &edge_path, limit)?;
            output::print_nodes(&nodes, OutputMode::Compact, db);
        }
        Commands::Path {
            from,
            to,
            depth,
            via,
        } => {
            let (_, from_id) = parse::node_ref(&from)?;
            let (_, to_id) = parse::node_ref(&to)?;
            let via_kinds: Option<Vec<&str>> = via.as_ref().map(|v| v.split(',').collect());
            let result =
                ctx_graph::shortest_path(db, from_id, to_id, depth, via_kinds.as_deref())?;
            match result {
                Some(path) => output::print_path(&path),
                None => println!("No path found within depth {depth}."),
            }
        }
        Commands::Schema { kind } => {
            if let Some(k) = kind {
                output::print_kind_schema(db.schema(), &k);
            } else {
                output::print_schema_overview(db.schema());
            }
        }
        Commands::Kinds => {
            let counts = db.kind_counts()?;
            for (kind, count) in counts {
                println!("{kind:<15} {count}");
            }
        }
        Commands::Stats => {
            let node_count = db.node_count()?;
            let edge_count = db.edge_count()?;
            let kind_counts = db.kind_counts()?;
            let kinds_str = kind_counts
                .iter()
                .map(|(k, c)| format!("{k}: {c}"))
                .collect::<Vec<_>>()
                .join(", ");
            println!("Nodes: {node_count}  ({kinds_str})");
            println!("Edges: {edge_count}");
        }
    }
    Ok(())
}
