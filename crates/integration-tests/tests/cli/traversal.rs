use crate::harness::Ctx;

// ─── Seed: 3 sessions sharing topics via a project ─────────────────────
//
//   jwt ──HAS_TOPIC──> auth
//   jwt ──HAS_TOPIC──> security
//   jwt ──IN_PROJECT─> app
//   jwt ──ON_BRANCH──> main
//   jwt ──CONTINUES──> rate
//
//   rate ──HAS_TOPIC──> security
//   rate ──IN_PROJECT─> app
//
//   migration ──HAS_TOPIC──> db_topic
//   migration ──IN_PROJECT─> app

struct TraversalGraph {
    jwt: i64,
    rate: i64,
    security: i64,
    db_topic: i64,
    main_branch: i64,
}

fn seed_traversal(ctx: &Ctx) -> TraversalGraph {
    let jwt = ctx.run(&["add", "Session", "session_id=s1", "title=Fix JWT", "tool=claude", "model=opus", "project_path=/app"])
        .success().ref_id();
    let rate = ctx.run(&["add", "Session", "session_id=s2", "title=Add rate limiting", "tool=claude", "model=sonnet", "project_path=/app"])
        .success().ref_id();
    let migration = ctx.run(&["add", "Session", "session_id=s3", "title=DB migration", "tool=codex", "model=gpt-5", "project_path=/app"])
        .success().ref_id();

    let auth = ctx.run(&["add", "Topic", "name=auth"]).success().ref_id();
    let security = ctx.run(&["add", "Topic", "name=security"]).success().ref_id();
    let db_topic = ctx.run(&["add", "Topic", "name=database"]).success().ref_id();
    let app = ctx.run(&["add", "Project", "name=myapp", "path=/app"]).success().ref_id();
    let main_branch = ctx.run(&["add", "Branch", "name=main"]).success().ref_id();

    for (from, edge, to) in [
        (format!("Session:{jwt}"), "HAS_TOPIC", format!("Topic:{auth}")),
        (format!("Session:{jwt}"), "HAS_TOPIC", format!("Topic:{security}")),
        (format!("Session:{jwt}"), "IN_PROJECT", format!("Project:{app}")),
        (format!("Session:{jwt}"), "ON_BRANCH", format!("Branch:{main_branch}")),
        (format!("Session:{rate}"), "HAS_TOPIC", format!("Topic:{security}")),
        (format!("Session:{rate}"), "IN_PROJECT", format!("Project:{app}")),
        (format!("Session:{migration}"), "HAS_TOPIC", format!("Topic:{db_topic}")),
        (format!("Session:{migration}"), "IN_PROJECT", format!("Project:{app}")),
    ] {
        ctx.run(&["link", &from, edge, &to]).success();
    }
    ctx.run(&["link", &format!("Session:{jwt}"), "CONTINUES", &format!("Session:{rate}"), "reason=same feature"]).success();

    TraversalGraph { jwt, rate, security, db_topic, main_branch }
}

// -- walk -------------------------------------------------------------------

#[test]
fn walk_returns_correct_kind() {
    let ctx = Ctx::new();
    let g = seed_traversal(&ctx);

    ctx.run(&["walk", &format!("Session:{}", g.jwt), "HAS_TOPIC"])
        .success()
        .stdout_line_count(2)
        .stdout_all_lines("all results are Topics", |l| l.starts_with("Topic:"));
}

#[test]
fn walk_reverse_returns_sessions() {
    let ctx = Ctx::new();
    let g = seed_traversal(&ctx);

    ctx.run(&["walk", &format!("Topic:{}", g.security), "~HAS_TOPIC"])
        .success()
        .stdout_line_count(2)
        .stdout_all_lines("all results are Sessions", |l| l.starts_with("Session:"))
        .stdout_contains("Fix JWT")
        .stdout_contains("Add rate limiting");
}

