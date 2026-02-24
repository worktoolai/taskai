#[allow(deprecated)]
use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

// ─── helpers ───────────────────────────────────────────────────────

struct TestEnv {
    dir: TempDir,
}

impl TestEnv {
    fn new() -> Self {
        let dir = TempDir::new().expect("create tempdir");
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(dir.path())
            .output()
            .expect("git init");
        Self { dir }
    }

    fn cmd(&self) -> Command {
        let mut cmd = Command::cargo_bin("taskai").expect("binary");
        cmd.current_dir(self.dir.path());
        cmd
    }

    fn run_json(&self, args: &[&str]) -> Value {
        let mut a: Vec<&str> = args.to_vec();
        a.push("--json");
        let output = self.cmd().args(&a).output().expect("run");
        let stdout = String::from_utf8_lossy(&output.stdout);
        serde_json::from_str(&stdout)
            .unwrap_or_else(|e| panic!("parse JSON failed: {e}\nstdout: {stdout}"))
    }

    fn run_ok(&self, args: &[&str]) -> Value {
        let v = self.run_json(args);
        assert_eq!(v["success"], true, "expected success=true: {v}");
        v
    }

    fn run_err(&self, args: &[&str]) -> Value {
        let v = self.run_json(args);
        assert_eq!(v["success"], false, "expected success=false: {v}");
        v
    }

    fn write_plan(&self, filename: &str, content: &str) -> PathBuf {
        let p = self.dir.path().join(filename);
        fs::write(&p, content).expect("write plan file");
        p
    }

    fn load_plan(&self, content: &str) -> Value {
        let p = self.write_plan("_plan.json", content);
        let output = self
            .cmd()
            .args(["plan", "load", "--json"])
            .pipe_stdin(&p)
            .unwrap()
            .output()
            .expect("plan load");
        let stdout = String::from_utf8_lossy(&output.stdout);
        let v: Value = serde_json::from_str(&stdout)
            .unwrap_or_else(|e| panic!("parse JSON failed: {e}\nstdout: {stdout}"));
        assert_eq!(v["success"], true, "plan load failed: {v}");
        v
    }

    fn load_plan_raw(&self, content: &str) -> Value {
        let p = self.write_plan("_plan.json", content);
        let output = self
            .cmd()
            .args(["plan", "load", "--json"])
            .pipe_stdin(&p)
            .unwrap()
            .output()
            .expect("plan load");
        let stdout = String::from_utf8_lossy(&output.stdout);
        serde_json::from_str(&stdout)
            .unwrap_or_else(|e| panic!("parse JSON failed: {e}\nstdout: {stdout}"))
    }
}

fn basic_plan_json() -> String {
    serde_json::json!({
        "name": "test-plan",
        "title": "Test Plan",
        "description": "A test plan",
        "documents": [
            {"title": "Design Doc", "content": "## Design\nContent here"}
        ],
        "tasks": [
            {"id": "t1", "title": "First Task", "description": "Do first", "priority": 10,
             "documents": [{"title": "Task Doc", "content": "## Steps\n1. do it"}]},
            {"id": "t2", "title": "Second Task", "after": ["t1"]},
            {"id": "t3", "title": "Third Task", "after": ["t1"]},
            {"id": "t4", "title": "Final Task", "after": ["t2", "t3"]}
        ]
    })
    .to_string()
}

fn setup_with_plan(env: &TestEnv) -> Value {
    env.run_ok(&["init"]);
    env.load_plan(&basic_plan_json())
}

fn get_task_id(load_result: &Value, temp_id: &str) -> String {
    load_result["data"]["id_mapping"][temp_id]
        .as_str()
        .unwrap()
        .to_string()
}

// ─── 1. init ───────────────────────────────────────────────────────

#[test]
fn test_init() {
    let env = TestEnv::new();
    let v = env.run_ok(&["init"]);
    let path = v["data"]["path"].as_str().unwrap();
    assert!(path.ends_with(".worktoolai/taskai/taskai.db"));
    assert!(PathBuf::from(path).exists());
}

#[test]
fn test_init_idempotent() {
    let env = TestEnv::new();
    env.run_ok(&["init"]);
    let v = env.run_ok(&["init"]);
    assert!(v["data"]["path"].as_str().unwrap().contains("taskai.db"));
}

#[test]
fn test_init_required_before_commands() {
    let env = TestEnv::new();
    let v = env.run_err(&["plan", "list"]);
    assert_eq!(v["error"]["code"], "NOT_INITIALIZED");
}

// ─── 2. plan create / list / show / activate / delete ──────────────

