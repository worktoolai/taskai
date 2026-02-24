use rusqlite::{params, Connection};

use crate::error::TaskaiError;
use crate::models::{PlanDocument, TaskDocument};

pub fn create_plan_document(
    conn: &Connection,
    id: &str,
    plan_id: &str,
    title: &str,
    content: &str,
) -> Result<(), TaskaiError> {
    conn.execute(
        "INSERT INTO plan_documents (id, plan_id, title, content) VALUES (?1, ?2, ?3, ?4)",
        params![id, plan_id, title, content],
    )?;
    Ok(())
}

pub fn get_plan_documents(conn: &Connection, plan_id: &str) -> Result<Vec<PlanDocument>, TaskaiError> {
    let mut stmt = conn.prepare(
        "SELECT id, plan_id, title, content FROM plan_documents WHERE plan_id = ?1",
    )?;
    let docs = stmt
        .query_map(params![plan_id], |row| {
            Ok(PlanDocument {
                id: row.get(0)?,
                plan_id: row.get(1)?,
                title: row.get(2)?,
                content: row.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(docs)
}

pub fn create_task_document(
    conn: &Connection,
    id: &str,
    task_id: &str,
    title: &str,
    content: &str,
) -> Result<(), TaskaiError> {
    conn.execute(
        "INSERT INTO task_documents (id, task_id, title, content) VALUES (?1, ?2, ?3, ?4)",
        params![id, task_id, title, content],
    )?;
    Ok(())
}

pub fn get_task_documents(conn: &Connection, task_id: &str) -> Result<Vec<TaskDocument>, TaskaiError> {
    let mut stmt = conn.prepare(
        "SELECT id, task_id, title, content FROM task_documents WHERE task_id = ?1",
    )?;
    let docs = stmt
        .query_map(params![task_id], |row| {
            Ok(TaskDocument {
                id: row.get(0)?,
                task_id: row.get(1)?,
                title: row.get(2)?,
                content: row.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(docs)
}
