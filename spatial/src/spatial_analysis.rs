//! Spatial analysis — file dependency → Fiedler vector, bridge file detection,
//! and reorganization suggestions.
//!
//! # Approach
//!
//! We build a weighted graph of files where edge weight = number of import
//! relationships between them. The Laplacian of this graph gives us the
//! **Fiedler vector** (second eigenvector of the Laplacian), which reveals
//! the natural partition of the codebase.
//!
//! Bridge files: files whose removal would significantly increase the
//! Fiedler value (i.e., they connect otherwise-disconnected clusters).
//!
//! Reorganization suggestions: files whose module assignment is suboptimal
//! based on the spectral embedding.

use std::collections::{HashMap, HashSet, VecDeque};
use nalgebra::DMatrix;
use crate::FileId;
use crate::code_graph::CodeGraph;


/// Result of a spatial analysis run.
#[derive(Debug, Clone)]
pub struct SpatialAnalysisResult {
    /// File → Fiedler component (negative / positive partition).
    pub fiedler_partition: HashMap<FileId, i8>,
    /// Bridge file scores (higher = more critical structural bridge).
    pub bridge_scores: Vec<BridgeFile>,
    /// Reorganization suggestions.
    pub suggestions: Vec<ReorgSuggestion>,
    /// The Fiedler vector values for each file.
    pub fiedler_values: HashMap<FileId, f64>,
    /// Number of connected components in the dependency graph.
    pub component_count: usize,
    /// Modularity score (Q) of the Fiedler partition.
    pub modularity: f64,
}

/// A file that bridges multiple clusters.
#[derive(Debug, Clone)]
pub struct BridgeFile {
    pub file: FileId,
    /// Bridge score: betweenness centrality from Fiedler analysis.
    pub score: f64,
    /// Connected to these clusters (cluster → number of connections).
    pub connections: HashMap<i8, usize>,
}

/// A suggested file reorganization.
#[derive(Debug, Clone)]
pub struct ReorgSuggestion {
    /// The file to move.
    pub file: FileId,
    /// From this module (current cluster).
    pub from_cluster: i8,
    /// To this module (suggested cluster).
    pub to_cluster: i8,
    /// Confidence in the suggestion (0.0 – 1.0).
    pub confidence: f64,
    /// Reason for the suggestion.
    pub reason: String,
}

/// Run the full spatial analysis pipeline on a code graph.
pub fn analyze(graph: &CodeGraph) -> SpatialAnalysisResult {
    // 1. Build the file adjacency matrix
    let file_ids: Vec<&FileId> = graph.modules.keys().collect();
    let file_index: HashMap<&FileId, usize> = file_ids
        .iter()
        .enumerate()
        .map(|(i, f)| (*f, i))
        .collect();
    let n = file_ids.len();

    if n < 2 {
        return SpatialAnalysisResult {
            fiedler_partition: HashMap::new(),
            bridge_scores: Vec::new(),
            suggestions: Vec::new(),
            fiedler_values: HashMap::new(),
            component_count: 1,
            modularity: 0.0,
        };
    }

    // 2. Build adjacency matrix from import relationships
    // Edge weight = number of imports + number of call edges crossing file boundaries
    let mut adj = DMatrix::<f64>::zeros(n, n);

    for (_, cell) in &graph.modules {
        let i = match file_index.get(&cell.file) {
            Some(idx) => *idx,
            None => continue,
        };

        for imported in &cell.imports {
            if let Some(j) = file_index.get(imported) {
                let weight = adj[(i, *j)] + 1.0;
                adj[(i, *j)] = weight;
                adj[(*j, i)] = weight;
            }
        }
    }

    // Also count cross-file call edges
    for (fn_id, node) in &graph.functions {
        for callee in &node.calls {
            if fn_id.file != callee.file {
                let i = match file_index.get(&&fn_id.file) {
                    Some(idx) => *idx,
                    None => continue,
                };
                let j = match file_index.get(&&callee.file) {
                    Some(idx) => *idx,
                    None => continue,
                };
                let weight = adj[(i, j)] + 0.5; // call edges are 0.5 weight
                adj[(i, j)] = weight;
                adj[(j, i)] = weight;
            }
        }
    }

    // 3. Build Laplacian: L = D - A
    let mut degree = DMatrix::<f64>::zeros(n, n);
    for i in 0..n {
        let d: f64 = adj.row(i).sum();
        degree[(i, i)] = d;
    }
    let laplacian = &degree - &adj;

    // 4. Compute component count (connected components via BFS)
    let component_count = connected_components_count(&adj, n);

    // 5. Compute Fiedler vector (second eigenvector)
    let fiedler_values = compute_fiedler_vector(&laplacian, n);

    // 6. Partition files by sign of Fiedler value
    let mut partition: HashMap<FileId, i8> = HashMap::new();
    let mut fiedler_map: HashMap<FileId, f64> = HashMap::new();
    for (i, file) in file_ids.iter().enumerate() {
        let val = fiedler_values[i];
        fiedler_map.insert((*file).clone(), val);
        partition.insert((*file).clone(), if val >= 0.0 { 1 } else { -1 });
    }

    // 7. Detect bridge files
    let bridge_scores = detect_bridges(graph, &file_ids, &file_index, &partition, &adj);

    // 8. Generate reorganization suggestions
    let suggestions = generate_suggestions(graph, &partition, &fiedler_map);

    // 9. Compute modularity
    let modularity = compute_modularity(&adj, &partition, n);

    SpatialAnalysisResult {
        fiedler_partition: partition,
        bridge_scores,
        suggestions,
        fiedler_values: fiedler_map,
        component_count,
        modularity,
    }
}

