use crate::harness::Ctx;

// ─── No seed (stress tests on empty/minimal data) ──────────────────────

#[test]
fn walk_on_nonexistent_node_returns_empty() {
    let ctx = Ctx::new();
    ctx.run(&["walk", "Session:999", "HAS_TOPIC"])
        .success()
        .stdout_line_count(0);
}

#[test]
fn walk_with_no_edges_returns_empty() {
    let ctx = Ctx::new();
    ctx.run(&["add", "Session", "session_id=s1", "title=Lonely", "tool=claude", "model=opus", "project_path=/app"]).success();
    ctx.run(&["walk", "Session:1", "HAS_TOPIC"])
        .success()
        .stdout_line_count(0);
}

#[test]
fn path_to_self() {
    let ctx = Ctx::new();
    let id = ctx.run(&["add", "Session", "session_id=s1", "title=Self", "tool=claude", "model=opus", "project_path=/app"])
        .success().ref_id();

    // path from a node to itself — should find trivially or return no path (0 hops)
    let out = ctx.run(&["path", &format!("Session:{id}"), &format!("Session:{id}")]);
    out.success(); // should not crash
}

#[test]
fn walk_empty_edge_path_is_rejected() {
    let ctx = Ctx::new();
    let id = ctx.run(&["add", "Session", "session_id=s1", "title=T", "tool=claude", "model=opus", "project_path=/app"])
        .success().ref_id();

    ctx.run(&["walk", &format!("Session:{id}"), ""])
        .failure();
}

#[test]
fn add_special_characters_in_string_values() {
    let ctx = Ctx::new();
    let id = ctx.run(&["add", "Session",
        "session_id=s1",
        r#"title=Fix "quoted" bug's & <html>"#,
        "tool=claude", "model=opus", "project_path=/app",
    ]).success().ref_id();

    ctx.run(&["get", &format!("Session:{id}")])
        .success()
        .stdout_contains("quoted");
}

#[test]
fn add_very_long_title() {
    let ctx = Ctx::new();
    let long_title = "x".repeat(5000);
    ctx.run(&["add", "Session",
        "session_id=s1", &format!("title={long_title}"),
        "tool=claude", "model=opus", "project_path=/app",
    ]).success();
}

#[test]
fn link_to_nonexistent_node_fails() {
    let ctx = Ctx::new();
    let id = ctx.run(&["add", "Session", "session_id=s1", "title=T", "tool=claude", "model=opus", "project_path=/app"])
        .success().ref_id();

    ctx.run(&["link", &format!("Session:{id}"), "HAS_TOPIC", "Topic:999"])
        .failure();
}

#[test]
fn set_on_nonexistent_node_fails() {
    let ctx = Ctx::new();
    ctx.run(&["set", "Session:999", "summary=Nope"]).failure();
}

#[test]
fn malformed_ref_fails() {
    let ctx = Ctx::new();
    ctx.run(&["get", "bad-ref"]).failure();
    ctx.run(&["get", "Session:"]).failure();
    ctx.run(&["get", ":42"]).failure();
}

#[test]
fn multiple_nodes_same_kind_get_distinct_ids() {
    let ctx = Ctx::new();
    let a = ctx.run(&["add", "Topic", "name=first"]).success().ref_id();
    let b = ctx.run(&["add", "Topic", "name=second"]).success().ref_id();
    let c = ctx.run(&["add", "Topic", "name=third"]).success().ref_id();

    assert_ne!(a, b);
    assert_ne!(b, c);
    assert_ne!(a, c);
}

// ─── Seed: cycle detection ─────────────────────────────────────────────
//
//   a ──CONTINUES──> b ──CONTINUES──> c ──CONTINUES──> a  (cycle!)

