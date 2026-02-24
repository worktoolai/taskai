use serde_json::{json, Value};

use crate::db::task_repo::TaskProgress;
use crate::error::TaskaiError;
use crate::models::{Plan, Task, TaskDocument, PlanDocument};

pub fn success(data: Value) -> Value {
    json!({
        "success": true,
        "data": data
    })
}

pub fn success_with_plan_completed(data: Value, plan_completed: bool) -> Value {
    json!({
        "success": true,
        "plan_completed": plan_completed,
        "data": data
    })
}

pub fn error(err: &TaskaiError) -> Value {
    json!({
        "success": false,
        "error": {
            "code": err.code.as_str(),
            "message": err.message
        }
    })
}

pub fn progress_json(p: &TaskProgress) -> Value {
    json!({
        "total": p.total,
        "blocked": p.blocked,
        "ready": p.ready,
        "in_progress": p.in_progress,
        "done": p.done,
        "skipped": p.skipped,
        "cancelled": p.cancelled,
        "percentage": (p.percentage * 10.0).round() / 10.0
    })
}

pub fn task_summary(t: &Task) -> Value {
    let mut v = json!({
        "id": t.id,
        "title": t.title,
        "status": t.status.as_str(),
        "priority": t.priority
    });
    if let Some(ref agent) = t.agent {
        v["agent"] = json!(agent);
    }
    v
}

pub fn task_detail(t: &Task, has_documents: bool) -> Value {
    let mut v = json!({
        "id": t.id,
        "title": t.title,
        "description": t.description,
        "status": t.status.as_str(),
        "priority": t.priority,
        "has_documents": has_documents
    });
    if let Some(ref agent) = t.agent {
        v["agent"] = json!(agent);
    }
    if let Some(ref assigned) = t.assigned_to {
        v["assigned_to"] = json!(assigned);
    }
    v
}

pub fn in_progress_entry(t: &Task, elapsed_minutes: i64) -> Value {
    json!({
        "id": t.id,
        "title": t.title,
        "assigned_to": t.assigned_to,
        "started_at": t.started_at,
        "elapsed_minutes": elapsed_minutes
    })
}

pub fn plan_json(p: &Plan) -> Value {
    json!({
        "id": p.id,
        "name": p.name,
        "title": p.title,
        "description": p.description,
        "status": p.status.as_str(),
        "created_at": p.created_at,
        "updated_at": p.updated_at
    })
}

pub fn plan_document_json(d: &PlanDocument) -> Value {
    json!({
        "id": d.id,
        "title": d.title,
        "content": d.content
    })
}

pub fn task_document_json(d: &TaskDocument) -> Value {
    json!({
        "id": d.id,
        "title": d.title,
        "content": d.content
    })
}