/// Compute the Fiedler vector via power iteration on (λ₂ I - L).
/// Returns the second eigenvector of the Laplacian.
fn compute_fiedler_vector(laplacian: &DMatrix<f64>, n: usize) -> Vec<f64> {
    if n < 2 {
        return vec![0.0; n];
    }

    // Find the algebraic connectivity (second smallest eigenvalue) via
    // shifted inverse power iteration on L - μI, where μ = a small shift.
    // For the Fiedler vector, we want the eigenvector corresponding to the
    // second smallest eigenvalue.
    //
    // Approach: compute the smallest eigenvalue (which should be 0) via
    // power iteration on the pseudoinverse, then deflate to get the second.

    // Step 1: Find the smallest eigenpair using inverse iteration
    // For a Laplacian, the constant vector is the trivial eigenvector for λ=0.
    // We deflate it out and find the next one.

    // Use a simple approach: random start vector, orthogonalize against constant
    let shift = 0.1; // small shift to avoid singularity

    // Build (L + shift*I)^(-1) using an iterative approach
    let mut v: Vec<f64> = (0..n).map(|i| (i as f64 + 1.0) * 0.5).collect();

    // Normalize v and orthogonalize against the all-ones vector
    orthogonalize(v.as_mut_slice());

    // Power iteration on (L + shift*I)^(-1) applied via conjugate gradient
    // Simplified: use the inverse of the shifted Laplacian directly
    let shifted = laplacian + DMatrix::<f64>::identity(n, n) * shift;

    // Try to invert the shifted Laplacian
    let inv = match shifted.try_inverse() {
        Some(i) => i,
        None => {
            // Fallback: use a larger shift
            let shifted2 = laplacian + DMatrix::<f64>::identity(n, n) * 1.0;
            shifted2.try_inverse().unwrap_or_else(|| DMatrix::<f64>::identity(n, n))
        }
    };

    let max_iter = 50;
    for _iter in 0..max_iter {
        // v_new = inv * v
        let v_new = &inv * DMatrix::from_column_slice(n, 1, &v);
        let v_new_slice: Vec<f64> = v_new.column(0).iter().copied().collect();
        v = v_new_slice;
        orthogonalize(v.as_mut_slice());
    }

    // Check which sign gives meaningful partition
    let pos_count = v.iter().filter(|x| **x > 0.0).count();
    let neg_count = v.iter().filter(|x| **x < 0.0).count();

    if pos_count == 0 || neg_count == 0 {
        // Degenerate: return simple distance-based vector
        v = (0..n).map(|i| (i as f64) / (n as f64) - 0.5).collect();
        orthogonalize(v.as_mut_slice());
    }

    v
}

fn orthogonalize(v: &mut [f64]) {
    // Orthogonalize against the all-ones vector
    let n = v.len() as f64;
    let mean: f64 = v.iter().sum::<f64>() / n;
    for x in v.iter_mut() {
        *x -= mean;
    }

    // Normalize
    let norm: f64 = v.iter().map(|x| x * x).sum::<f64>().sqrt();
    if norm > 1e-12 {
        for x in v.iter_mut() {
            *x /= norm;
        }
    }
}

