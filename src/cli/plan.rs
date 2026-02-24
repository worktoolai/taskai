use std::collections::{HashMap, HashSet};
use std::io::{self, Read};

use rusqlite::Connection;
use serde::Deserialize;
use serde_json::json;

use crate::cli::commands::PlanCommands;
use crate::db::{connection, plan_repo, task_repo, dependency_repo, document_repo};
use crate::error::TaskaiError;
use crate::graph::cycle;
use crate::models::TaskStatus;
use crate::output;

pub fn run(cmd: PlanCommands, json_output: bool) -> i32 {
    let result = match cmd {
        PlanCommands::Create { name, title, description } => run_create(&name, title.as_deref(), description.as_deref(), json_output),
        PlanCommands::List => run_list(json_output),
        PlanCommands::Show { reference } => run_show(&reference, json_output),
        PlanCommands::Activate { name } => run_activate(&name, json_output),
        PlanCommands::Delete { reference } => run_delete(&reference, json_output),
        PlanCommands::Load => run_load(json_output),
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

fn validate_plan_name(name: &str) -> Result<(), TaskaiError> {
    let re = regex_lite(name);
    if !re {
        return Err(TaskaiError::validation(
            "Plan name must match ^[a-z0-9][a-z0-9-]*[a-z0-9]$ (or single char [a-z0-9])",
        ));
    }
    Ok(())
}

fn regex_lite(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    if name.len() == 1 {
        return name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
    }
    let chars: Vec<char> = name.chars().collect();
    let first = chars[0];
    let last = *chars.last().unwrap();
    if !(first.is_ascii_lowercase() || first.is_ascii_digit()) {
        return false;
    }
    if !(last.is_ascii_lowercase() || last.is_ascii_digit()) {
        return false;
    }
    chars.iter().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '-')
}

fn run_create(name: &str, title: Option<&str>, description: Option<&str>, json_output: bool) -> Result<i32, TaskaiError> {
    validate_plan_name(name)?;
    let conn = connection::open_db()?;
    let id = ulid::Ulid::new().to_string();
    let title = title.unwrap_or(name);
    let plan = plan_repo::create_plan(&conn, &id, name, title, description)?;

    if json_output {
        println!("{}", serde_json::to_string_pretty(&output::json::success(output::json::plan_json(&plan))).unwrap());
    } else {
        println!("Created plan: {} ({})", plan.name, plan.id);
    }
    Ok(0)
}

fn run_list(json_output: bool) -> Result<i32, TaskaiError> {
    let conn = connection::open_db()?;
    let plans = plan_repo::list_plans(&conn)?;
    let active_id = get_active_plan_id();

    if json_output {
        let plans_json: Vec<_> = plans.iter().map(|p| {
            let mut v = output::json::plan_json(p);
            if Some(&p.id) == active_id.as_ref() {
                v["active"] = json!(true);
            }
            v
        }).collect();
        println!("{}", serde_json::to_string_pretty(&output::json::success(json!({ "plans": plans_json }))).unwrap());
    } else {
        if plans.is_empty() {
            println!("No plans found.");
        } else {
            for p in &plans {
                let marker = if Some(&p.id) == active_id.as_ref() { " *" } else { "" };
                println!("  {} ({}) [{}] - {}{}", p.name, &p.id[..8], p.status.as_str(), p.title, marker);
            }
        }
    }
    Ok(0)
}

fn run_show(reference: &str, json_output: bool) -> Result<i32, TaskaiError> {
    let conn = connection::open_db()?;
    let plan = plan_repo::resolve_plan(&conn, reference)?;
    let tasks = task_repo::list_tasks_by_plan(&conn, &plan.id)?;
    let progress = task_repo::task_progress(&conn, &plan.id)?;
    let docs = document_repo::get_plan_documents(&conn, &plan.id)?;

    if json_output {
        let tasks_json: Vec<_> = tasks.iter().map(|t| output::json::task_summary(t)).collect();
        let docs_json: Vec<_> = docs.iter().map(|d| output::json::plan_document_json(d)).collect();
        println!("{}", serde_json::to_string_pretty(&output::json::success(json!({
            "plan": output::json::plan_json(&plan),
            "tasks": tasks_json,
            "documents": docs_json,
            "progress": output::json::progress_json(&progress)
        }))).unwrap());
    } else {
        output::text::print_plan(&plan);
        println!();
        output::text::print_progress(&progress);
        println!("\nTasks:");
        output::text::print_task_list(&tasks);
        if !docs.is_empty() {
            output::text::print_plan_documents(&docs);
        }
    }
    Ok(0)
}

fn run_activate(name: &str, json_output: bool) -> Result<i32, TaskaiError> {
    let conn = connection::open_db()?;
    let plan = plan_repo::resolve_plan(&conn, name)?;

    let config_path = connection::config_path()?;
    let config = json!({ "active_plan_id": plan.id });
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| TaskaiError::database(e.to_string()))?;
    }
    std::fs::write(&config_path, serde_json::to_string_pretty(&config).unwrap())
        .map_err(|e| TaskaiError::database(e.to_string()))?;

    if json_output {
        println!("{}", serde_json::to_string_pretty(&output::json::success(json!({
            "activated": { "id": plan.id, "name": plan.name }
        }))).unwrap());
    } else {
        println!("Activated plan: {} ({})", plan.name, plan.id);
    }
    Ok(0)
}

