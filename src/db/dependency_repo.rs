use rusqlite::{params, Connection};

use crate::error::TaskaiError;
use crate::models::TaskDependency;

pub fn add_dependency(conn: &Connection, task_id: &str, dependency_id: &str) -> Result<(), TaskaiError> {
    conn.execute(
        "INSERT OR IGNORE INTO task_dependencies (task_id, dependency_id) VALUES (?1, ?2)",
        params![task_id, dependency_id],
    )?;
    Ok(())
}

pub fn remove_dependency(conn: &Connection, task_id: &str, dependency_id: &str) -> Result<(), TaskaiError> {
    conn.execute(
        "DELETE FROM task_dependencies WHERE task_id = ?1 AND dependency_id = ?2",
        params![task_id, dependency_id],
    )?;
    Ok(())
}

/// Get all dependencies (predecessors) of a task.
pub fn get_dependencies(conn: &Connection, task_id: &str) -> Result<Vec<String>, TaskaiError> {
    let mut stmt = conn.prepare(
        "SELECT dependency_id FROM task_dependencies WHERE task_id = ?1",
    )?;
    let deps = stmt
        .query_map(params![task_id], |row| row.get(0))?
        .collect::<Result<Vec<String>, _>>()?;
    Ok(deps)
}

/// Get all dependents (successors) of a task — tasks that depend on this one.
pub fn get_dependents(conn: &Connection, dependency_id: &str) -> Result<Vec<String>, TaskaiError> {
    let mut stmt = conn.prepare(
        "SELECT task_id FROM task_dependencies WHERE dependency_id = ?1",
    )?;
    let deps = stmt
        .query_map(params![dependency_id], |row| row.get(0))?
        .collect::<Result<Vec<String>, _>>()?;
    Ok(deps)
}

/// Get all dependency edges for a plan (used for cycle detection).
pub fn get_all_dependencies_for_plan(conn: &Connection, plan_id: &str) -> Result<Vec<TaskDependency>, TaskaiError> {
    let mut stmt = conn.prepare(
        "SELECT td.task_id, td.dependency_id
         FROM task_dependencies td
         JOIN tasks t ON td.task_id = t.id
         WHERE t.plan_id = ?1",
    )?;
    let deps = stmt
        .query_map(params![plan_id], |row| {
            Ok(TaskDependency {
                task_id: row.get(0)?,
                dependency_id: row.get(1)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(deps)
}

/// Check if all dependencies of a task are done.
pub fn all_dependencies_done(conn: &Connection, task_id: &str) -> Result<bool, TaskaiError> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM task_dependencies td
         JOIN tasks t ON td.dependency_id = t.id
         WHERE td.task_id = ?1 AND t.status != 'done'",
        params![task_id],
        |row| row.get(0),
    )?;
    Ok(count == 0)
}