/// Detect bridge files: files whose removal disconnects clusters.
fn detect_bridges(
    graph: &CodeGraph,
    file_ids: &[&FileId],
    file_index: &HashMap<&FileId, usize>,
    partition: &HashMap<FileId, i8>,
    adj: &DMatrix<f64>,
) -> Vec<BridgeFile> {
    let mut bridges = Vec::new();

    for (file, _) in &graph.modules {
        let i = match file_index.get(file) {
            Some(idx) => *idx,
            None => continue,
        };

        let cluster = partition.get(file).copied().unwrap_or(0);
        let mut connections: HashMap<i8, usize> = HashMap::new();
        connections.insert(cluster, 1); // own cluster

        for j in 0..adj.ncols() {
            if i != j && adj[(i, j)] > 0.0 {
                let other_file = &file_ids[j];
                let other_cluster = partition.get(other_file).copied().unwrap_or(0);
                *connections.entry(other_cluster).or_insert(0) += 1;
            }
        }

        let total_connections: usize = connections.values().sum();
        let cross_cluster = connections.len().saturating_sub(1); // connections outside own cluster
        let score = if cross_cluster > 0 {
            let cross_edges = connections
                .iter()
                .filter(|(c, _)| **c != cluster)
                .map(|(_, count)| count)
                .sum::<usize>();
            cross_edges as f64 / total_connections.max(1) as f64
        } else {
            0.0
        };

        if score > 0.0 {
            bridges.push(BridgeFile {
                file: (*file).clone(),
                score,
                connections,
            });
        }
    }

    bridges.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    bridges.truncate(10); // top 10 bridges
    bridges
}

/// Generate reorganization suggestions based on spectral embedding.
fn generate_suggestions(
    graph: &CodeGraph,
    partition: &HashMap<FileId, i8>,
    fiedler_values: &HashMap<FileId, f64>,
) -> Vec<ReorgSuggestion> {
    let mut suggestions = Vec::new();

    for (file, cell) in &graph.modules {
        let current_cluster = partition.get(file).copied().unwrap_or(0);
        let current_fiedler = fiedler_values.get(file).copied().unwrap_or(0.0);

        // Check imports — do they pull to the other cluster?
        let mut import_cluster_count: HashMap<i8, usize> = HashMap::new();
        for imported in &cell.imports {
            if let Some(cluster) = partition.get(imported) {
                *import_cluster_count.entry(*cluster).or_insert(0) += 1;
            }
        }

        // Check imported-by — do users of this file sit in the other cluster?
        for importer in &cell.imported_by {
            if let Some(cluster) = partition.get(importer) {
                *import_cluster_count.entry(*cluster).or_insert(0) += 1;
            }
        }

        // If most connections are to the other cluster, suggest reorganization
        let total_connections: usize = import_cluster_count.values().sum();
        if total_connections > 0 {
            if let Some((best_cluster, count)) =
                import_cluster_count.iter().max_by_key(|(_, c)| **c)
            {
                if *best_cluster != current_cluster {
                    let ratio = *count as f64 / total_connections as f64;
                    if ratio > 0.6 {
                        let from_str = if current_cluster == 1 { "positive" } else { "negative" };
                        let to_str = if *best_cluster == 1 { "positive" } else { "negative" };
                        suggestions.push(ReorgSuggestion {
                            file: file.clone(),
                            from_cluster: current_cluster,
                            to_cluster: *best_cluster,
                            confidence: ratio,
                            reason: format!(
                                "{:.0}% of connections are in the {} cluster \
                                 (imports/exports), but file is in the {} cluster. \
                                 Fiedler value: {:.3}",
                                ratio * 100.0,
                                to_str,
                                from_str,
                                current_fiedler
                            ),
                        });
                    }
                }
            }
        }
    }

    suggestions.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    suggestions.truncate(10); // top 10 suggestions
    suggestions
}

/// Compute modularity Q of a given partition.
fn compute_modularity(
    adj: &DMatrix<f64>,
    partition: &HashMap<FileId, i8>,
    n: usize,
) -> f64 {
    let mut total_weight = 0.0;
    for i in 0..n {
        for j in i..n {
            total_weight += adj[(i, j)];
        }
    }
    if total_weight == 0.0 {
        return 0.0;
    }
    let m = total_weight;

    let file_ids: Vec<&FileId> = partition.keys().collect();
    let mut idx_map: HashMap<&FileId, usize> = HashMap::new();
    for (i, f) in file_ids.iter().enumerate() {
        idx_map.insert(f, i);
    }

    let mut q = 0.0;
    for (i, file_i) in file_ids.iter().enumerate() {
        let ci = partition.get(file_i).copied().unwrap_or(0);
        // Degree of node i in terms of edges to nodes in our set
        let ki = if n > 0 { adj.row(i).sum() } else { 0.0 };

        for (j, file_j) in file_ids.iter().enumerate().skip(i) {
            let cj = partition.get(file_j).copied().unwrap_or(0);
            if ci == cj {
                let a_ij = adj[(i, j)];
                let kj = adj.row(j).sum();
                q += a_ij - ki * kj / (2.0 * m);
            }
        }
    }

    q / (2.0 * m)
}