fn run_delete(reference: &str, json_output: bool) -> Result<i32, TaskaiError> {
    let conn = connection::open_db()?;
    let plan = plan_repo::resolve_plan(&conn, reference)?;
    plan_repo::delete_plan(&conn, &plan.id)?;

    // Clear active plan if we just deleted it
    if get_active_plan_id().as_deref() == Some(plan.id.as_str()) {
        if let Ok(config_path) = connection::config_path() {
            let _ = std::fs::remove_file(config_path);
        }
    }

    if json_output {
        println!("{}", serde_json::to_string_pretty(&output::json::success(json!({
            "deleted": { "id": plan.id, "name": plan.name }
        }))).unwrap());
    } else {
        println!("Deleted plan: {} ({})", plan.name, plan.id);
    }
    Ok(0)
}

// --- plan load ---

#[derive(Deserialize)]
struct PlanLoadInput {
    name: String,
    title: String,
    description: Option<String>,
    #[serde(default)]
    documents: Vec<DocInput>,
    tasks: Vec<TaskInput>,
}

#[derive(Deserialize)]
struct DocInput {
    title: String,
    content: String,
}

#[derive(Deserialize)]
struct TaskInput {
    id: String,
    title: String,
    description: Option<String>,
    #[serde(default)]
    priority: i32,
    agent: Option<String>,
    #[serde(default)]
    after: Vec<String>,
    #[serde(default)]
    documents: Vec<DocInput>,
}

fn run_load(json_output: bool) -> Result<i32, TaskaiError> {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).map_err(|e| TaskaiError::validation(e.to_string()))?;

    let plan_input: PlanLoadInput =
        serde_json::from_str(&input).map_err(|e| TaskaiError::validation(format!("Invalid JSON: {e}")))?;

    // Validate
    validate_plan_name(&plan_input.name)?;
    validate_load_input(&plan_input)?;

    let conn = connection::open_db()?;

    // Check name conflict
    if plan_repo::find_plan_by_name(&conn, &plan_input.name)?.is_some() {
        return Err(TaskaiError::plan_name_conflict(&plan_input.name));
    }

    // Create everything in a transaction
    let plan_id = ulid::Ulid::new().to_string();
    let mut id_mapping: HashMap<String, String> = HashMap::new();

    conn.execute_batch("BEGIN IMMEDIATE")?;

    // Create plan
    let result = (|| -> Result<_, TaskaiError> {
        conn.execute(
            "INSERT INTO plans (id, name, title, description) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![plan_id, plan_input.name, plan_input.title, plan_input.description],
        )?;

        // Plan documents
        for doc in &plan_input.documents {
            let doc_id = ulid::Ulid::new().to_string();
            document_repo::create_plan_document(&conn, &doc_id, &plan_id, &doc.title, &doc.content)?;
        }

        // Create tasks
        for (i, task_input) in plan_input.tasks.iter().enumerate() {
            let task_id = ulid::Ulid::new().to_string();
            id_mapping.insert(task_input.id.clone(), task_id.clone());

            let status = if task_input.after.is_empty() {
                TaskStatus::Ready
            } else {
                TaskStatus::Blocked
            };

            task_repo::create_task(
                &conn, &task_id, &plan_id, &task_input.title,
                task_input.description.as_deref(), task_input.priority,
                i as i32, &status, task_input.agent.as_deref(),
            )?;

            // Task documents
            for doc in &task_input.documents {
                let doc_id = ulid::Ulid::new().to_string();
                document_repo::create_task_document(&conn, &doc_id, &task_id, &doc.title, &doc.content)?;
            }
        }

        // Create dependencies
        for task_input in &plan_input.tasks {
            let task_id = &id_mapping[&task_input.id];
            for dep_temp_id in &task_input.after {
                let dep_id = &id_mapping[dep_temp_id];
                dependency_repo::add_dependency(&conn, task_id, dep_id)?;
            }
        }

        Ok(())
    })();

    match result {
        Ok(()) => {
            conn.execute_batch("COMMIT")?;
        }
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK");
            return Err(e);
        }
    }

    // Auto-activate if no valid active plan
    let should_activate = match get_active_plan_id() {
        None => true,
        Some(ref id) => plan_repo::get_plan_by_id(&conn, id).is_err(),
    };
    if should_activate {
        let config_path = connection::config_path()?;
        let config = json!({ "active_plan_id": plan_id });
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| TaskaiError::database(e.to_string()))?;
        }
        std::fs::write(&config_path, serde_json::to_string_pretty(&config).unwrap())
            .map_err(|e| TaskaiError::database(e.to_string()))?;
    }

    // Build response
    let tasks = task_repo::list_tasks_by_plan(&conn, &plan_id)?;
    let ready_now: Vec<_> = tasks.iter().filter(|t| t.status == TaskStatus::Ready).collect();

    if json_output {
        let id_map_json: serde_json::Map<String, serde_json::Value> = id_mapping
            .iter()
            .map(|(k, v)| (k.clone(), json!(v)))
            .collect();
        let ready_json: Vec<_> = ready_now.iter().map(|t| json!({ "id": t.id, "title": t.title })).collect();
        println!("{}", serde_json::to_string_pretty(&output::json::success(json!({
            "plan": { "name": plan_input.name, "id": plan_id },
            "tasks_created": plan_input.tasks.len(),
            "id_mapping": id_map_json,
            "ready_now": ready_json
        }))).unwrap());
    } else {
        println!("Loaded plan '{}' with {} tasks.", plan_input.name, plan_input.tasks.len());
        if !ready_now.is_empty() {
            println!("Ready now:");
            for t in &ready_now {
                println!("  {} - {}", t.id, t.title);
            }
        }
    }
    Ok(0)
}

