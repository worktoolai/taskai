use serde_json::json;

use crate::cli::commands::TaskCommands;
use crate::cli::plan::resolve_plan_id;
use crate::db::{connection, task_repo, dependency_repo, document_repo};
use crate::error::TaskaiError;
use crate::graph::{cycle, next_tasks};
use crate::models::TaskStatus;
use crate::output;

pub fn run(cmd: TaskCommands, json_output: bool, plan_flag: Option<&str>) -> i32 {
    let result = match cmd {
        TaskCommands::Add { title, description, priority, agent, after } => {
            run_add(&title, description.as_deref(), priority, agent.as_deref(), &after, json_output, plan_flag)
        }
        TaskCommands::List => run_list(json_output, plan_flag),
        TaskCommands::Show { id } => run_show(&id, json_output, plan_flag),
        TaskCommands::Start { id, agent } => run_transition(&id, "start", agent.as_deref(), json_output, plan_flag),
        TaskCommands::Done { id } => run_transition(&id, "done", None, json_output, plan_flag),
        TaskCommands::Fail { id } => run_transition(&id, "fail", None, json_output, plan_flag),
        TaskCommands::Skip { id } => run_transition(&id, "skip", None, json_output, plan_flag),
        TaskCommands::Cancel { id } => run_transition(&id, "cancel", None, json_output, plan_flag),
        TaskCommands::Dep(dep_cmd) => run_dep(dep_cmd, json_output, plan_flag),
    };
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

fn run_add(
    title: &str,
    description: Option<&str>,
    priority: i32,
    agent: Option<&str>,
    after: &[String],
    json_output: bool,
    plan_flag: Option<&str>,
) -> Result<i32, TaskaiError> {
    let conn = connection::open_db()?;
    let plan_id = resolve_plan_id(&conn, plan_flag)?;

    // Resolve deps first (before any writes) to fail fast
    let mut resolved_deps = Vec::new();
    for dep_ref in after {
        let dep_task = task_repo::resolve_task(&conn, &plan_id, dep_ref)?;
        resolved_deps.push(dep_task);
    }

    // Determine sort_order (next available)
    let max_order: i32 = conn
        .query_row(
            "SELECT COALESCE(MAX(sort_order), -1) FROM tasks WHERE plan_id = ?1",
            rusqlite::params![plan_id],
            |row| row.get(0),
        )
        .unwrap_or(-1);

    let task_id = ulid::Ulid::new().to_string();

    // Atomic: create task + deps in transaction
    conn.execute_batch("BEGIN IMMEDIATE")?;
    let result = (|| -> Result<_, TaskaiError> {
        // Initial status: ready (will be corrected after deps are checked)
        task_repo::create_task(
            &conn, &task_id, &plan_id, title, description, priority,
            max_order + 1, &TaskStatus::Ready, agent,
        )?;

        for dep_task in &resolved_deps {
            dependency_repo::add_dependency(&conn, &task_id, &dep_task.id)?;
        }

        // Set correct status: blocked only if any dep is not done
        if !resolved_deps.is_empty() && !dependency_repo::all_dependencies_done(&conn, &task_id)? {
            task_repo::update_task_status(&conn, &task_id, &TaskStatus::Blocked, None)?;
        }

        Ok(())
    })();

    match result {
        Ok(()) => conn.execute_batch("COMMIT")?,
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK");
            return Err(e);
        }
    }

    let task = task_repo::get_task_by_id(&conn, &task_id)?;
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&output::json::success(json!({
                "task": output::json::task_summary(&task)
            })))
            .unwrap()
        );
    } else {
        println!("Added task: {} ({})", task.title, task.id);
    }
    Ok(0)
}

fn run_list(json_output: bool, plan_flag: Option<&str>) -> Result<i32, TaskaiError> {
    let conn = connection::open_db()?;
    let plan_id = resolve_plan_id(&conn, plan_flag)?;
    let tasks = task_repo::list_tasks_by_plan(&conn, &plan_id)?;

    if json_output {
        let tasks_json: Vec<_> = tasks.iter().map(|t| {
            let mut v = output::json::task_summary(t);
            if let Some(ref a) = t.agent {
                v["agent"] = json!(a);
            }
            if let Some(ref a) = t.assigned_to {
                v["assigned_to"] = json!(a);
            }
            v
        }).collect();
        let progress = task_repo::task_progress(&conn, &plan_id)?;
        println!(
            "{}",
            serde_json::to_string_pretty(&output::json::success(json!({
                "tasks": tasks_json,
                "progress": output::json::progress_json(&progress)
            })))
            .unwrap()
        );
    } else {
        output::text::print_task_list(&tasks);
    }
    Ok(0)
}

