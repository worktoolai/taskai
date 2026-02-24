use rusqlite::{params, Connection};

use crate::error::TaskaiError;
use crate::models::{Task, TaskStatus};

pub fn create_task(
    conn: &Connection,
    id: &str,
    plan_id: &str,
    title: &str,
    description: Option<&str>,
    priority: i32,
    sort_order: i32,
    status: &TaskStatus,
    agent: Option<&str>,
) -> Result<Task, TaskaiError> {
    conn.execute(
        "INSERT INTO tasks (id, plan_id, title, description, priority, sort_order, status, agent)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![id, plan_id, title, description, priority, sort_order, status.as_str(), agent],
    )?;
    get_task_by_id(conn, id)
}

pub fn get_task_by_id(conn: &Connection, id: &str) -> Result<Task, TaskaiError> {
    conn.query_row(
        "SELECT id, plan_id, title, description, status, priority, sort_order,
                agent, assigned_to, created_at, updated_at, started_at, completed_at
         FROM tasks WHERE id = ?1",
        params![id],
        row_to_task,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => TaskaiError::task_not_found(id),
        _ => TaskaiError::from(e),
    })
}

/// Resolve task by ID prefix within a plan.
pub fn resolve_task(conn: &Connection, plan_id: &str, reference: &str) -> Result<Task, TaskaiError> {
    // Exact ID match first
    if let Ok(task) = get_task_by_id(conn, reference) {
        if task.plan_id == plan_id {
            return Ok(task);
        }
    }

    // ID prefix match within plan
    let mut stmt = conn.prepare(
        "SELECT id, plan_id, title, description, status, priority, sort_order,
                agent, assigned_to, created_at, updated_at, started_at, completed_at
         FROM tasks WHERE plan_id = ?1 AND id LIKE ?2",
    )?;
    let prefix = format!("{reference}%");
    let tasks: Vec<Task> = stmt
        .query_map(params![plan_id, prefix], row_to_task)?
        .collect::<Result<Vec<_>, _>>()?;

    match tasks.len() {
        0 => Err(TaskaiError::task_not_found(reference)),
        1 => Ok(tasks.into_iter().next().unwrap()),
        _ => {
            let candidates: Vec<String> = tasks.iter().map(|t| format!("{} ({})", t.title, t.id)).collect();
            Err(TaskaiError::ambiguous_ref(reference, &candidates))
        }
    }
}

pub fn list_tasks_by_plan(conn: &Connection, plan_id: &str) -> Result<Vec<Task>, TaskaiError> {
    let mut stmt = conn.prepare(
        "SELECT id, plan_id, title, description, status, priority, sort_order,
                agent, assigned_to, created_at, updated_at, started_at, completed_at
         FROM tasks WHERE plan_id = ?1 ORDER BY sort_order ASC",
    )?;
    let tasks = stmt
        .query_map(params![plan_id], row_to_task)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(tasks)
}

pub fn update_task_status(
    conn: &Connection,
    id: &str,
    status: &TaskStatus,
    assigned_to: Option<&str>,
) -> Result<(), TaskaiError> {
    let (started_clause, completed_clause) = match status {
        TaskStatus::InProgress => ("started_at = datetime('now'),", ""),
        TaskStatus::Done => ("", "completed_at = datetime('now'),"),
        _ => ("", ""),
    };

    let sql = format!(
        "UPDATE tasks SET status = ?1, {started_clause} {completed_clause}
         assigned_to = COALESCE(?2, assigned_to),
         updated_at = datetime('now')
         WHERE id = ?3"
    );
    conn.execute(&sql, params![status.as_str(), assigned_to, id])?;
    Ok(())
}

/// Get the next ready task for a plan (highest priority, lowest sort_order).
pub fn next_ready_task(conn: &Connection, plan_id: &str) -> Result<Option<Task>, TaskaiError> {
    let mut stmt = conn.prepare(
        "SELECT id, plan_id, title, description, status, priority, sort_order,
                agent, assigned_to, created_at, updated_at, started_at, completed_at
         FROM tasks
         WHERE plan_id = ?1 AND status = 'ready'
         ORDER BY priority DESC, sort_order ASC
         LIMIT 1",
    )?;
    let mut rows = stmt.query(params![plan_id])?;
    match rows.next()? {
        Some(row) => Ok(Some(row_to_task(row)?)),
        None => Ok(None),
    }
}

/// Get all in_progress tasks for a plan.
pub fn in_progress_tasks(conn: &Connection, plan_id: &str) -> Result<Vec<Task>, TaskaiError> {
    let mut stmt = conn.prepare(
        "SELECT id, plan_id, title, description, status, priority, sort_order,
                agent, assigned_to, created_at, updated_at, started_at, completed_at
         FROM tasks
         WHERE plan_id = ?1 AND status = 'in_progress'
         ORDER BY started_at ASC",
    )?;
    let tasks = stmt
        .query_map(params![plan_id], row_to_task)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(tasks)
}

/// Get task status counts for a plan.
pub fn task_progress(conn: &Connection, plan_id: &str) -> Result<TaskProgress, TaskaiError> {
    let mut stmt = conn.prepare(
        "SELECT status, COUNT(*) FROM tasks WHERE plan_id = ?1 GROUP BY status",
    )?;
    let mut progress = TaskProgress::default();
    let rows = stmt.query_map(params![plan_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    })?;
    for row in rows {
        let (status, count) = row?;
        match status.as_str() {
            "blocked" => progress.blocked = count,
            "ready" => progress.ready = count,
            "in_progress" => progress.in_progress = count,
            "done" => progress.done = count,
            "cancelled" => progress.cancelled = count,
            "skipped" => progress.skipped = count,
            _ => {}
        }
    }
    progress.total = progress.blocked + progress.ready + progress.in_progress
        + progress.done + progress.cancelled + progress.skipped;
    progress.percentage = if progress.total > 0 {
        (progress.done as f64 / progress.total as f64) * 100.0
    } else {
        0.0
    };
    Ok(progress)
}

/// Check if task has any documents.
pub fn task_has_documents(conn: &Connection, task_id: &str) -> Result<bool, TaskaiError> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM task_documents WHERE task_id = ?1",
        params![task_id],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

#[derive(Debug, Default, Clone, serde::Serialize)]
pub struct TaskProgress {
    pub total: i64,
    pub blocked: i64,
    pub ready: i64,
    pub in_progress: i64,
    pub done: i64,
    pub skipped: i64,
    pub cancelled: i64,
    pub percentage: f64,
}

fn row_to_task(row: &rusqlite::Row) -> rusqlite::Result<Task> {
    Ok(Task {
        id: row.get(0)?,
        plan_id: row.get(1)?,
        title: row.get(2)?,
        description: row.get(3)?,
        status: TaskStatus::from_str(&row.get::<_, String>(4)?).unwrap_or(TaskStatus::Blocked),
        priority: row.get(5)?,
        sort_order: row.get(6)?,
        agent: row.get(7)?,
        assigned_to: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
        started_at: row.get(11)?,
        completed_at: row.get(12)?,
    })
}
