use std::collections::HashMap;

use crate::error::TaskaiError;

/// Detect cycles in a dependency graph using DFS 3-color algorithm.
/// edges: Vec<(task_id, dependency_id)> meaning task_id depends on dependency_id.
/// Returns Ok(()) if no cycle, Err if cycle detected.
pub fn detect_cycle(nodes: &[String], edges: &[(String, String)]) -> Result<(), TaskaiError> {
    // Build adjacency list: dependency → dependents (reverse direction for traversal)
    // We check: can we reach task_id from dependency_id following other edges?
    // Actually, we want to check the dependency graph for cycles.
    // Edge (task_id, dep_id) means task_id → dep_id (task depends on dep).
    // A cycle means: A depends on B, B depends on C, C depends on A.
    // So we check the graph where edges go from task to its dependency.

    let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();
    for node in nodes {
        adj.entry(node.as_str()).or_default();
    }
    for (task_id, dep_id) in edges {
        adj.entry(task_id.as_str()).or_default().push(dep_id.as_str());
    }

    // DFS 3-color: 0=white, 1=gray, 2=black
    let mut color: HashMap<&str, u8> = HashMap::new();
    for node in adj.keys() {
        color.insert(node, 0);
    }

    for node in adj.keys() {
        if color[node] == 0 {
            if has_cycle_dfs(node, &adj, &mut color) {
                return Err(TaskaiError::cycle_detected());
            }
        }
    }
    Ok(())
}

fn has_cycle_dfs<'a>(
    node: &'a str,
    adj: &HashMap<&'a str, Vec<&'a str>>,
    color: &mut HashMap<&'a str, u8>,
) -> bool {
    color.insert(node, 1); // gray
    if let Some(neighbors) = adj.get(node) {
        for &neighbor in neighbors {
            match color.get(neighbor) {
                Some(1) => return true,  // back edge = cycle
                Some(0) | None => {
                    if has_cycle_dfs(neighbor, adj, color) {
                        return true;
                    }
                }
                _ => {} // black, already processed
            }
        }
    }
    color.insert(node, 2); // black
    false
}

/// Check if adding edge (task_id → dep_id) would create a cycle.
pub fn would_create_cycle(
    nodes: &[String],
    existing_edges: &[(String, String)],
    new_task_id: &str,
    new_dep_id: &str,
) -> Result<(), TaskaiError> {
    let mut edges = existing_edges.to_vec();
    edges.push((new_task_id.to_string(), new_dep_id.to_string()));
    detect_cycle(nodes, &edges)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_cycle() {
        let nodes = vec!["a".into(), "b".into(), "c".into()];
        let edges = vec![("b".into(), "a".into()), ("c".into(), "b".into())];
        assert!(detect_cycle(&nodes, &edges).is_ok());
    }

    #[test]
    fn test_cycle() {
        let nodes = vec!["a".into(), "b".into(), "c".into()];
        let edges = vec![
            ("b".into(), "a".into()),
            ("c".into(), "b".into()),
            ("a".into(), "c".into()),
        ];
        assert!(detect_cycle(&nodes, &edges).is_err());
    }

    #[test]
    fn test_self_cycle() {
        let nodes = vec!["a".into()];
        let edges = vec![("a".into(), "a".into())];
        assert!(detect_cycle(&nodes, &edges).is_err());
    }
}
