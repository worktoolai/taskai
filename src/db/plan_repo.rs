use rusqlite::{params, Connection};

use crate::error::TaskaiError;
use crate::models::{Plan, PlanStatus};

pub fn create_plan(
    conn: &Connection,
    id: &str,
    name: &str,
    title: &str,
    description: Option<&str>,
) -> Result<Plan, TaskaiError> {
    // Check name conflict
    if find_plan_by_name(conn, name)?.is_some() {
        return Err(TaskaiError::plan_name_conflict(name));
    }

    conn.execute(
        "INSERT INTO plans (id, name, title, description) VALUES (?1, ?2, ?3, ?4)",
        params![id, name, title, description],
    )?;

    get_plan_by_id(conn, id)
}

pub fn get_plan_by_id(conn: &Connection, id: &str) -> Result<Plan, TaskaiError> {
    conn.query_row(
        "SELECT id, name, title, description, status, created_at, updated_at FROM plans WHERE id = ?1",
        params![id],
        |row| {
            Ok(Plan {
                id: row.get(0)?,
                name: row.get(1)?,
                title: row.get(2)?,
                description: row.get(3)?,
                status: PlanStatus::from_str(&row.get::<_, String>(4)?).unwrap_or(PlanStatus::Active),
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        },
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => TaskaiError::plan_not_found(id),
        _ => TaskaiError::from(e),
    })
}

pub fn find_plan_by_name(conn: &Connection, name: &str) -> Result<Option<Plan>, TaskaiError> {
    let mut stmt = conn.prepare(
        "SELECT id, name, title, description, status, created_at, updated_at FROM plans WHERE name = ?1",
    )?;
    let mut rows = stmt.query(params![name])?;
    match rows.next()? {
        Some(row) => Ok(Some(Plan {
            id: row.get(0)?,
            name: row.get(1)?,
            title: row.get(2)?,
            description: row.get(3)?,
            status: PlanStatus::from_str(&row.get::<_, String>(4)?).unwrap_or(PlanStatus::Active),
            created_at: row.get(5)?,
            updated_at: row.get(6)?,
        })),
        None => Ok(None),
    }
}

/// Resolve a plan reference: exact name → ULID prefix → name partial match.
pub fn resolve_plan(conn: &Connection, reference: &str) -> Result<Plan, TaskaiError> {
    // 1. Exact name match
    if let Some(plan) = find_plan_by_name(conn, reference)? {
        return Ok(plan);
    }

    // 2. ID prefix match
    let mut stmt = conn.prepare(
        "SELECT id, name, title, description, status, created_at, updated_at FROM plans WHERE id LIKE ?1",
    )?;
    let prefix = format!("{reference}%");
    let plans: Vec<Plan> = stmt
        .query_map(params![prefix], |row| {
            Ok(Plan {
                id: row.get(0)?,
                name: row.get(1)?,
                title: row.get(2)?,
                description: row.get(3)?,
                status: PlanStatus::from_str(&row.get::<_, String>(4)?).unwrap_or(PlanStatus::Active),
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    if plans.len() == 1 {
        return Ok(plans.into_iter().next().unwrap());
    }
    if plans.len() > 1 {
        let candidates: Vec<String> = plans.iter().map(|p| format!("{} ({})", p.name, p.id)).collect();
        return Err(TaskaiError::ambiguous_ref(reference, &candidates));
    }

    // 3. Name partial match
    let mut stmt = conn.prepare(
        "SELECT id, name, title, description, status, created_at, updated_at FROM plans WHERE name LIKE ?1",
    )?;
    let pattern = format!("%{reference}%");
    let plans: Vec<Plan> = stmt
        .query_map(params![pattern], |row| {
            Ok(Plan {
                id: row.get(0)?,
                name: row.get(1)?,
                title: row.get(2)?,
                description: row.get(3)?,
                status: PlanStatus::from_str(&row.get::<_, String>(4)?).unwrap_or(PlanStatus::Active),
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    match plans.len() {
        0 => Err(TaskaiError::plan_not_found(reference)),
        1 => Ok(plans.into_iter().next().unwrap()),
        _ => {
            let candidates: Vec<String> = plans.iter().map(|p| format!("{} ({})", p.name, p.id)).collect();
            Err(TaskaiError::ambiguous_ref(reference, &candidates))
        }
    }
}

pub fn list_plans(conn: &Connection) -> Result<Vec<Plan>, TaskaiError> {
    let mut stmt = conn.prepare(
        "SELECT id, name, title, description, status, created_at, updated_at FROM plans ORDER BY created_at DESC",
    )?;
    let plans = stmt
        .query_map([], |row| {
            Ok(Plan {
                id: row.get(0)?,
                name: row.get(1)?,
                title: row.get(2)?,
                description: row.get(3)?,
                status: PlanStatus::from_str(&row.get::<_, String>(4)?).unwrap_or(PlanStatus::Active),
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(plans)
}

pub fn delete_plan(conn: &Connection, id: &str) -> Result<(), TaskaiError> {
    let changed = conn.execute("DELETE FROM plans WHERE id = ?1", params![id])?;
    if changed == 0 {
        return Err(TaskaiError::plan_not_found(id));
    }
    Ok(())
}

pub fn update_plan_status(conn: &Connection, id: &str, status: &PlanStatus) -> Result<(), TaskaiError> {
    conn.execute(
        "UPDATE plans SET status = ?1, updated_at = datetime('now') WHERE id = ?2",
        params![status.as_str(), id],
    )?;
    Ok(())
}