#[test]
fn test_plan_crud() {
    let env = TestEnv::new();
    env.run_ok(&["init"]);

    // create
    let v = env.run_ok(&[
        "plan", "create", "my-plan", "--title", "My Plan", "--description", "desc",
    ]);
    let plan_id = v["data"]["id"].as_str().unwrap().to_string();
    assert_eq!(v["data"]["name"], "my-plan");
    assert_eq!(v["data"]["title"], "My Plan");

    // list
    let v = env.run_ok(&["plan", "list"]);
    let plans = v["data"]["plans"].as_array().unwrap();
    assert_eq!(plans.len(), 1);
    assert_eq!(plans[0]["name"], "my-plan");

    // show
    let v = env.run_ok(&["plan", "show", "my-plan"]);
    assert_eq!(v["data"]["plan"]["id"], plan_id);
    assert_eq!(v["data"]["plan"]["description"], "desc");

    // activate
    let v = env.run_ok(&["plan", "activate", "my-plan"]);
    assert_eq!(v["data"]["activated"]["name"], "my-plan");

    // delete
    let v = env.run_ok(&["plan", "delete", "my-plan"]);
    assert_eq!(v["data"]["deleted"]["name"], "my-plan");

    // list empty
    let v = env.run_ok(&["plan", "list"]);
    assert_eq!(v["data"]["plans"].as_array().unwrap().len(), 0);
}

#[test]
fn test_plan_name_validation() {
    let env = TestEnv::new();
    env.run_ok(&["init"]);
    let v = env.run_err(&["plan", "create", "UPPER"]);
    assert_eq!(v["error"]["code"], "VALIDATION_ERROR");
    // "-bad" is parsed by clap as a flag, so test with trailing hyphen instead
    let v = env.run_err(&["plan", "create", "bad-"]);
    assert_eq!(v["error"]["code"], "VALIDATION_ERROR");
    let v = env.run_err(&["plan", "create", "has spaces"]);
    assert_eq!(v["error"]["code"], "VALIDATION_ERROR");
}

#[test]
fn test_plan_name_conflict() {
    let env = TestEnv::new();
    env.run_ok(&["init"]);
    env.run_ok(&["plan", "create", "dup"]);
    let v = env.run_err(&["plan", "create", "dup"]);
    assert_eq!(v["error"]["code"], "PLAN_NAME_CONFLICT");
}

#[test]
fn test_plan_resolve_ambiguous() {
    let env = TestEnv::new();
    env.run_ok(&["init"]);
    env.run_ok(&["plan", "create", "foo-bar"]);
    env.run_ok(&["plan", "create", "foo-baz"]);
    let v = env.run_err(&["plan", "show", "foo"]);
    assert_eq!(v["error"]["code"], "AMBIGUOUS_REF");
}

#[test]
fn test_plan_not_found() {
    let env = TestEnv::new();
    env.run_ok(&["init"]);
    let v = env.run_err(&["plan", "show", "nope"]);
    assert_eq!(v["error"]["code"], "PLAN_NOT_FOUND");
}

// ─── 3. plan load ──────────────────────────────────────────────────

#[test]
fn test_plan_load_basic() {
    let env = TestEnv::new();
    env.run_ok(&["init"]);
    let v = env.load_plan(&basic_plan_json());

    assert_eq!(v["data"]["tasks_created"], 4);
    assert_eq!(v["data"]["plan"]["name"], "test-plan");

    let mapping = v["data"]["id_mapping"].as_object().unwrap();
    assert!(mapping.contains_key("t1"));
    assert!(mapping.contains_key("t2"));
    assert!(mapping.contains_key("t3"));
    assert!(mapping.contains_key("t4"));

    let ready = v["data"]["ready_now"].as_array().unwrap();
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0]["title"], "First Task");
}

#[test]
fn test_plan_load_auto_activate() {
    let env = TestEnv::new();
    env.run_ok(&["init"]);
    env.load_plan(&basic_plan_json());

    let v = env.run_ok(&["next"]);
    assert!(v["data"]["task"].is_object());
}

#[test]
fn test_plan_load_duplicate_name_rejected() {
    let env = TestEnv::new();
    env.run_ok(&["init"]);
    env.load_plan(&basic_plan_json());
    let v = env.load_plan_raw(&basic_plan_json());
    assert_eq!(v["success"], false);
    assert_eq!(v["error"]["code"], "PLAN_NAME_CONFLICT");
}

#[test]
fn test_plan_load_cycle_rejected() {
    let env = TestEnv::new();
    env.run_ok(&["init"]);
    let json = serde_json::json!({
        "name":"cyc","title":"Cycle","tasks":[
            {"id":"a","title":"A","after":["b"]},
            {"id":"b","title":"B","after":["a"]}
        ]
    })
    .to_string();
    let v = env.load_plan_raw(&json);
    assert_eq!(v["success"], false);
    assert_eq!(v["error"]["code"], "CYCLE_DETECTED");
}

#[test]
fn test_plan_load_self_reference_rejected() {
    let env = TestEnv::new();
    env.run_ok(&["init"]);
    let json = serde_json::json!({
        "name":"selfref","title":"Self","tasks":[
            {"id":"a","title":"A","after":["a"]}
        ]
    })
    .to_string();
    let v = env.load_plan_raw(&json);
    assert_eq!(v["success"], false);
    assert_eq!(v["error"]["code"], "VALIDATION_ERROR");
    assert!(v["error"]["message"]
        .as_str()
        .unwrap()
        .contains("depends on itself"));
}

