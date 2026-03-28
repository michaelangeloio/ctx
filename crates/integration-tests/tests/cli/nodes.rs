use crate::harness::Ctx;

// ─── No seed (validation on empty DB) ──────────────────────────────────

#[test]
fn add_returns_ref() {
    let ctx = Ctx::new();
    ctx.run(&["add", "Session", "session_id=x", "title=Test", "tool=claude", "model=opus", "project_path=/tmp"])
        .success()
        .stdout_contains("Session:");
}

#[test]
fn add_rejects_unknown_kind() {
    let ctx = Ctx::new();
    ctx.run(&["add", "FakeKind", "name=bad"]).failure();
}

#[test]
fn add_rejects_missing_required() {
    let ctx = Ctx::new();
    ctx.run(&["add", "Session", "title=Incomplete"]).failure();
}

#[test]
fn add_rejects_unknown_property() {
    let ctx = Ctx::new();
    ctx.run(&["add", "Session", "session_id=x", "title=T", "tool=claude", "model=opus", "project_path=/tmp", "mood=happy"])
        .failure()
        .stderr_contains("unknown property");
}

#[test]
fn add_rejects_bad_enum() {
    let ctx = Ctx::new();
    ctx.run(&["add", "Session", "session_id=x", "title=T", "tool=vim", "model=opus", "project_path=/tmp"])
        .failure()
        .stderr_contains("enum");
}

#[test]
fn get_unknown_node_fails() {
    let ctx = Ctx::new();
    ctx.run(&["get", "Session:999"]).failure();
}

#[test]
fn rm_unknown_node_fails() {
    let ctx = Ctx::new();
    ctx.run(&["rm", "Session:999"]).failure();
}

// ─── Seed: one session + one topic + one edge ──────────────────────────

struct SessionWithTopic {
    session: i64,
    topic: i64,
}

fn seed_session_with_topic(ctx: &Ctx) -> SessionWithTopic {
    let s = ctx.run(&["add", "Session", "session_id=s1", "title=Fix JWT", "tool=claude", "model=opus", "project_path=/app"])
        .success().ref_id();
    let t = ctx.run(&["add", "Topic", "name=auth"]).success().ref_id();
    ctx.run(&["link", &format!("Session:{s}"), "HAS_TOPIC", &format!("Topic:{t}")]).success();
    SessionWithTopic { session: s, topic: t }
}

#[test]
fn get_shows_properties_and_edges() {
    let ctx = Ctx::new();
    let d = seed_session_with_topic(&ctx);

    let out = ctx.run(&["get", &format!("Session:{}", d.session)]);
    out.success()
        .stdout_line_at(0, "first line is the ref + label", |l| {
            l.starts_with(&format!("Session:{}", d.session)) && l.contains("Fix JWT")
        })
        .stdout_any_line("properties line has tool=claude", |l| l.contains("tool=claude"))
        .stdout_any_line("edge line references topic", |l| {
            l.contains("HAS_TOPIC") && l.contains(&format!("Topic:{}", d.topic))
        });
}

#[test]
fn set_merges_properties() {
    let ctx = Ctx::new();
    let d = seed_session_with_topic(&ctx);

    ctx.run(&["set", &format!("Session:{}", d.session), "summary=Done"]).success();
    ctx.run(&["get", &format!("Session:{}", d.session)])
        .success()
        .stdout_contains("Done")
        .stdout_contains("Fix JWT");
}

#[test]
fn set_rejects_unknown_property() {
    let ctx = Ctx::new();
    let d = seed_session_with_topic(&ctx);
    ctx.run(&["set", &format!("Session:{}", d.session), "mood=sad"])
        .failure()
        .stderr_contains("unknown property");
}

#[test]
fn rm_cascades_edges() {
    let ctx = Ctx::new();
    let d = seed_session_with_topic(&ctx);

    ctx.run(&["rm", &format!("Session:{}", d.session)]).success();
    ctx.run(&["get", &format!("Session:{}", d.session)]).failure();
    ctx.run(&["edges", &format!("Topic:{}", d.topic)])
        .success()
        .stdout_not_contains(&format!("Session:{}", d.session));
}
