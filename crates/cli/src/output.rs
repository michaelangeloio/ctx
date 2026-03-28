use std::collections::BTreeMap;

use ctx_db::{Database, Edge, Node};
use ctx_graph::PathResult;
use ctx_schema::Schema;

pub enum OutputMode {
    Compact,
    Full,
    Json,
    Quiet,
}

pub fn print_nodes(nodes: &[Node], mode: OutputMode, db: &Database) {
    match mode {
        OutputMode::Compact => {
            for node in nodes {
                println!("{} \"{}\"", node.ref_str(), node.label());
            }
        }
        OutputMode::Full => {
            for node in nodes {
                let edges = db.list_edges(node.id).unwrap_or_default();
                print_node_full(node, &edges);
            }
        }
        OutputMode::Json => {
            let json = serde_json::to_string_pretty(nodes).unwrap_or_default();
            println!("{json}");
        }
        OutputMode::Quiet => {
            for node in nodes {
                println!("{}", node.id);
            }
        }
    }
}

pub fn print_node_full(node: &Node, edges: &[Edge]) {
    println!("{} \"{}\"", node.ref_str(), node.label());

    let props: Vec<String> = node
        .properties
        .iter()
        .filter(|(k, _)| *k != "title" && *k != "name")
        .map(|(k, v)| {
            let val = match v {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            format!("{k}={val}")
        })
        .collect();

    if !props.is_empty() {
        println!("  {}  created={}", props.join("  "), &node.created_at[..10]);
    }

    // group edges by (direction, kind)
    let mut outgoing: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut incoming: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for edge in edges {
        if edge.from_id == node.id {
            let target = format!("{}:{}", edge.to_kind, edge.to_id);
            outgoing.entry(edge.kind.clone()).or_default().push(target);
        } else {
            let source = format!("{}:{}", edge.from_kind, edge.from_id);
            incoming.entry(edge.kind.clone()).or_default().push(source);
        }
    }

    for (kind, targets) in &outgoing {
        println!("  > {kind}: {}", targets.join(", "));
    }
    for (kind, sources) in &incoming {
        println!("  < {kind}: {}", sources.join(", "));
    }
}

pub fn print_edges(
    node_id: i64,
    edges: &[Edge],
    show_in: bool,
    show_out: bool,
    kind_filter: Option<&str>,
) {
    let show_both = !show_in && !show_out;
    let mut outgoing: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut incoming: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for edge in edges {
        if let Some(filter) = kind_filter {
            if edge.kind != filter {
                continue;
            }
        }

        if edge.from_id == node_id {
            let target = format!("{}:{}", edge.to_kind, edge.to_id);
            outgoing.entry(edge.kind.clone()).or_default().push(target);
        } else {
            let source = format!("{}:{}", edge.from_kind, edge.from_id);
            incoming.entry(edge.kind.clone()).or_default().push(source);
        }
    }

    if show_both || show_out {
        for (kind, targets) in &outgoing {
            println!("> {kind}: {}", targets.join(", "));
        }
    }
    if show_both || show_in {
        for (kind, sources) in &incoming {
            println!("< {kind}: {}", sources.join(", "));
        }
    }
}

pub fn print_path(path: &PathResult) {
    let parts: Vec<String> = path
        .hops
        .iter()
        .enumerate()
        .map(|(i, hop)| {
            let node_str = format!("{}:{}", hop.node_kind, hop.node_id);
            if i == 0 {
                return node_str;
            }
            match (&hop.edge_kind, &hop.direction) {
                (Some(ek), Some(ctx_graph::Direction::Outgoing)) => {
                    format!(">{ek}> {node_str}")
                }
                (Some(ek), Some(ctx_graph::Direction::Incoming)) => {
                    format!("<{ek}< {node_str}")
                }
                _ => format!("-- {node_str}"),
            }
        })
        .collect();

    let hop_count = path.hops.len().saturating_sub(1);
    println!("{}  ({hop_count} hops)", parts.join(" "));
}

fn print_edge_props(props: &std::collections::BTreeMap<String, ctx_schema::PropDef>) {
    if props.is_empty() { return; }
    for (name, prop) in props {
        let type_str = match &prop.prop_type {
            ctx_schema::PropType::Enum(vals) => format!("enum: {}", vals.join(", ")),
            other => format!("{:?}", other).to_lowercase(),
        };
        let suffix = if prop.required { "" } else { "?" };
        let hint = prop.hint.as_deref().map(|h| format!("  — {h}")).unwrap_or_default();
        print!("\n    {name}: {type_str}{suffix}{hint}");
    }
}

pub fn print_schema_overview(schema: &Schema) {
    let nodes: Vec<&str> = schema.nodes.keys().map(|s| s.as_str()).collect();
    let edges: Vec<&str> = schema.edges.keys().map(|s| s.as_str()).collect();
    println!("Nodes: {}", nodes.join(", "));
    println!("Edges: {}", edges.join(", "));
}

pub fn print_kind_schema(schema: &Schema, kind: &str) {
    if let Some(node_def) = schema.nodes.get(kind) {
        let req_count = node_def.required_props().count();
        let opt_count = node_def.optional_props().count();
        println!("{kind} ({req_count} required, {opt_count} optional)");
        for (name, prop) in &node_def.properties {
            let type_str = match &prop.prop_type {
                ctx_schema::PropType::Enum(vals) => format!("enum: {}", vals.join(", ")),
                other => format!("{:?}", other).to_lowercase(),
            };
            let suffix = if prop.required { "" } else { "?" };
            let hint = prop.hint.as_deref().map(|h| format!("  — {h}")).unwrap_or_default();
            println!("  {name:<20} {type_str}{suffix}{hint}");
        }

        println!();
        for (edge_name, edge_def) in &schema.edges {
            if edge_def.from.iter().any(|f| f == kind) {
                let targets = edge_def.to.join(", ");
                print!("  edges out: {edge_name} → {targets}");
                print_edge_props(&edge_def.properties);
                println!();
            }
            if edge_def.to.iter().any(|t| t == kind) {
                let sources = edge_def.from.join(", ");
                println!("  edges in:  {edge_name} ← {sources}");
            }
        }
    } else {
        eprintln!("Unknown kind: {kind}");
    }
}