#[test]
fn test_plan_load_duplicate_task_id_rejected() {
    let env = TestEnv::new();
    env.run_ok(&["init"]);
    let json = r#"{"name":"dupid","title":"Dup","tasks":[
        {"id":"x","title":"A"},
        {"id":"x","title":"B"}
    ]}"#;
    let v = env.load_plan_raw(json);
    assert_eq!(v["success"], false);
    assert_eq!(v["error"]["code"], "VALIDATION_ERROR");
    assert!(v["error"]["message"]
        .as_str()
        .unwrap()
        .contains("Duplicate"));
}

#[test]
fn test_plan_load_unknown_after_rejected() {
    let env = TestEnv::new();
    env.run_ok(&["init"]);
    let json = serde_json::json!({
        "name":"unk","title":"Unk","tasks":[
            {"id":"a","title":"A","after":["nonexistent"]}
        ]
    })
    .to_string();
    let v = env.load_plan_raw(&json);
    assert_eq!(v["success"], false);
    assert_eq!(v["error"]["code"], "VALIDATION_ERROR");
    assert!(v["error"]["message"]
        .as_str()
        .unwrap()
        .contains("unknown dependency"));
}

#[test]
fn test_plan_load_empty_title_rejected() {
    let env = TestEnv::new();
    env.run_ok(&["init"]);
    let json = r#"{"name":"et","title":"Et","tasks":[
        {"id":"a","title":""}
    ]}"#;
    let v = env.load_plan_raw(json);
    assert_eq!(v["success"], false);
    assert_eq!(v["error"]["code"], "VALIDATION_ERROR");
}

#[test]
fn test_plan_load_preserves_sort_order() {
    let env = TestEnv::new();
    env.run_ok(&["init"]);
    let json = serde_json::json!({
        "name":"order","title":"Order","tasks":[
            {"id":"a","title":"Alpha"},
            {"id":"b","title":"Bravo"},
            {"id":"c","title":"Charlie"}
        ]
    })
    .to_string();
    env.load_plan(&json);
    let v = env.run_ok(&["task", "list"]);
    let tasks = v["data"]["tasks"].as_array().unwrap();
    assert_eq!(tasks[0]["title"], "Alpha");
    assert_eq!(tasks[1]["title"], "Bravo");
    assert_eq!(tasks[2]["title"], "Charlie");
}

#[test]
fn test_plan_load_documents() {
    let env = TestEnv::new();
    env.run_ok(&["init"]);
    env.load_plan(&basic_plan_json());

    let v = env.run_ok(&["plan", "show", "test-plan"]);
    let docs = v["data"]["documents"].as_array().unwrap();
    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0]["title"], "Design Doc");

    let json2 = serde_json::json!({
        "name":"doc2","title":"D","tasks":[
            {"id":"x","title":"X","documents":[{"title":"TD","content":"body"}]}
        ]
    })
    .to_string();
    let mapping_v = env.load_plan(&json2);
    let task_id = mapping_v["data"]["id_mapping"]["x"].as_str().unwrap();
    let v = env.run_ok(&["task", "show", task_id, "--plan", "doc2"]);
    let docs = v["data"]["documents"].as_array().unwrap();
    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0]["title"], "TD");
}

// ─── 4. task state machine ─────────────────────────────────────────

#[test]
fn test_task_start() {
    let env = TestEnv::new();
    let loaded = setup_with_plan(&env);
    let t1 = get_task_id(&loaded, "t1");

    let v = env.run_ok(&["task", "start", &t1, "--agent", "bot-1"]);
    assert_eq!(v["data"]["completed_task"]["status"], "in_progress");
}

#[test]
fn test_task_done_from_ready() {
    let env = TestEnv::new();
    let loaded = setup_with_plan(&env);
    let t1 = get_task_id(&loaded, "t1");

    let v = env.run_ok(&["task", "done", &t1]);
    assert_eq!(v["data"]["completed_task"]["status"], "done");
}

#[test]
fn test_task_done_from_in_progress() {
    let env = TestEnv::new();
    let loaded = setup_with_plan(&env);
    let t1 = get_task_id(&loaded, "t1");

    env.run_ok(&["task", "start", &t1]);
    let v = env.run_ok(&["task", "done", &t1]);
    assert_eq!(v["data"]["completed_task"]["status"], "done");
}

#[test]
fn test_task_fail_retries() {
    let env = TestEnv::new();
    let loaded = setup_with_plan(&env);
    let t1 = get_task_id(&loaded, "t1");

    env.run_ok(&["task", "start", &t1]);
    let v = env.run_ok(&["task", "fail", &t1]);
    assert_eq!(v["data"]["completed_task"]["status"], "ready");

    let v = env.run_ok(&["task", "start", &t1]);
    assert_eq!(v["data"]["completed_task"]["status"], "in_progress");
}

#[test]
fn test_task_skip_from_ready() {
    let env = TestEnv::new();
    let loaded = setup_with_plan(&env);
    let t1 = get_task_id(&loaded, "t1");

    let v = env.run_ok(&["task", "skip", &t1]);
    assert_eq!(v["data"]["completed_task"]["status"], "skipped");
}

#[test]
fn test_task_skip_from_blocked() {
    let env = TestEnv::new();
    let loaded = setup_with_plan(&env);
    let t2 = get_task_id(&loaded, "t2");

    let v = env.run_ok(&["task", "skip", &t2]);
    assert_eq!(v["data"]["completed_task"]["status"], "skipped");
}