#[test]
fn walk_handles_cycles_without_infinite_loop() {
    let ctx = Ctx::new();
    let a = ctx.run(&["add", "Session", "session_id=a", "title=A", "tool=claude", "model=opus", "project_path=/app"]).success().ref_id();
    let b = ctx.run(&["add", "Session", "session_id=b", "title=B", "tool=claude", "model=opus", "project_path=/app"]).success().ref_id();
    let c = ctx.run(&["add", "Session", "session_id=c", "title=C", "tool=claude", "model=opus", "project_path=/app"]).success().ref_id();

    ctx.run(&["link", &format!("Session:{a}"), "CONTINUES", &format!("Session:{b}")]).success();
    ctx.run(&["link", &format!("Session:{b}"), "CONTINUES", &format!("Session:{c}")]).success();
    ctx.run(&["link", &format!("Session:{c}"), "CONTINUES", &format!("Session:{a}")]).success();

    // walk should terminate and return results without looping forever
    ctx.run(&["walk", &format!("Session:{a}"), "CONTINUES/CONTINUES/CONTINUES"])
        .success();
}

#[test]
fn path_handles_cycles() {
    let ctx = Ctx::new();
    let a = ctx.run(&["add", "Session", "session_id=a", "title=A", "tool=claude", "model=opus", "project_path=/app"]).success().ref_id();
    let b = ctx.run(&["add", "Session", "session_id=b", "title=B", "tool=claude", "model=opus", "project_path=/app"]).success().ref_id();
    let c = ctx.run(&["add", "Session", "session_id=c", "title=C", "tool=claude", "model=opus", "project_path=/app"]).success().ref_id();

    ctx.run(&["link", &format!("Session:{a}"), "CONTINUES", &format!("Session:{b}")]).success();
    ctx.run(&["link", &format!("Session:{b}"), "CONTINUES", &format!("Session:{c}")]).success();
    ctx.run(&["link", &format!("Session:{c}"), "CONTINUES", &format!("Session:{a}")]).success();

    // should find path a→b→c without going around the cycle
    ctx.run(&["path", &format!("Session:{a}"), &format!("Session:{c}"), "--via", "CONTINUES"])
        .success()
        .stdout_any_line("finds path", |l| l.contains("hops)"));
}

// ─── Seed: high fan-out ────────────────────────────────────────────────

#[test]
fn walk_with_high_fanout() {
    let ctx = Ctx::new();
    let s = ctx.run(&["add", "Session", "session_id=s1", "title=Hub", "tool=claude", "model=opus", "project_path=/app"])
        .success().ref_id();

    for i in 0..20 {
        let t = ctx.run(&["add", "Topic", &format!("name=topic_{i}")]).success().ref_id();
        ctx.run(&["link", &format!("Session:{s}"), "HAS_TOPIC", &format!("Topic:{t}")]).success();
    }

    ctx.run(&["walk", &format!("Session:{s}"), "HAS_TOPIC"])
        .success()
        .stdout_line_count(20);

    ctx.run(&["walk", &format!("Session:{s}"), "HAS_TOPIC", "--limit", "5"])
        .success()
        .stdout_line_count(5);
}

// ─── Seed: duplicate edge attempt ──────────────────────────────────────

#[test]
fn duplicate_edges_are_independent() {
    let ctx = Ctx::new();
    let s = ctx.run(&["add", "Session", "session_id=s1", "title=T", "tool=claude", "model=opus", "project_path=/app"])
        .success().ref_id();
    let t = ctx.run(&["add", "Topic", "name=x"]).success().ref_id();

    // link twice — DuckDB doesn't enforce uniqueness on edges
    ctx.run(&["link", &format!("Session:{s}"), "HAS_TOPIC", &format!("Topic:{t}")]).success();
    ctx.run(&["link", &format!("Session:{s}"), "HAS_TOPIC", &format!("Topic:{t}")]).success();

    // walk should still work (may return duplicates or deduplicate via DISTINCT)
    ctx.run(&["walk", &format!("Session:{s}"), "HAS_TOPIC"])
        .success()
        .stdout_any_line("finds the topic", |l| l.contains(&format!("Topic:{t}")));
}
