use serde_json::json;

use crate::cli::plan::resolve_plan_id;
use crate::db::{connection, plan_repo, task_repo};
use crate::error::TaskaiError;
use crate::output;

pub fn run(json_output: bool, plan_flag: Option<&str>) -> i32 {
    let result = run_inner(json_output, plan_flag);
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

fn run_inner(json_output: bool, plan_flag: Option<&str>) -> Result<i32, TaskaiError> {
    let conn = connection::open_db()?;
    let plan_id = resolve_plan_id(&conn, plan_flag)?;
    let plan = plan_repo::get_plan_by_id(&conn, &plan_id)?;
    let tasks = task_repo::list_tasks_by_plan(&conn, &plan_id)?;
    let progress = task_repo::task_progress(&conn, &plan_id)?;
    let in_progress = task_repo::in_progress_tasks(&conn, &plan_id)?;

    let plan_completed = progress.ready == 0 && progress.blocked == 0 && progress.in_progress == 0;

    if json_output {
        let in_progress_json: Vec<_> = in_progress.iter().map(|t| {
            let elapsed = crate::cli::next::elapsed_minutes_pub(t.started_at.as_deref());
            output::json::in_progress_entry(t, elapsed)
        }).collect();
        let tasks_json: Vec<_> = tasks.iter().map(|t| output::json::task_summary(t)).collect();

        println!("{}", serde_json::to_string_pretty(
            &output::json::success_with_plan_completed(json!({
                "plan": output::json::plan_json(&plan),
                "tasks": tasks_json,
                "in_progress": in_progress_json,
                "progress": output::json::progress_json(&progress)
            }), plan_completed)
        ).unwrap());
    } else {
        output::text::print_plan(&plan);
        println!();
        output::text::print_progress(&progress);
        if plan_completed {
            println!("\nPlan completed!");
        }
        if !in_progress.is_empty() {
            println!("\nIn progress:");
            for t in &in_progress {
                let assigned = t.assigned_to.as_deref().unwrap_or("?");
                println!("  {} - {} (@{})", t.id, t.title, assigned);
            }
        }
        println!("\nAll tasks:");
        output::text::print_task_list(&tasks);
    }
    Ok(0)
}