#[test]
fn test_task_cancel_from_ready() {
    let env = TestEnv::new();
    let loaded = setup_with_plan(&env);
    let t1 = get_task_id(&loaded, "t1");

    let v = env.run_ok(&["task", "cancel", &t1]);
    assert_eq!(v["data"]["completed_task"]["status"], "cancelled");
}

#[test]
fn test_task_cancel_from_blocked() {
    let env = TestEnv::new();
    let loaded = setup_with_plan(&env);
    let t2 = get_task_id(&loaded, "t2");

    let v = env.run_ok(&["task", "cancel", &t2]);
    assert_eq!(v["data"]["completed_task"]["status"], "cancelled");
}

#[test]
fn test_task_cancel_from_in_progress() {
    let env = TestEnv::new();
    let loaded = setup_with_plan(&env);
    let t1 = get_task_id(&loaded, "t1");

    env.run_ok(&["task", "start", &t1]);
    let v = env.run_ok(&["task", "cancel", &t1]);
    assert_eq!(v["data"]["completed_task"]["status"], "cancelled");
}

// ─── 4b. forbidden transitions ─────────────────────────────────────

#[test]
fn test_cannot_start_blocked_task() {
    let env = TestEnv::new();
    let loaded = setup_with_plan(&env);
    let t2 = get_task_id(&loaded, "t2");

    let v = env.run_err(&["task", "start", &t2]);
    assert_eq!(v["error"]["code"], "INVALID_STATUS_TRANSITION");
}

#[test]
fn test_cannot_done_blocked_task() {
    let env = TestEnv::new();
    let loaded = setup_with_plan(&env);
    let t2 = get_task_id(&loaded, "t2");

    let v = env.run_err(&["task", "done", &t2]);
    assert_eq!(v["error"]["code"], "INVALID_STATUS_TRANSITION");
}

#[test]
fn test_cannot_fail_ready_task() {
    let env = TestEnv::new();
    let loaded = setup_with_plan(&env);
    let t1 = get_task_id(&loaded, "t1");

    let v = env.run_err(&["task", "fail", &t1]);
    assert_eq!(v["error"]["code"], "INVALID_STATUS_TRANSITION");
}

#[test]
fn test_cannot_transition_from_done() {
    let env = TestEnv::new();
    let loaded = setup_with_plan(&env);
    let t1 = get_task_id(&loaded, "t1");

    env.run_ok(&["task", "done", &t1]);
    for action in &["start", "done", "fail", "skip", "cancel"] {
        let v = env.run_err(&["task", action, &t1]);
        assert_eq!(
            v["error"]["code"], "INVALID_STATUS_TRANSITION",
            "transition from done via {action} should fail"
        );
    }
}

#[test]
fn test_cannot_transition_from_skipped() {
    let env = TestEnv::new();
    let loaded = setup_with_plan(&env);
    let t1 = get_task_id(&loaded, "t1");

    env.run_ok(&["task", "skip", &t1]);
    let v = env.run_err(&["task", "done", &t1]);
    assert_eq!(v["error"]["code"], "INVALID_STATUS_TRANSITION");
}

#[test]
fn test_cannot_transition_from_cancelled() {
    let env = TestEnv::new();
    let loaded = setup_with_plan(&env);
    let t1 = get_task_id(&loaded, "t1");

    env.run_ok(&["task", "cancel", &t1]);
    let v = env.run_err(&["task", "done", &t1]);
    assert_eq!(v["error"]["code"], "INVALID_STATUS_TRANSITION");
}

// ─── 5. cascade unblock ────────────────────────────────────────────

#[test]
fn test_cascade_unblock_on_done() {
    let env = TestEnv::new();
    let loaded = setup_with_plan(&env);
    let t1 = get_task_id(&loaded, "t1");

    let v = env.run_ok(&["task", "done", &t1]);
    let newly_ready = v["data"]["newly_ready"].as_array().unwrap();
    assert_eq!(newly_ready.len(), 2);
    let titles: Vec<&str> = newly_ready
        .iter()
        .map(|t| t["title"].as_str().unwrap())
        .collect();
    assert!(titles.contains(&"Second Task"));
    assert!(titles.contains(&"Third Task"));
}

#[test]
fn test_cascade_unblock_requires_all_deps_done() {
    let env = TestEnv::new();
    let loaded = setup_with_plan(&env);
    let t1 = get_task_id(&loaded, "t1");
    let t2 = get_task_id(&loaded, "t2");
    let t3 = get_task_id(&loaded, "t3");

    env.run_ok(&["task", "done", &t1]);
    let v = env.run_ok(&["task", "done", &t2]);
    let has_final = v["data"]
        .get("newly_ready")
        .and_then(|nr| nr.as_array())
        .map(|arr| arr.iter().any(|t| t["title"] == "Final Task"))
        .unwrap_or(false);
    assert!(!has_final, "t4 should not be unblocked yet");

    let v = env.run_ok(&["task", "done", &t3]);
    let newly_ready = v["data"]["newly_ready"].as_array().unwrap();
    assert_eq!(newly_ready.len(), 1);
    assert_eq!(newly_ready[0]["title"], "Final Task");
}