fn run_show(id: &str, json_output: bool, plan_flag: Option<&str>) -> Result<i32, TaskaiError> {
    let conn = connection::open_db()?;
    let plan_id = resolve_plan_id(&conn, plan_flag)?;
    let task = task_repo::resolve_task(&conn, &plan_id, id)?;
    let deps = dependency_repo::get_dependencies(&conn, &task.id)?;
    let docs = document_repo::get_task_documents(&conn, &task.id)?;

    if json_output {
        let dep_tasks: Vec<_> = deps
            .iter()
            .filter_map(|d| task_repo::get_task_by_id(&conn, d).ok())
            .map(|t| output::json::task_summary(&t))
            .collect();
        let docs_json: Vec<_> = docs.iter().map(|d| output::json::task_document_json(d)).collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&output::json::success(json!({
                "task": {
                    "id": task.id,
                    "title": task.title,
                    "description": task.description,
                    "status": task.status.as_str(),
                    "priority": task.priority,
                    "sort_order": task.sort_order,
                    "agent": task.agent,
                    "assigned_to": task.assigned_to,
                    "created_at": task.created_at,
                    "updated_at": task.updated_at,
                    "started_at": task.started_at,
                    "completed_at": task.completed_at,
                },
                "dependencies": dep_tasks,
                "documents": docs_json,
            })))
            .unwrap()
        );
    } else {
        output::text::print_task(&task);
        if !deps.is_empty() {
            println!("\nDependencies:");
            for d in &deps {
                if let Ok(dep_task) = task_repo::get_task_by_id(&conn, d) {
                    println!("  [{}] {} ({})", dep_task.status.as_str(), dep_task.title, dep_task.id);
                }
            }
        }
        if !docs.is_empty() {
            output::text::print_task_documents(&docs);
        }
    }
    Ok(0)
}

fn run_transition(
    id: &str,
    action: &str,
    agent: Option<&str>,
    json_output: bool,
    plan_flag: Option<&str>,
) -> Result<i32, TaskaiError> {
    let conn = connection::open_db()?;
    let plan_id = resolve_plan_id(&conn, plan_flag)?;
    let task = task_repo::resolve_task(&conn, &plan_id, id)?;

    let new_status = validate_transition(&task.status, action)?;

    conn.execute_batch("BEGIN IMMEDIATE")?;
    let result = (|| -> Result<_, TaskaiError> {
        // For fail: check if deps are still met before going back to ready
        let actual_status = if new_status == TaskStatus::Ready
            && action == "fail"
            && !dependency_repo::all_dependencies_done(&conn, &task.id)?
        {
            TaskStatus::Blocked
        } else {
            new_status.clone()
        };

        task_repo::update_task_status(&conn, &task.id, &actual_status, agent)?;

        let mut newly_ready = Vec::new();
        if actual_status == TaskStatus::Done {
            newly_ready = next_tasks::cascade_unblock(&conn, &task.id)?;
        }

        let updated_task = task_repo::get_task_by_id(&conn, &task.id)?;
        let progress = task_repo::task_progress(&conn, &plan_id)?;
        Ok((updated_task, newly_ready, progress))
    })();

    match result {
        Ok((updated_task, newly_ready, progress)) => {
            conn.execute_batch("COMMIT")?;

            let plan_completed = progress.ready == 0 && progress.blocked == 0 && progress.in_progress == 0;

            if json_output {
                let mut data = json!({
                    "completed_task": {
                        "id": updated_task.id,
                        "title": updated_task.title,
                        "status": updated_task.status.as_str()
                    },
                    "progress": output::json::progress_json(&progress)
                });
                if !newly_ready.is_empty() {
                    data["newly_ready"] = json!(newly_ready.iter().map(|t| json!({
                        "id": t.id,
                        "title": t.title,
                        "priority": t.priority
                    })).collect::<Vec<_>>());
                }
                println!("{}", serde_json::to_string_pretty(
                    &output::json::success_with_plan_completed(data, plan_completed)
                ).unwrap());
            } else {
                println!("Task {} → {}", updated_task.id, updated_task.status.as_str());
                if !newly_ready.is_empty() {
                    println!("Newly ready:");
                    for t in &newly_ready {
                        println!("  {} - {}", t.id, t.title);
                    }
                }
                if plan_completed {
                    println!("Plan completed!");
                }
            }
            Ok(0)
        }
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(e)
        }
    }
}

