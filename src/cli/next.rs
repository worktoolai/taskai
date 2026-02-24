use serde_json::json;

use crate::cli::plan::resolve_plan_id;
use crate::db::{connection, task_repo, dependency_repo};
use crate::error::TaskaiError;
use crate::graph::next_tasks;
use crate::models::TaskStatus;
use crate::output;

pub fn run(claim: bool, agent: Option<&str>, json_output: bool, plan_flag: Option<&str>) -> i32 {
    let result = run_inner(claim, agent, json_output, plan_flag);
    match result {
        Ok(code) => code,
        Err(e) => {
            if json_output {
                println!("{}", serde_json::to_string_pretty(&output::json::error(&e)).unwrap());
            } else {
                eprintln!("Error: {}", e.message);
            }
            1
        }
    }
}

fn run_inner(claim: bool, agent: Option<&str>, json_output: bool, plan_flag: Option<&str>) -> Result<i32, TaskaiError> {
    let conn = connection::open_db()?;
    let plan_id = resolve_plan_id(&conn, plan_flag)?;
    let progress = task_repo::task_progress(&conn, &plan_id)?;

    let plan_completed = progress.ready == 0 && progress.blocked == 0 && progress.in_progress == 0;

    if plan_completed {
        if json_output {
            println!("{}", serde_json::to_string_pretty(
                &output::json::success_with_plan_completed(
                    json!({ "progress": output::json::progress_json(&progress) }),
                    true,
                )
            ).unwrap());
        } else {
            println!("Plan completed!");
            output::text::print_progress(&progress);
        }
        return Ok(0);
    }

    // Get in_progress tasks
    let in_progress = task_repo::in_progress_tasks(&conn, &plan_id)?;
    let in_progress_json: Vec<_> = in_progress.iter().map(|t| {
        let elapsed = elapsed_minutes(t.started_at.as_deref());
        output::json::in_progress_entry(t, elapsed)
    }).collect();

    // Get/claim next ready task
    let task = if claim {
        conn.execute_batch("BEGIN IMMEDIATE")?;
        let result = next_tasks::claim_next_task(&conn, &plan_id, agent);
        match result {
            Ok(task) => {
                conn.execute_batch("COMMIT")?;
                task
            }
            Err(e) => {
                let _ = conn.execute_batch("ROLLBACK");
                return Err(e);
            }
        }
    } else {
        task_repo::next_ready_task(&conn, &plan_id)?
    };

    if let Some(ref t) = task {
        let has_docs = task_repo::task_has_documents(&conn, &t.id)?;

        if json_output {
            // Re-fetch progress after potential claim
            let progress = task_repo::task_progress(&conn, &plan_id)?;
            let plan_completed = progress.ready == 0 && progress.blocked == 0 && progress.in_progress == 0;
            println!("{}", serde_json::to_string_pretty(
                &output::json::success_with_plan_completed(json!({
                    "task": output::json::task_detail(t, has_docs),
                    "in_progress": in_progress_json,
                    "progress": output::json::progress_json(&progress)
                }), plan_completed)
            ).unwrap());
        } else {
            println!("Next task: {} ({})", t.title, t.id);
            if let Some(ref desc) = t.description {
                println!("  {desc}");
            }
            println!("  Status: {}", t.status.as_str());
            if has_docs {
                println!("  (has documents - use `taskai task show {}` for details)", t.id);
            }
        }
        return Ok(0);
    }

    // No ready task — check if blocked remain
    if progress.blocked > 0 {
        if json_output {
            let blocked_tasks = get_blocked_tasks_detail(&conn, &plan_id)?;
            println!("{}", serde_json::to_string_pretty(
                &output::json::success_with_plan_completed(json!({
                    "task": null,
                    "reason": "BLOCKED_REMAINING",
                    "blocked_tasks": blocked_tasks,
                    "in_progress": in_progress_json,
                    "progress": output::json::progress_json(&progress)
                }), false)
            ).unwrap());
        } else {
            println!("No ready tasks. {} blocked tasks remaining.", progress.blocked);
            if !in_progress.is_empty() {
                println!("In progress:");
                for t in &in_progress {
                    let elapsed = elapsed_minutes(t.started_at.as_deref());
                    println!("  {} - {} ({}min)", t.id, t.title, elapsed);
                }
            }
        }
        return Ok(2);
    }

    // in_progress tasks exist but no ready/blocked
    if json_output {
        println!("{}", serde_json::to_string_pretty(
            &output::json::success_with_plan_completed(json!({
                "task": null,
                "reason": "ALL_IN_PROGRESS",
                "in_progress": in_progress_json,
                "progress": output::json::progress_json(&progress)
            }), false)
        ).unwrap());
    } else {
        println!("No ready tasks. {} in progress.", progress.in_progress);
    }
    Ok(2)
}

pub fn elapsed_minutes_pub(started_at: Option<&str>) -> i64 {
    elapsed_minutes(started_at)
}

fn elapsed_minutes(started_at: Option<&str>) -> i64 {
    let Some(started) = started_at else { return 0 };
    let Ok(started) = chrono::NaiveDateTime::parse_from_str(started, "%Y-%m-%d %H:%M:%S") else {
        return 0;
    };
    let now = chrono::Utc::now().naive_utc();
    (now - started).num_minutes()
}

fn get_blocked_tasks_detail(
    conn: &rusqlite::Connection,
    plan_id: &str,
) -> Result<Vec<serde_json::Value>, TaskaiError> {
    let tasks = task_repo::list_tasks_by_plan(conn, plan_id)?;
    let blocked: Vec<_> = tasks.iter().filter(|t| t.status == TaskStatus::Blocked).collect();

    let mut result = Vec::new();
    for t in blocked {
        let deps = dependency_repo::get_dependencies(conn, &t.id)?;
        let blocked_by: Vec<serde_json::Value> = deps
            .iter()
            .filter_map(|d| task_repo::get_task_by_id(conn, d).ok())
            .filter(|d| d.status != TaskStatus::Done)
            .map(|d| {
                json!({
                    "id": d.id,
                    "title": d.title,
                    "status": d.status.as_str()
                })
            })
            .collect();

        result.push(json!({
            "id": t.id,
            "title": t.title,
            "blocked_by": blocked_by
        }));
    }
    Ok(result)
}