#[test]
fn test_cancelled_predecessor_does_not_unblock() {
    let env = TestEnv::new();
    env.run_ok(&["init"]);
    let json = serde_json::json!({
        "name":"cancel-block","title":"CB","tasks":[
            {"id":"a","title":"A"},
            {"id":"b","title":"B","after":["a"]}
        ]
    })
    .to_string();
    let loaded = env.load_plan(&json);
    let a = get_task_id(&loaded, "a");

    env.run_ok(&["task", "cancel", &a]);

    let v = env.run_ok(&["task", "list"]);
    let tasks = v["data"]["tasks"].as_array().unwrap();
    let b = tasks.iter().find(|t| t["title"] == "B").unwrap();
    assert_eq!(b["status"], "blocked");
}

#[test]
fn test_skipped_predecessor_does_not_unblock() {
    let env = TestEnv::new();
    env.run_ok(&["init"]);
    let json = serde_json::json!({
        "name":"skip-block","title":"SB","tasks":[
            {"id":"a","title":"A"},
            {"id":"b","title":"B","after":["a"]}
        ]
    })
    .to_string();
    let loaded = env.load_plan(&json);
    let a = get_task_id(&loaded, "a");

    env.run_ok(&["task", "skip", &a]);

    let v = env.run_ok(&["task", "list"]);
    let tasks = v["data"]["tasks"].as_array().unwrap();
    let b = tasks.iter().find(|t| t["title"] == "B").unwrap();
    assert_eq!(b["status"], "blocked");
}

// ─── 6. next command ───────────────────────────────────────────────

#[test]
fn test_next_returns_highest_priority() {
    let env = TestEnv::new();
    env.run_ok(&["init"]);
    let json = serde_json::json!({
        "name":"prio","title":"P","tasks":[
            {"id":"lo","title":"Low","priority":1},
            {"id":"hi","title":"High","priority":99}
        ]
    })
    .to_string();
    env.load_plan(&json);

    let v = env.run_ok(&["next"]);
    assert_eq!(v["data"]["task"]["title"], "High");
}

#[test]
fn test_next_respects_sort_order_when_same_priority() {
    let env = TestEnv::new();
    env.run_ok(&["init"]);
    let json = serde_json::json!({
        "name":"sort","title":"S","tasks":[
            {"id":"first","title":"First"},
            {"id":"second","title":"Second"},
            {"id":"third","title":"Third"}
        ]
    })
    .to_string();
    env.load_plan(&json);

    let v = env.run_ok(&["next"]);
    assert_eq!(v["data"]["task"]["title"], "First");
}

#[test]
fn test_next_claim_sets_in_progress() {
    let env = TestEnv::new();
    let loaded = setup_with_plan(&env);
    let t1 = get_task_id(&loaded, "t1");

    let v = env.run_ok(&["next", "--claim", "--agent", "bot-1"]);
    assert_eq!(v["data"]["task"]["status"], "in_progress");
    assert_eq!(v["data"]["task"]["assigned_to"], "bot-1");
    assert_eq!(v["data"]["task"]["id"], t1);
}

#[test]
fn test_next_without_claim_does_not_change_status() {
    let env = TestEnv::new();
    let loaded = setup_with_plan(&env);
    let t1 = get_task_id(&loaded, "t1");

    let v = env.run_ok(&["next"]);
    assert_eq!(v["data"]["task"]["status"], "ready");

    let v = env.run_ok(&["task", "show", &t1]);
    assert_eq!(v["data"]["task"]["status"], "ready");
}

#[test]
fn test_next_claim_atomic_different_tasks() {
    let env = TestEnv::new();
    env.run_ok(&["init"]);
    let json = serde_json::json!({
        "name":"multi","title":"M","tasks":[
            {"id":"a","title":"A"},
            {"id":"b","title":"B"}
        ]
    })
    .to_string();
    env.load_plan(&json);

    let v1 = env.run_ok(&["next", "--claim", "--agent", "agent-1"]);
    let v2 = env.run_ok(&["next", "--claim", "--agent", "agent-2"]);

    let id1 = v1["data"]["task"]["id"].as_str().unwrap();
    let id2 = v2["data"]["task"]["id"].as_str().unwrap();
    assert_ne!(id1, id2, "two claims should return different tasks");
}

#[test]
fn test_next_includes_in_progress_list() {
    let env = TestEnv::new();
    env.run_ok(&["init"]);
    let json = serde_json::json!({
        "name":"ip","title":"IP","tasks":[
            {"id":"a","title":"A"},
            {"id":"b","title":"B"}
        ]
    })
    .to_string();
    env.load_plan(&json);

    env.run_ok(&["next", "--claim", "--agent", "bot"]);
    let v = env.run_ok(&["next"]);
    let in_progress = v["data"]["in_progress"].as_array().unwrap();
    assert_eq!(in_progress.len(), 1);
    assert_eq!(in_progress[0]["title"], "A");
    assert!(in_progress[0]["elapsed_minutes"].is_number());
}

#[test]
fn test_next_has_documents_flag() {
    let env = TestEnv::new();
    let _loaded = setup_with_plan(&env);
    let v = env.run_ok(&["next"]);
    assert_eq!(v["data"]["task"]["has_documents"], true);
}

