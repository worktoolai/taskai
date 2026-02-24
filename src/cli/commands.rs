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
  Terminal states (`done`/`cancelled`/`skipped`) are immutable.

AGENT FIELD:
  Tasks can have a pre-assigned `agent` field (who should execute) set at creation time.
  This is separate from `assigned_to` (who actually claimed it at runtime).
  Set via `task add --agent <name>` or `\"agent\"` key in `plan load` JSON.
  The `next` command returns the `agent` field in JSON output so orchestrators can route tasks.
  Use `next --claim --agent <name>` to set `assigned_to` when an agent picks up a task."
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
  Use --agent with --claim to record which agent owns the task (sets `assigned_to`).
  JSON output includes the task's pre-assigned `agent` field for routing decisions.")]
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
  {\"name\":\"slug\", \"title\":\"...\", \"tasks\":[{\"id\":\"t1\", \"title\":\"...\", \"agent\":\"...\", \"after\":[...]}]}

TASK FIELDS:
  id          (required) Temporary ID for dependency references
  title       (required) Task title
  description (optional) Task description
  priority    (optional) Integer, default 0. Higher = picked first by `next`
  agent       (optional) Pre-assigned agent name for task routing
  after       (optional) List of task IDs this task depends on
  documents   (optional) List of {title, content} attached to the task

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
        /// Agent to execute this task
        #[arg(long)]
        agent: Option<String>,
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
