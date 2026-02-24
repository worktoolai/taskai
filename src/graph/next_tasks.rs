use rusqlite::Connection;

use crate::db::{dependency_repo, task_repo};
use crate::error::TaskaiError;
use crate::models::{Task, TaskStatus};

/// Cascade unblock: after a task is done, check its dependents and unblock if all deps are done.
/// Returns the list of newly unblocked (ready) task IDs.
pub fn cascade_unblock(conn: &Connection, completed_task_id: &str) -> Result<Vec<Task>, TaskaiError> {
    let dependents = dependency_repo::get_dependents(conn, completed_task_id)?;
    let mut newly_ready = Vec::new();

    for dependent_id in dependents {
        let task = task_repo::get_task_by_id(conn, &dependent_id)?;
        if task.status != TaskStatus::Blocked {
            continue;
        }

        if dependency_repo::all_dependencies_done(conn, &dependent_id)? {
            task_repo::update_task_status(conn, &dependent_id, &TaskStatus::Ready, None)?;
            let updated = task_repo::get_task_by_id(conn, &dependent_id)?;
            newly_ready.push(updated);
        }
    }

    Ok(newly_ready)
}

/// Claim the next ready task atomically (within an existing transaction).
pub fn claim_next_task(
    conn: &Connection,
    plan_id: &str,
    agent: Option<&str>,
) -> Result<Option<Task>, TaskaiError> {
    let task = task_repo::next_ready_task(conn, plan_id)?;
    if let Some(ref task) = task {
        task_repo::update_task_status(conn, &task.id, &TaskStatus::InProgress, agent)?;
        return Ok(Some(task_repo::get_task_by_id(conn, &task.id)?));
    }
    Ok(None)
}
