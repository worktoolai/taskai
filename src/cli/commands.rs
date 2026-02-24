use clap::{Parser, Subcommand};

const VERSION: &str = env!("GIT_VERSION");

#[derive(Parser)]
#[command(
    name = "taskai",
    version = VERSION,
    about = "AI agent task orchestration CLI",
    after_help = "\
NOTE:
  Requires a git repository. DB is stored at <git-root>/.worktoolai/taskai/taskai.db
  Run `taskai init` before any other command.

EXIT CODES:
  0  Success (task returned, or plan completed)
  1  Error (DB, validation, invalid transition, etc.)
  2  Waiting (no ready tasks, but blocked/in_progress remain)

UNBLOCK RULES:
  Only `done` unblocks dependents. `cancelled`/`skipped` do NOT.
  If a predecessor is cancelled, its dependents stay blocked (manual intervention needed).

BEHAVIOR NOTES:
  `task fail` may return `blocked` (not `ready`) if deps were cancelled while in_progress.
  `task add --after <done-task>` starts as `ready` (dep already satisfied).
  `plan delete` of the active plan clears the active plan config.
  Terminal states (`done`/`cancelled`/`skipped`) are immutable."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Output as JSON
    #[arg(long, global = true)]
    pub json: bool,

    /// Specify plan by name or ID
    #[arg(long, global = true)]
    pub plan: Option<String>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize taskai in this repository
    Init,

    /// Plan management
    #[command(subcommand)]
    Plan(PlanCommands),

    /// Task management
    #[command(subcommand)]
    Task(TaskCommands),

    /// Get next ready task (highest priority, then sort order)
    #[command(after_help = "\
NOTE:
  Without --claim: read-only, returns the next ready task without changing state.
  With    --claim: atomically sets the task to in_progress (SQLite transaction).
  Use --agent with --claim to record which agent owns the task.")]
    Next {
        /// Atomically claim the task (set to in_progress)
        #[arg(long)]
        claim: bool,

        /// Agent identifier for claim
        #[arg(long)]
        agent: Option<String>,
    },

    /// Show overall status
    Status,
}

#[derive(Subcommand)]
pub enum PlanCommands {
    /// Create a new plan
    Create {
        /// Plan name (slug: lowercase alphanumeric with hyphens)
        name: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        description: Option<String>,
    },
    /// List all plans
    List,
    /// Show plan details
    Show {
        /// Plan name or ID
        reference: String,
    },
    /// Set active plan
    Activate {
        /// Plan name
        name: String,
    },
    /// Delete a plan
    Delete {
        /// Plan name or ID
        reference: String,
    },
    /// Load plan from stdin JSON
    #[command(after_help = "\
STDIN FORMAT:
  {\"name\":\"slug\", \"title\":\"...\", \"tasks\":[{\"id\":\"t1\", \"title\":\"...\", \"after\":[...]}]}

NOTE:
  Atomic: all-or-nothing. Validates cycles, duplicate IDs, unknown refs.
  Plan name must be unique. Existing name → error (no overwrite).
  Tasks without `after` start as `ready`; with `after` start as `blocked`.
  Auto-activates if no valid active plan exists (none set, or stale reference).")]
    Load,
}

#[derive(Subcommand)]
pub enum TaskCommands {
    /// Add a task to the active plan
    Add {
        /// Task title
        title: String,
        #[arg(long)]
        description: Option<String>,
        #[arg(long, default_value = "0")]
        priority: i32,
        /// Add dependency: task runs after this task ID
        #[arg(long)]
        after: Vec<String>,
    },
    /// List tasks in the active plan
    List,
    /// Show task details
    Show {
        /// Task ID or prefix
        id: String,
    },
    /// Start a task (ready → in_progress)
    Start {
        id: String,
        #[arg(long)]
        agent: Option<String>,
    },
    /// Complete a task (ready|in_progress → done)
    Done {
        id: String,
    },
    /// Fail a task (in_progress → ready, or → blocked if deps no longer met)
    Fail {
        id: String,
    },
    /// Skip a task (ready|blocked → skipped)
    Skip {
        id: String,
    },
    /// Cancel a task (→ cancelled)
    Cancel {
        id: String,
    },
    /// Manage task dependencies
    #[command(subcommand)]
    Dep(DepCommands),
}

#[derive(Subcommand)]
pub enum DepCommands {
    /// Add a dependency
    Add {
        /// Task ID
        id: String,
        /// Dependency task ID
        dep_id: String,
    },
    /// Remove a dependency
    Remove {
        /// Task ID
        id: String,
        /// Dependency task ID
        dep_id: String,
    },
}