#[test]
fn test_next_no_active_plan() {
    let env = TestEnv::new();
    env.run_ok(&["init"]);
    let v = env.run_err(&["next"]);
    assert_eq!(v["error"]["code"], "NO_ACTIVE_PLAN");
}

// ─── 7. plan_completed flag ────────────────────────────────────────

#[test]
fn test_plan_completed_all_done() {
    let env = TestEnv::new();
    env.run_ok(&["init"]);
    let json = serde_json::json!({
        "name":"comp","title":"C","tasks":[
            {"id":"a","title":"A"},
            {"id":"b","title":"B","after":["a"]}
        ]
    })
    .to_string();
    let loaded = env.load_plan(&json);
    let a = get_task_id(&loaded, "a");
    let b = get_task_id(&loaded, "b");

    let v = env.run_ok(&["task", "done", &a]);
    assert_eq!(v["plan_completed"], false);

    let v = env.run_ok(&["task", "done", &b]);
    assert_eq!(v["plan_completed"], true);
    assert_eq!(v["data"]["progress"]["percentage"], 100.0);
}

#[test]
fn test_plan_completed_mix_done_skipped_cancelled() {
    let env = TestEnv::new();
    env.run_ok(&["init"]);
    let json = serde_json::json!({
        "name":"mix","title":"M","tasks":[
            {"id":"a","title":"A"},
            {"id":"b","title":"B"},
            {"id":"c","title":"C"}
        ]
    })
    .to_string();
    let loaded = env.load_plan(&json);
    let a = get_task_id(&loaded, "a");
    let b = get_task_id(&loaded, "b");
    let c = get_task_id(&loaded, "c");

    env.run_ok(&["task", "done", &a]);
    env.run_ok(&["task", "skip", &b]);
    let v = env.run_ok(&["task", "cancel", &c]);
    assert_eq!(v["plan_completed"], true);
}

#[test]
fn test_plan_completed_on_next() {
    let env = TestEnv::new();
    env.run_ok(&["init"]);
    let json = serde_json::json!({
        "name":"comp2","title":"C2","tasks":[{"id":"a","title":"A"}]
    })
    .to_string();
    let loaded = env.load_plan(&json);
    env.run_ok(&["task", "done", &get_task_id(&loaded, "a")]);
    let v = env.run_ok(&["next"]);
    assert_eq!(v["plan_completed"], true);
}

// ─── 8. next with BLOCKED_REMAINING ────────────────────────────────

#[test]
fn test_next_blocked_remaining() {
    let env = TestEnv::new();
    env.run_ok(&["init"]);
    let json = serde_json::json!({
        "name":"blk","title":"B","tasks":[
            {"id":"a","title":"A"},
            {"id":"b","title":"B","after":["a"]}
        ]
    })
    .to_string();
    let loaded = env.load_plan(&json);
    let a = get_task_id(&loaded, "a");

    env.run_ok(&["task", "cancel", &a]);

    let output = env.cmd().args(["next", "--json"]).output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    let v: Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).unwrap();
    assert_eq!(v["success"], true);
    assert_eq!(v["plan_completed"], false);
    assert_eq!(v["data"]["reason"], "BLOCKED_REMAINING");
    assert!(v["data"]["task"].is_null());
    let blocked = v["data"]["blocked_tasks"].as_array().unwrap();
    assert_eq!(blocked.len(), 1);
    assert_eq!(blocked[0]["title"], "B");
    let blocked_by = blocked[0]["blocked_by"].as_array().unwrap();
    assert_eq!(blocked_by[0]["status"], "cancelled");
}

// ─── 9. exit codes ─────────────────────────────────────────────────

#[test]
fn test_exit_code_0_on_success() {
    let env = TestEnv::new();
    let output = env.cmd().args(["init", "--json"]).output().unwrap();
    assert_eq!(output.status.code(), Some(0));
}

