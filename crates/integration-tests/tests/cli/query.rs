use crate::harness::Ctx;

// ─── No seed (schema commands work on empty DB) ────────────────────────

#[test]
fn schema_overview_lists_all_types() {
    let ctx = Ctx::new();
    ctx.run(&["schema"])
        .success()
        .stdout_has_line("Nodes: Artifact, Branch, Highlight, Model, Project, Session, Topic")
        .stdout_any_line("edges line has HAS_TOPIC", |l| l.contains("HAS_TOPIC") && l.contains("CONTINUES"));
}

#[test]
fn schema_session_shows_properties() {
    let ctx = Ctx::new();
    ctx.run(&["schema", "Session"])
        .success()
        .stdout_any_line("session_id is string", |l| l.contains("session_id") && l.contains("string"))
        .stdout_any_line("tool is enum", |l| l.contains("tool") && l.contains("enum"));
}

#[test]
fn find_unknown_kind_fails() {
    let ctx = Ctx::new();
    ctx.run(&["find", "FakeKind"]).failure();
}

#[test]
fn count_empty_returns_zero() {
    let ctx = Ctx::new();
    ctx.run(&["count", "Model"]).success().stdout_eq("0");
}

// ─── Seed: 4 sessions across 2 tools and 3 models ─────────────────────
//
//   s1: tool=claude, model=opus
//   s2: tool=claude, model=sonnet
//   s3: tool=claude, model=opus
//   s4: tool=codex,  model=gpt-5

fn seed_sessions(ctx: &Ctx) {
    for (id, title, tool, model) in [
        ("s1", "Fix JWT", "claude", "opus"),
        ("s2", "Add rate limiting", "claude", "sonnet"),
        ("s3", "Refactor auth", "claude", "opus"),
        ("s4", "DB migration", "codex", "gpt-5"),
    ] {
        ctx.run(&["add", "Session",
            &format!("session_id={id}"), &format!("title={title}"),
            &format!("tool={tool}"), &format!("model={model}"), "project_path=/app",
        ]).success();
    }
    ctx.run(&["add", "Topic", "name=auth"]).success();
    ctx.run(&["add", "Topic", "name=security"]).success();
}

#[test]
fn find_all_sessions() {
    let ctx = Ctx::new();
    seed_sessions(&ctx);

    ctx.run(&["find", "Session"])
        .success()
        .stdout_line_count(4)
        .stdout_contains("Fix JWT")
        .stdout_contains("DB migration");
}

#[test]
fn find_respects_limit() {
    let ctx = Ctx::new();
    seed_sessions(&ctx);

    ctx.run(&["find", "Session", "--limit", "2"])
        .success()
        .stdout_line_count(2);
}

#[test]
fn find_output_is_compact() {
    let ctx = Ctx::new();
    seed_sessions(&ctx);

    ctx.run(&["find", "Session", "--limit", "1"])
        .success()
        .stdout_all_lines("each line is Kind:id \"label\"", |l| {
            l.starts_with("Session:") && l.contains('"')
        });
}

#[test]
fn count_total_sessions() {
    let ctx = Ctx::new();
    seed_sessions(&ctx);

    ctx.run(&["count", "Session"]).success().stdout_eq("4");
}

#[test]
fn count_total_topics() {
    let ctx = Ctx::new();
    seed_sessions(&ctx);

    ctx.run(&["count", "Topic"]).success().stdout_eq("2");
}

// -- count --by (aggregation) ----------------------------------------------

#[test]
fn count_by_tool() {
    let ctx = Ctx::new();
    seed_sessions(&ctx);

    ctx.run(&["count", "Session", "--by", "tool"])
        .success()
        .stdout_has_kv("claude", "3")
        .stdout_has_kv("codex", "1")
        .stdout_line_count(2);
}

#[test]
fn count_by_model() {
    let ctx = Ctx::new();
    seed_sessions(&ctx);

    ctx.run(&["count", "Session", "--by", "model"])
        .success()
        .stdout_has_kv("opus", "2")
        .stdout_has_kv("sonnet", "1")
        .stdout_has_kv("gpt-5", "1")
        .stdout_line_count(3);
}

