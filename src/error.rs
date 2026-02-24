use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    NotInitialized,
    NoActivePlan,
    PlanNotFound,
    TaskNotFound,
    AmbiguousRef,
    TaskBlocked,
    CycleDetected,
    InvalidStatusTransition,
    CrossPlanDependency,
    PlanNameConflict,
    ValidationError,
    DatabaseError,
}

impl ErrorCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NotInitialized => "NOT_INITIALIZED",
            Self::NoActivePlan => "NO_ACTIVE_PLAN",
            Self::PlanNotFound => "PLAN_NOT_FOUND",
            Self::TaskNotFound => "TASK_NOT_FOUND",
            Self::AmbiguousRef => "AMBIGUOUS_REF",
            Self::TaskBlocked => "TASK_BLOCKED",
            Self::CycleDetected => "CYCLE_DETECTED",
            Self::InvalidStatusTransition => "INVALID_STATUS_TRANSITION",
            Self::CrossPlanDependency => "CROSS_PLAN_DEPENDENCY",
            Self::PlanNameConflict => "PLAN_NAME_CONFLICT",
            Self::ValidationError => "VALIDATION_ERROR",
            Self::DatabaseError => "DATABASE_ERROR",
        }
    }
}

#[derive(Debug, Error)]
#[error("{message}")]
pub struct TaskaiError {
    pub code: ErrorCode,
    pub message: String,
}

impl TaskaiError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn not_initialized() -> Self {
        Self::new(
            ErrorCode::NotInitialized,
            "taskai is not initialized. Run `taskai init` first.",
        )
    }

    pub fn no_active_plan() -> Self {
        Self::new(
            ErrorCode::NoActivePlan,
            "No active plan. Use `taskai plan activate <name>` or `--plan <name>`.",
        )
    }

    pub fn plan_not_found(reference: &str) -> Self {
        Self::new(
            ErrorCode::PlanNotFound,
            format!("Plan not found: {reference}"),
        )
    }

    pub fn task_not_found(reference: &str) -> Self {
        Self::new(
            ErrorCode::TaskNotFound,
            format!("Task not found: {reference}"),
        )
    }

    pub fn ambiguous_ref(reference: &str, candidates: &[String]) -> Self {
        Self::new(
            ErrorCode::AmbiguousRef,
            format!(
                "Ambiguous reference '{}'. Candidates: {}",
                reference,
                candidates.join(", ")
            ),
        )
    }

    pub fn task_blocked(task_id: &str) -> Self {
        Self::new(
            ErrorCode::TaskBlocked,
            format!("Task {task_id} is blocked by unfinished dependencies"),
        )
    }

    pub fn cycle_detected() -> Self {
        Self::new(ErrorCode::CycleDetected, "Dependency cycle detected")
    }

    pub fn invalid_transition(from: &str, to: &str) -> Self {
        Self::new(
            ErrorCode::InvalidStatusTransition,
            format!("Invalid status transition: {from} → {to}"),
        )
    }

    pub fn cross_plan_dependency() -> Self {
        Self::new(
            ErrorCode::CrossPlanDependency,
            "Dependencies across different plans are not allowed",
        )
    }

    pub fn plan_name_conflict(name: &str) -> Self {
        Self::new(
            ErrorCode::PlanNameConflict,
            format!("Plan with name '{name}' already exists"),
        )
    }

    pub fn validation(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::ValidationError, message)
    }

    pub fn database(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::DatabaseError, message)
    }
}

impl From<rusqlite::Error> for TaskaiError {
    fn from(e: rusqlite::Error) -> Self {
        Self::database(e.to_string())
    }
}