fn validate_transition(current: &TaskStatus, action: &str) -> Result<TaskStatus, TaskaiError> {
    match (current, action) {
        (TaskStatus::Ready, "start") => Ok(TaskStatus::InProgress),
        (TaskStatus::Ready, "done") => Ok(TaskStatus::Done),
        (TaskStatus::InProgress, "done") => Ok(TaskStatus::Done),
        (TaskStatus::InProgress, "fail") => Ok(TaskStatus::Ready),
        (TaskStatus::Ready | TaskStatus::Blocked, "skip") => Ok(TaskStatus::Skipped),
        (TaskStatus::Ready | TaskStatus::Blocked | TaskStatus::InProgress, "cancel") => {
            Ok(TaskStatus::Cancelled)
        }
        _ => Err(TaskaiError::invalid_transition(current.as_str(), action)),
    }
}

fn run_dep(
    cmd: crate::cli::commands::DepCommands,
    json_output: bool,
    plan_flag: Option<&str>,
) -> Result<i32, TaskaiError> {
    let conn = connection::open_db()?;
    let plan_id = resolve_plan_id(&conn, plan_flag)?;

    match cmd {
        crate::cli::commands::DepCommands::Add { id, dep_id } => {
            let task = task_repo::resolve_task(&conn, &plan_id, &id)?;
            let dep_task = task_repo::resolve_task(&conn, &plan_id, &dep_id)?;

            // Same plan check
            if task.plan_id != dep_task.plan_id {
                return Err(TaskaiError::cross_plan_dependency());
            }

            // Cycle check
            let all_tasks = task_repo::list_tasks_by_plan(&conn, &plan_id)?;
            let nodes: Vec<String> = all_tasks.iter().map(|t| t.id.clone()).collect();
            let existing_deps = dependency_repo::get_all_dependencies_for_plan(&conn, &plan_id)?;
            let edges: Vec<(String, String)> = existing_deps.iter().map(|d| (d.task_id.clone(), d.dependency_id.clone())).collect();
            cycle::would_create_cycle(&nodes, &edges, &task.id, &dep_task.id)?;

            dependency_repo::add_dependency(&conn, &task.id, &dep_task.id)?;

            // If task was ready and new dep is not done, set to blocked
            if task.status == TaskStatus::Ready && dep_task.status != TaskStatus::Done {
                task_repo::update_task_status(&conn, &task.id, &TaskStatus::Blocked, None)?;
            }

            if json_output {
                println!("{}", serde_json::to_string_pretty(&output::json::success(json!({
                    "added": { "task_id": task.id, "dependency_id": dep_task.id }
                }))).unwrap());
            } else {
                println!("Added dependency: {} depends on {}", task.id, dep_task.id);
            }
            Ok(0)
        }
        crate::cli::commands::DepCommands::Remove { id, dep_id } => {
            let task = task_repo::resolve_task(&conn, &plan_id, &id)?;
            let dep_task = task_repo::resolve_task(&conn, &plan_id, &dep_id)?;

            dependency_repo::remove_dependency(&conn, &task.id, &dep_task.id)?;

            // Check if task can be unblocked
            if task.status == TaskStatus::Blocked && dependency_repo::all_dependencies_done(&conn, &task.id)? {
                // Also check if there are no remaining deps at all, or all are done
                let remaining = dependency_repo::get_dependencies(&conn, &task.id)?;
                if remaining.is_empty() || dependency_repo::all_dependencies_done(&conn, &task.id)? {
                    task_repo::update_task_status(&conn, &task.id, &TaskStatus::Ready, None)?;
                }
            }

            if json_output {
                println!("{}", serde_json::to_string_pretty(&output::json::success(json!({
                    "removed": { "task_id": task.id, "dependency_id": dep_task.id }
                }))).unwrap());
            } else {
                println!("Removed dependency: {} no longer depends on {}", task.id, dep_task.id);
            }
            Ok(0)
        }
    }
}
