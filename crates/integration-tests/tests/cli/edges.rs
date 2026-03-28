use crate::harness::Ctx;

// ─── Seed: session with multiple edge types ────────────────────────────
//
//   session ──HAS_TOPIC──> topic_a
//   session ──HAS_TOPIC──> topic_b
//   session ──IN_PROJECT─> project

struct EdgeGraph {
    session: i64,
    topic_a: i64,
    topic_b: i64,
    project: i64,
}

fn seed_edge_graph(ctx: &Ctx) -> EdgeGraph {
    let s = ctx.run(&["add", "Session", "session_id=s1", "title=Work", "tool=claude", "model=opus", "project_path=/app"])
        .success().ref_id();
    let ta = ctx.run(&["add", "Topic", "name=auth"]).success().ref_id();
    let tb = ctx.run(&["add", "Topic", "name=security"]).success().ref_id();
    let p = ctx.run(&["add", "Project", "name=myapp", "path=/app"]).success().ref_id();

    ctx.run(&["link", &format!("Session:{s}"), "HAS_TOPIC", &format!("Topic:{ta}")]).success();
    ctx.run(&["link", &format!("Session:{s}"), "HAS_TOPIC", &format!("Topic:{tb}")]).success();
    ctx.run(&["link", &format!("Session:{s}"), "IN_PROJECT", &format!("Project:{p}")]).success();

    EdgeGraph { session: s, topic_a: ta, topic_b: tb, project: p }
}

#[test]
fn edges_groups_by_kind() {
    let ctx = Ctx::new();
    let g = seed_edge_graph(&ctx);

    ctx.run(&["edges", &format!("Session:{}", g.session)])
        .success()
        .stdout_any_line("HAS_TOPIC line lists both topics", |l| {
            l.starts_with("> HAS_TOPIC:") && l.contains(&format!("Topic:{}", g.topic_a)) && l.contains(&format!("Topic:{}", g.topic_b))
        })
        .stdout_any_line("IN_PROJECT line", |l| l.starts_with("> IN_PROJECT:") && l.contains(&format!("Project:{}", g.project)));
}

#[test]
fn edges_outgoing_only() {
    let ctx = Ctx::new();
    let g = seed_edge_graph(&ctx);

    ctx.run(&["edges", &format!("Session:{}", g.session), "--out"])
        .success()
        .stdout_all_lines("all lines are outgoing", |l| l.starts_with("> "));
}

#[test]
fn edges_incoming_on_topic() {
    let ctx = Ctx::new();
    let g = seed_edge_graph(&ctx);

    ctx.run(&["edges", &format!("Topic:{}", g.topic_a), "--in"])
        .success()
        .stdout_any_line("incoming from session", |l| l.starts_with("< HAS_TOPIC:") && l.contains(&format!("Session:{}", g.session)));
}

#[test]
fn edges_kind_filter() {
    let ctx = Ctx::new();
    let g = seed_edge_graph(&ctx);

    ctx.run(&["edges", &format!("Session:{}", g.session), "--kind", "HAS_TOPIC"])
        .success()
        .stdout_contains("HAS_TOPIC")
        .stdout_not_contains("IN_PROJECT");
}

#[test]
fn link_rejects_invalid_endpoints() {
    let ctx = Ctx::new();
    let g = seed_edge_graph(&ctx);

    ctx.run(&["link", &format!("Session:{}", g.session), "HAS_TOPIC", &format!("Project:{}", g.project)])
        .failure()
        .stderr_contains("cannot connect");
}

#[test]
fn link_rejects_unknown_edge() {
    let ctx = Ctx::new();
    let g = seed_edge_graph(&ctx);

    ctx.run(&["link", &format!("Session:{}", g.session), "FAKE", &format!("Topic:{}", g.topic_a)])
        .failure();
}

#[test]
fn unlink_removes_specific_edge() {
    let ctx = Ctx::new();
    let g = seed_edge_graph(&ctx);

    ctx.run(&["unlink", &format!("Session:{}", g.session), "HAS_TOPIC", &format!("Topic:{}", g.topic_a)]).success();

    ctx.run(&["edges", &format!("Session:{}", g.session)])
        .success()
        .stdout_not_contains(&format!("Topic:{}", g.topic_a))
        .stdout_contains(&format!("Topic:{}", g.topic_b));
}

#[test]
fn unlink_nonexistent_fails() {
    let ctx = Ctx::new();
    let g = seed_edge_graph(&ctx);

    ctx.run(&["unlink", &format!("Session:{}", g.session), "IN_PROJECT", &format!("Topic:{}", g.topic_b)])
        .failure();
}