fn validate_load_input(input: &PlanLoadInput) -> Result<(), TaskaiError> {
    if input.name.is_empty() {
        return Err(TaskaiError::validation("Plan name is required"));
    }
    if input.title.is_empty() {
        return Err(TaskaiError::validation("Plan title is required"));
    }
    if input.tasks.is_empty() {
        return Err(TaskaiError::validation("At least one task is required"));
    }

    // Check duplicate temp IDs
    let mut seen_ids = HashSet::new();
    for t in &input.tasks {
        if t.id.is_empty() {
            return Err(TaskaiError::validation("Task id is required"));
        }
        if t.title.is_empty() {
            return Err(TaskaiError::validation(format!("Task '{}' has empty title", t.id)));
        }
        if !seen_ids.insert(&t.id) {
            return Err(TaskaiError::validation(format!("Duplicate task id: {}", t.id)));
        }
    }

    // Check after references
    for t in &input.tasks {
        for dep in &t.after {
            if dep == &t.id {
                return Err(TaskaiError::validation(format!("Task '{}' depends on itself", t.id)));
            }
            if !seen_ids.contains(dep) {
                return Err(TaskaiError::validation(format!(
                    "Task '{}' references unknown dependency '{}'",
                    t.id, dep
                )));
            }
        }
    }

    // Cycle detection
    let nodes: Vec<String> = input.tasks.iter().map(|t| t.id.clone()).collect();
    let edges: Vec<(String, String)> = input
        .tasks
        .iter()
        .flat_map(|t| t.after.iter().map(move |dep| (t.id.clone(), dep.clone())))
        .collect();
    cycle::detect_cycle(&nodes, &edges)?;

    Ok(())
}

pub fn get_active_plan_id() -> Option<String> {
    let config_path = connection::config_path().ok()?;
    let content = std::fs::read_to_string(config_path).ok()?;
    let config: serde_json::Value = serde_json::from_str(&content).ok()?;
    config["active_plan_id"].as_str().map(|s| s.to_string())
}

pub fn resolve_plan_id(conn: &Connection, plan_flag: Option<&str>) -> Result<String, TaskaiError> {
    if let Some(reference) = plan_flag {
        let plan = plan_repo::resolve_plan(conn, reference)?;
        return Ok(plan.id);
    }
    let id = get_active_plan_id().ok_or_else(TaskaiError::no_active_plan)?;
    // Validate that the active plan still exists
    plan_repo::get_plan_by_id(conn, &id)?;
    Ok(id)
}