#[test]
fn walk_two_hops_finds_related() {
    let ctx = Ctx::new();
    let g = seed_traversal(&ctx);

    ctx.run(&["walk", &format!("Session:{}", g.jwt), "HAS_TOPIC/~HAS_TOPIC"])
        .success()
        .stdout_any_line("finds rate session", |l| l.contains("Add rate limiting"))
        .stdout_all_lines("all results are Sessions", |l| l.starts_with("Session:"));
}

#[test]
fn walk_three_hops_discovers_topics_across_project() {
    let ctx = Ctx::new();
    let g = seed_traversal(&ctx);

    ctx.run(&["walk", &format!("Session:{}", g.jwt), "IN_PROJECT/~IN_PROJECT/HAS_TOPIC"])
        .success()
        .stdout_any_line("finds database topic", |l| l.contains("database"))
        .stdout_all_lines("all results are Topics", |l| l.starts_with("Topic:"));
}

#[test]
fn walk_wildcard_returns_mixed_kinds() {
    let ctx = Ctx::new();
    let g = seed_traversal(&ctx);

    let out = ctx.run(&["walk", &format!("Session:{}", g.jwt), "*"]);
    out.success();

    let lines = out.stdout.lines().filter(|l| !l.is_empty()).collect::<Vec<_>>();
    let kinds: std::collections::HashSet<&str> = lines.iter()
        .filter_map(|l| l.split(':').next())
        .collect();

    assert!(kinds.len() >= 3, "expected at least 3 different kinds in wildcard walk, got {kinds:?}");
}

#[test]
fn walk_limit_truncates() {
    let ctx = Ctx::new();
    let g = seed_traversal(&ctx);

    ctx.run(&["walk", &format!("Session:{}", g.jwt), "*", "--limit", "2"])
        .success()
        .stdout_line_count(2);
}

#[test]
fn walk_rejects_injection() {
    let ctx = Ctx::new();
    let g = seed_traversal(&ctx);

    ctx.run(&["walk", &format!("Session:{}", g.jwt), "'; DROP TABLE node; --"])
        .failure();
}

// -- path -------------------------------------------------------------------

#[test]
fn path_shows_hop_count() {
    let ctx = Ctx::new();
    let g = seed_traversal(&ctx);

    ctx.run(&["path", &format!("Session:{}", g.jwt), &format!("Session:{}", g.rate)])
        .success()
        .stdout_any_line("output contains hop count", |l| l.contains("hops)"));
}

#[test]
fn path_via_continues_is_one_hop() {
    let ctx = Ctx::new();
    let g = seed_traversal(&ctx);

    ctx.run(&["path", &format!("Session:{}", g.jwt), &format!("Session:{}", g.rate), "--via", "CONTINUES"])
        .success()
        .stdout_any_line("exactly 1 hop", |l| l.contains("1 hops)"));
}

#[test]
fn path_via_shared_topic_is_two_hops() {
    let ctx = Ctx::new();
    let g = seed_traversal(&ctx);

    ctx.run(&["path", &format!("Session:{}", g.jwt), &format!("Session:{}", g.rate), "--via", "HAS_TOPIC"])
        .success()
        .stdout_any_line("2 hops via topic", |l| l.contains("2 hops)"));
}

#[test]
fn path_not_found_within_depth() {
    let ctx = Ctx::new();
    let g = seed_traversal(&ctx);

    ctx.run(&["path", &format!("Branch:{}", g.main_branch), &format!("Topic:{}", g.db_topic), "--depth", "1"])
        .success()
        .stdout_has_line("No path found within depth 1.");
}

#[test]
fn path_output_shows_edge_directions() {
    let ctx = Ctx::new();
    let g = seed_traversal(&ctx);

    ctx.run(&["path", &format!("Session:{}", g.jwt), &format!("Session:{}", g.rate), "--via", "CONTINUES"])
        .success()
        .stdout_any_line("shows direction arrows", |l| l.contains('>') || l.contains('<'));
}
