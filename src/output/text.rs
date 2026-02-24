use crate::db::task_repo::TaskProgress;
use crate::models::{Plan, Task, PlanDocument, TaskDocument};

pub fn print_plan(p: &Plan) {
    println!("Plan: {} ({})", p.name, p.id);
    println!("  Title: {}", p.title);
    if let Some(ref desc) = p.description {
        println!("  Description: {desc}");
    }
    println!("  Status: {}", p.status.as_str());
    println!("  Created: {}", p.created_at);
}

pub fn print_plan_list(plans: &[Plan]) {
    if plans.is_empty() {
        println!("No plans found.");
        return;
    }
    for p in plans {
        println!("  {} ({}) [{}] - {}", p.name, &p.id[..8], p.status.as_str(), p.title);
    }
}

pub fn print_task(t: &Task) {
    println!("Task: {} ({})", t.title, t.id);
    if let Some(ref desc) = t.description {
        println!("  Description: {desc}");
    }
    println!("  Status: {}", t.status.as_str());
    println!("  Priority: {}", t.priority);
    if let Some(ref assigned) = t.assigned_to {
        println!("  Assigned to: {assigned}");
    }
    if let Some(ref started) = t.started_at {
        println!("  Started: {started}");
    }
    if let Some(ref completed) = t.completed_at {
        println!("  Completed: {completed}");
    }
}

pub fn print_task_list(tasks: &[Task]) {
    if tasks.is_empty() {
        println!("No tasks found.");
        return;
    }
    for t in tasks {
        let assigned = t.assigned_to.as_deref().unwrap_or("");
        println!(
            "  [{}] {} ({}) p={} {}",
            t.status.as_str(),
            t.title,
            &t.id[..std::cmp::min(8, t.id.len())],
            t.priority,
            if assigned.is_empty() { String::new() } else { format!("@{assigned}") }
        );
    }
}

pub fn print_progress(p: &TaskProgress) {
    println!("Progress: {:.1}% ({}/{})", p.percentage, p.done, p.total);
    println!(
        "  blocked={} ready={} in_progress={} done={} skipped={} cancelled={}",
        p.blocked, p.ready, p.in_progress, p.done, p.skipped, p.cancelled
    );
}

pub fn print_task_documents(docs: &[TaskDocument]) {
    for d in docs {
        println!("\n--- Document: {} ---", d.title);
        println!("{}", d.content);
    }
}

pub fn print_plan_documents(docs: &[PlanDocument]) {
    for d in docs {
        println!("\n--- Document: {} ---", d.title);
        println!("{}", d.content);
    }
}