#[test]
fn count_by_returns_descending_order() {
    let ctx = Ctx::new();
    seed_sessions(&ctx);

    ctx.run(&["count", "Session", "--by", "tool"])
        .success()
        .stdout_line_at(0, "most frequent tool first", |l| l.starts_with("claude"));
}

// -- kinds / stats ----------------------------------------------------------

#[test]
fn kinds_shows_exact_counts() {
    let ctx = Ctx::new();
    seed_sessions(&ctx);

    ctx.run(&["kinds"])
        .success()
        .stdout_has_kv("Session", "4")
        .stdout_has_kv("Topic", "2");
}

#[test]
fn stats_shows_totals() {
    let ctx = Ctx::new();
    seed_sessions(&ctx);

    ctx.run(&["stats"])
        .success()
        .stdout_any_line("node total includes 6", |l| l.contains("Nodes:") && l.contains("6"))
        .stdout_any_line("edge total is 0", |l| l.contains("Edges:") && l.contains("0"));
}

// -- find filters -----------------------------------------------------------

// Seed: 4 sessions (reuses seed_sessions) + 3 highlights

fn seed_with_highlights(ctx: &Ctx) {
    seed_sessions(ctx);
    ctx.run(&["add", "Highlight", "content=Found the root cause", "kind=discovery"]).success();
    ctx.run(&["add", "Highlight", "content=Switch to batch processing", "kind=decision"]).success();
    ctx.run(&["add", "Highlight", "content=API rate limit blocks import", "kind=blocker"]).success();
}

#[test]
fn filter_exact_match() {
    let ctx = Ctx::new();
    seed_sessions(&ctx);

    ctx.run(&["find", "Session", "tool=codex"])
        .success()
        .stdout_line_count(1)
        .stdout_contains("DB migration");
}

#[test]
fn filter_multiple_conditions() {
    let ctx = Ctx::new();
    seed_sessions(&ctx);

    ctx.run(&["find", "Session", "tool=claude,model=opus"])
        .success()
        .stdout_line_count(2)
        .stdout_contains("Fix JWT")
        .stdout_contains("Refactor auth")
        .stdout_not_contains("Add rate limiting");
}

#[test]
fn filter_contains() {
    let ctx = Ctx::new();
    seed_sessions(&ctx);

    ctx.run(&["find", "Session", "title~rate"])
        .success()
        .stdout_line_count(1)
        .stdout_contains("Add rate limiting");
}

#[test]
fn filter_starts_with() {
    let ctx = Ctx::new();
    seed_sessions(&ctx);

    ctx.run(&["find", "Session", "title^Fix"])
        .success()
        .stdout_line_count(1)
        .stdout_contains("Fix JWT");
}

#[test]
fn filter_not_equal() {
    let ctx = Ctx::new();
    seed_sessions(&ctx);

    ctx.run(&["find", "Session", "tool!=codex"])
        .success()
        .stdout_line_count(3)
        .stdout_not_contains("DB migration");
}

#[test]
fn filter_by_highlight_kind() {
    let ctx = Ctx::new();
    seed_with_highlights(&ctx);

    ctx.run(&["find", "Highlight", "kind=blocker"])
        .success()
        .stdout_line_count(1)
        .stdout_contains("API rate limit");
}

#[test]
fn filter_by_highlight_kind_discovery() {
    let ctx = Ctx::new();
    seed_with_highlights(&ctx);

    ctx.run(&["find", "Highlight", "kind=discovery"])
        .success()
        .stdout_line_count(1)
        .stdout_contains("root cause");
}

#[test]
fn filter_no_match_returns_empty() {
    let ctx = Ctx::new();
    seed_sessions(&ctx);

    ctx.run(&["find", "Session", "tool=vim"])
        .success()
        .stdout_line_count(0);
}

#[test]
fn filter_with_order() {
    let ctx = Ctx::new();
    seed_sessions(&ctx);

    ctx.run(&["find", "Session", "--order", "title:asc", "tool=claude"])
        .success()
        .stdout_line_count(3)
        .stdout_line_at(0, "first alphabetically", |l| l.contains("Add rate limiting"));
}

#[test]
fn filter_with_limit() {
    let ctx = Ctx::new();
    seed_sessions(&ctx);

    ctx.run(&["find", "Session", "--limit", "2", "tool=claude"])
        .success()
        .stdout_line_count(2);
}
