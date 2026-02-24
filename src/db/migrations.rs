use rusqlite::Connection;

use crate::error::TaskaiError;

pub fn run_migrations(conn: &Connection) -> Result<(), TaskaiError> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS plans (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            title TEXT NOT NULL,
            description TEXT,
            status TEXT NOT NULL DEFAULT 'active'
                CHECK (status IN ('active', 'completed', 'archived')),
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS tasks (
            id TEXT PRIMARY KEY,
            plan_id TEXT NOT NULL REFERENCES plans(id) ON DELETE CASCADE,
            title TEXT NOT NULL,
            description TEXT,
            status TEXT NOT NULL DEFAULT 'blocked'
                CHECK (status IN ('blocked', 'ready', 'in_progress', 'done', 'cancelled', 'skipped')),
            priority INTEGER NOT NULL DEFAULT 0,
            sort_order INTEGER NOT NULL DEFAULT 0,
            assigned_to TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            started_at TEXT,
            completed_at TEXT
        );

        CREATE TABLE IF NOT EXISTS task_dependencies (
            task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
            dependency_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
            PRIMARY KEY (task_id, dependency_id),
            CHECK (task_id != dependency_id)
        );

        CREATE TABLE IF NOT EXISTS plan_documents (
            id TEXT PRIMARY KEY,
            plan_id TEXT NOT NULL REFERENCES plans(id) ON DELETE CASCADE,
            title TEXT NOT NULL,
            content TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS task_documents (
            id TEXT PRIMARY KEY,
            task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
            title TEXT NOT NULL,
            content TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_tasks_plan_status ON tasks(plan_id, status);
        CREATE INDEX IF NOT EXISTS idx_tasks_ready ON tasks(status, priority, sort_order)
            WHERE status = 'ready';
        CREATE INDEX IF NOT EXISTS idx_deps_task ON task_dependencies(task_id);
        CREATE INDEX IF NOT EXISTS idx_deps_dep ON task_dependencies(dependency_id);
        ",
    )?;
    Ok(())
}