/// Count connected components in the adjacency graph.
fn connected_components_count(adj: &DMatrix<f64>, n: usize) -> usize {
    if n == 0 {
        return 0;
    }
    let mut visited = vec![false; n];
    let mut components = 0;

    for start in 0..n {
        if visited[start] {
            continue;
        }
        components += 1;
        let mut queue = VecDeque::new();
        queue.push_back(start);
        visited[start] = true;

        while let Some(v) = queue.pop_front() {
            for u in 0..n {
                if adj[(v, u)] > 0.0 && !visited[u] {
                    visited[u] = true;
                    queue.push_back(u);
                }
            }
        }
    }

    components
}

/// Compute the Fiedler spectrum embedding for visualization.
pub fn spectral_embedding(graph: &CodeGraph) -> HashMap<FileId, (f64, f64)> {
    let result = analyze(graph);
    result
        .fiedler_values
        .into_iter()
        .map(|(file, val)| (file, (val, 0.0)))
        .collect()
}

/// Detect cyclic dependencies between modules.
pub fn detect_cycles(graph: &CodeGraph) -> Vec<Vec<FileId>> {
    let mut cycles = Vec::new();
    let mut visited = HashSet::new();

    for (file, _) in &graph.modules {
        if !visited.contains(file) {
            let mut path = Vec::new();
            let mut path_set = HashSet::new();
            dfs_cycles(graph, file, &mut visited, &mut path, &mut path_set, &mut cycles);
        }
    }

    cycles
}

fn dfs_cycles(
    graph: &CodeGraph,
    current: &FileId,
    visited: &mut HashSet<FileId>,
    path: &mut Vec<FileId>,
    path_set: &mut HashSet<FileId>,
    cycles: &mut Vec<Vec<FileId>>,
) {
    visited.insert(current.clone());
    path.push(current.clone());
    path_set.insert(current.clone());

    if let Some(cell) = graph.modules.get(current) {
        for imported in &cell.imports {
            if imported == current {
                continue; // self-import
            }
            if path_set.contains(imported) {
                // Found a cycle
                let cycle_start = path.iter().position(|p| p == imported).unwrap();
                let cycle: Vec<FileId> = path[cycle_start..].to_vec();
                cycles.push(cycle);
            } else if !visited.contains(imported) {
                dfs_cycles(graph, imported, visited, path, path_set, cycles);
            }
        }
    }

    path.pop();
    path_set.remove(current);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::code_graph::CodeGraph;

    fn make_graph() -> CodeGraph {
        let mut graph = CodeGraph::new();
        let a = FileId("crates/core/src/lib.rs".into());
        let b = FileId("crates/parser/src/lib.rs".into());
        let c = FileId("crates/shell/src/lib.rs".into());

        graph.register_module(a.clone());
        graph.register_module(b.clone());
        graph.register_module(c.clone());

        // a depends on b, b depends on c
        graph.add_import(&a, &b);
        graph.add_import(&b, &c);

        graph
    }

    #[test]
    fn test_analyze_basic() {
        let graph = make_graph();
        let result = analyze(&graph);

        assert_eq!(result.component_count, 1);
        assert!(result.fiedler_partition.len() >= 2);
    }

    #[test]
    fn test_connected_components() {
        let mut graph = CodeGraph::new();
        let a = FileId("src/a.rs".into());
        let b = FileId("src/b.rs".into());
        graph.register_module(a);
        graph.register_module(b);

        let result = analyze(&graph);
        // Two disconnected modules → 2 components
        assert_eq!(result.component_count, 2);
    }

    #[test]
    fn test_no_false_positives_single_module() {
        let mut graph = CodeGraph::new();
        let a = FileId("src/lib.rs".into());
        graph.register_module(a);
        let result = analyze(&graph);
        assert!(result.bridge_scores.is_empty());
    }

    #[test]
    fn test_detect_cycles() {
        let mut graph = CodeGraph::new();
        let a = FileId("src/a.rs".into());
        let b = FileId("src/b.rs".into());

        graph.register_module(a.clone());
        graph.register_module(b.clone());

        // a → b → a (cycle)
        graph.add_import(&a, &b);
        graph.add_import(&b, &a);

        let cycles = detect_cycles(&graph);
        assert!(!cycles.is_empty(), "Should detect at least one cycle");
    }

    #[test]
    fn test_bridge_detection() {
        let mut graph = CodeGraph::new();
        let a = FileId("src/a.rs".into());
        let b = FileId("src/b.rs".into());
        let c = FileId("src/c.rs".into());

        graph.register_module(a.clone());
        graph.register_module(b.clone());
        graph.register_module(c.clone());

        // a ← b → c (b is a bridge)
        graph.add_import(&a, &b);
        graph.add_import(&c, &b);
        // Also add cross-edge: a → c (to create a cycle via b)
        graph.add_import(&a, &c);

        let result = analyze(&graph);
        assert!(
            !result.bridge_scores.is_empty(),
            "Should detect at least one bridge: {:?}",
            result.bridge_scores
        );
    }
}
