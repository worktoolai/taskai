use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanDocument {
    pub id: String,
    pub plan_id: String,
    pub title: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDocument {
    pub id: String,
    pub task_id: String,
    pub title: String,
    pub content: String,
}