#[test]
fn test_exit_code_1_on_error() {
    let env = TestEnv::new();
    let output = env
        .cmd()
        .args(["plan", "list", "--json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
}

#[test]
fn test_exit_code_2_on_blocked_remaining() {
    let env = TestEnv::new();
    env.run_ok(&["init"]);
    let json = serde_json::json!({
        "name":"ec","title":"EC","tasks":[
            {"id":"a","title":"A"},
            {"id":"b","title":"B","after":["a"]}
        ]
    })
    .to_string();
    let loaded = env.load_plan(&json);
    env.run_ok(&["task", "cancel", &get_task_id(&loaded, "a")]);

    let output = env.cmd().args(["next", "--json"]).output().unwrap();
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn test_exit_code_0_on_plan_completed() {
    let env = TestEnv::new();
    env.run_ok(&["init"]);
    let json = serde_json::json!({
        "name":"ec2","title":"EC2","tasks":[{"id":"a","title":"A"}]
    })
    .to_string();
    let loaded = env.load_plan(&json);
    env.run_ok(&["task", "done", &get_task_id(&loaded, "a")]);

    let output = env.cmd().args(["next", "--json"]).output().unwrap();
    assert_eq!(output.status.code(), Some(0));
}

// ─── 10. task add / dep add / dep remove ───────────────────────────

#[test]
fn test_task_add() {
    let env = TestEnv::new();
    env.run_ok(&["init"]);
    let json = serde_json::json!({
        "name":"add","title":"Add","tasks":[{"id":"a","title":"A"}]
    })
    .to_string();
    env.load_plan(&json);

    let v = env.run_ok(&[
        "task", "add", "New Task", "--priority", "5", "--description", "do it",
    ]);
    assert_eq!(v["data"]["task"]["title"], "New Task");
    assert_eq!(v["data"]["task"]["priority"], 5);
    assert_eq!(v["data"]["task"]["status"], "ready");
}

#[test]
fn test_task_add_with_after() {
    let env = TestEnv::new();
    env.run_ok(&["init"]);
    let json = serde_json::json!({
        "name":"addaf","title":"AF","tasks":[{"id":"a","title":"A"}]
    })
    .to_string();
    let loaded = env.load_plan(&json);
    let a = get_task_id(&loaded, "a");

    let v = env.run_ok(&["task", "add", "B", "--after", &a]);
    assert_eq!(v["data"]["task"]["status"], "blocked");
}

#[test]
fn test_dep_add_and_remove() {
    let env = TestEnv::new();
    env.run_ok(&["init"]);
    let json = serde_json::json!({
        "name":"dep","title":"Dep","tasks":[
            {"id":"a","title":"A"},
            {"id":"b","title":"B"}
        ]
    })
    .to_string();
    let loaded = env.load_plan(&json);
    let a = get_task_id(&loaded, "a");
    let b = get_task_id(&loaded, "b");

    let v = env.run_ok(&["task", "dep", "add", &b, &a]);
    assert!(v["data"]["added"].is_object());

    let v = env.run_ok(&["task", "show", &b]);
    assert_eq!(v["data"]["task"]["status"], "blocked");

    env.run_ok(&["task", "dep", "remove", &b, &a]);
    let v = env.run_ok(&["task", "show", &b]);
    assert_eq!(v["data"]["task"]["status"], "ready");
}

#[test]
fn test_dep_add_cycle_rejected() {
    let env = TestEnv::new();
    env.run_ok(&["init"]);
    let json = serde_json::json!({
        "name":"depcyc","title":"DC","tasks":[
            {"id":"a","title":"A"},
            {"id":"b","title":"B","after":["a"]}
        ]
    })
    .to_string();
    let loaded = env.load_plan(&json);
    let a = get_task_id(&loaded, "a");
    let b = get_task_id(&loaded, "b");

    let v = env.run_err(&["task", "dep", "add", &a, &b]);
    assert_eq!(v["error"]["code"], "CYCLE_DETECTED");
}

// ─── 11. status command ────────────────────────────────────────────

#[test]
fn test_status() {
    let env = TestEnv::new();
    let _loaded = setup_with_plan(&env);

    let v = env.run_ok(&["status"]);
    assert_eq!(v["data"]["plan"]["name"], "test-plan");
    assert_eq!(v["data"]["progress"]["total"], 4);
    assert_eq!(v["data"]["progress"]["ready"], 1);
    assert_eq!(v["data"]["progress"]["blocked"], 3);
    assert_eq!(v["plan_completed"], false);
}

#[test]
fn test_status_plan_completed() {
    let env = TestEnv::new();
    env.run_ok(&["init"]);
    let json = serde_json::json!({
        "name":"sc","title":"SC","tasks":[{"id":"a","title":"A"}]
    })
    .to_string();
    let loaded = env.load_plan(&json);
    env.run_ok(&["task", "done", &get_task_id(&loaded, "a")]);

    let v = env.run_ok(&["status"]);
    assert_eq!(v["plan_completed"], true);
}

// ─── 12. --plan flag ───────────────────────────────────────────────

#[test]
fn test_plan_flag_overrides_active() {
    let env = TestEnv::new();
    env.run_ok(&["init"]);
    let json_a = serde_json::json!({
        "name":"plan-a","title":"A","tasks":[{"id":"a","title":"Task A"}]
    })
    .to_string();
    let json_b = serde_json::json!({
        "name":"plan-b","title":"B","tasks":[{"id":"b","title":"Task B"}]
    })
    .to_string();
    env.load_plan(&json_a);
    env.load_plan(&json_b);

    let v = env.run_ok(&["next"]);
    assert_eq!(v["data"]["task"]["title"], "Task A");

    let v = env.run_ok(&["next", "--plan", "plan-b"]);
    assert_eq!(v["data"]["task"]["title"], "Task B");
}

// ─── 13. full orchestrator loop ────────────────────────────────────

#[test]
fn test_full_loop() {
    let env = TestEnv::new();
    env.run_ok(&["init"]);
    let json = serde_json::json!({
        "name": "loop-test",
        "title": "Loop Test",
        "tasks": [
            {"id": "t1", "title": "Step 1", "priority": 10},
            {"id": "t2", "title": "Step 2", "after": ["t1"], "priority": 5},
            {"id": "t3", "title": "Step 3", "after": ["t1"], "priority": 3},
            {"id": "t4", "title": "Step 4", "after": ["t2", "t3"]}
        ]
    })
    .to_string();
    let _loaded = env.load_plan(&json);

    let mut iterations = 0;
    loop {
        let v = env.run_ok(&["next", "--claim", "--agent", "loop-bot"]);
        if v["plan_completed"] == true {
            break;
        }
        if v["data"]["task"].is_null() {
            panic!("stuck: no task and not completed: {v}");
        }
        let task_id = v["data"]["task"]["id"].as_str().unwrap().to_string();
        let v = env.run_ok(&["task", "done", &task_id]);
        if v["plan_completed"] == true {
            break;
        }
        iterations += 1;
        assert!(iterations <= 10, "too many iterations");
    }

    let v = env.run_ok(&["status"]);
    assert_eq!(v["data"]["progress"]["done"], 4);
    assert_eq!(v["data"]["progress"]["percentage"], 100.0);
    assert_eq!(v["plan_completed"], true);
}

// ─── 15. bug regression tests ──────────────────────────────────────

#[test]
fn test_task_add_after_done_dep_is_ready() {
    // Fix #2: task add --after with already-done dep should be ready, not blocked
    let env = TestEnv::new();
    env.run_ok(&["init"]);
    let json = serde_json::json!({
        "name":"add-done","title":"AD","tasks":[{"id":"a","title":"A"}]
    })
    .to_string();
    let loaded = env.load_plan(&json);
    let a = get_task_id(&loaded, "a");
    env.run_ok(&["task", "done", &a]);

    let v = env.run_ok(&["task", "add", "B", "--after", &a]);
    assert_eq!(v["data"]["task"]["status"], "ready", "dep already done → ready");
}

#[test]
fn test_task_add_bad_dep_no_orphan() {
    // Fix #1: task add with invalid dep should not leave orphaned task
    let env = TestEnv::new();
    env.run_ok(&["init"]);
    let json = serde_json::json!({
        "name":"orphan","title":"O","tasks":[{"id":"a","title":"A"}]
    })
    .to_string();
    env.load_plan(&json);

    let v = env.run_err(&["task", "add", "B", "--after", "nonexistent"]);
    assert_eq!(v["error"]["code"], "TASK_NOT_FOUND");

    // Only original task should exist
    let v = env.run_ok(&["task", "list"]);
    let tasks = v["data"]["tasks"].as_array().unwrap();
    assert_eq!(tasks.len(), 1, "orphan task should not exist");
}

#[test]
fn test_dep_add_done_dep_stays_ready() {
    // Fix #3: dep add with already-done dep should keep task ready
    let env = TestEnv::new();
    env.run_ok(&["init"]);
    let json = serde_json::json!({
        "name":"dep-done","title":"DD","tasks":[
            {"id":"a","title":"A"},
            {"id":"b","title":"B"}
        ]
    })
    .to_string();
    let loaded = env.load_plan(&json);
    let a = get_task_id(&loaded, "a");
    let b = get_task_id(&loaded, "b");

    env.run_ok(&["task", "done", &a]);
    env.run_ok(&["task", "dep", "add", &b, &a]);

    let v = env.run_ok(&["task", "show", &b]);
    assert_eq!(v["data"]["task"]["status"], "ready", "dep is done → stay ready");
}

#[test]
fn test_delete_active_plan_clears_config() {
    // Fix #5: deleting active plan should not leave stale config
    let env = TestEnv::new();
    env.run_ok(&["init"]);
    let json = serde_json::json!({
        "name":"del","title":"Del","tasks":[{"id":"a","title":"A"}]
    })
    .to_string();
    env.load_plan(&json);
    env.run_ok(&["plan", "delete", "del"]);

    let v = env.run_err(&["next"]);
    assert_eq!(v["error"]["code"], "NO_ACTIVE_PLAN");
}

#[test]
fn test_plan_load_activates_when_stale_config() {
    // Fix #6: plan load should auto-activate when active plan ID is stale
    let env = TestEnv::new();
    env.run_ok(&["init"]);
    let json1 = serde_json::json!({
        "name":"first","title":"F","tasks":[{"id":"a","title":"A"}]
    })
    .to_string();
    env.load_plan(&json1);
    env.run_ok(&["plan", "delete", "first"]);
    // Config now points to deleted plan

    let json2 = serde_json::json!({
        "name":"second","title":"S","tasks":[{"id":"b","title":"B"}]
    })
    .to_string();
    env.load_plan(&json2);

    // Should auto-activate "second" since old active is stale
    let v = env.run_ok(&["next"]);
    assert_eq!(v["data"]["task"]["title"], "B");
}

// ─── 14. text output (non-json) ────────────────────────────────────

#[test]
fn test_text_output_init() {
    let env = TestEnv::new();
    env.cmd()
        .args(["init"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Initialized taskai at"));
}

#[test]
fn test_text_output_plan_list() {
    let env = TestEnv::new();
    env.cmd().args(["init"]).assert().success();
    env.cmd()
        .args(["plan", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No plans found"));
}

#[test]
fn test_text_output_error() {
    let env = TestEnv::new();
    env.cmd()
        .args(["plan", "list"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("not initialized"));
}
