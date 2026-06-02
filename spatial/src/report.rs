//! Diagnostic report generation — show don't sell.
//!
//! The output format is designed for humans: clear, direct, factual.
//! No marketing. No guesswork painted as insight. Just numbers and structure.
//!
//! Example output:
//!
//! ```text
//! ── Spatial Intelligence Report ──────────────────────────────
//!
//! Modules: 47  |  Functions: 1,203  |  Lattice radius: 12
//!
//! auth.rs and user.rs are distance 2 but call each other 23×. Same module.
//! ─────────────
//! Buffer distance: 1 hex  |  Call frequency: 4/min  |  Coupling: tight
//!
//! Bridge files (top 3):
//!   auth.rs          score: 0.82  — connects 3 clusters
//!   db.rs            score: 0.71  — connects 2 clusters
//!   middleware.rs    score: 0.55  — connects 2 clusters
//!
//! Reorg suggestions (top 2):
//!   validator.rs → negative cluster (88% of connections there)
//!   analytics.rs → positive cluster (72% of connections there)
//!
//! Module cycles:
//!   core → parser → core
//!   auth → session → db → auth
//!
//! Spectral stats:
//!   Algebraic connectivity: 0.14  |  Modularity: 0.67
//!   Natural clusters found: 2
//! ─────────────────────────────────────────────────────────────
//! ```

use std::collections::HashSet;
use crate::FunctionId;
use crate::code_graph::CodeGraph;
use crate::spatial_analysis::{SpatialAnalysisResult, analyze, detect_cycles};

/// Formatting style for reports.
#[derive(Debug, Clone, Copy)]
pub enum ReportStyle {
    /// Colorful ASCII art (default).
    Normal,
    /// Machine-readable (JSON).
    Json,
}

/// Generate a full spatial intelligence report.
pub fn generate_report(
    graph: &CodeGraph,
    style: ReportStyle,
) -> String {
    let analysis = analyze(graph);

    match style {
        ReportStyle::Json => generate_json_report(graph, &analysis),
        ReportStyle::Normal => generate_text_report(graph, &analysis),
    }
}

fn generate_text_report(
    graph: &CodeGraph,
    analysis: &SpatialAnalysisResult,
) -> String {
    use std::fmt::Write;
    let mut out = String::new();

    // Header
    writeln!(out, "── Spatial Intelligence Report ──────────────────────────────").ok();
    writeln!(out).ok();

    // Overview
    let module_count = graph.modules.len();
    let function_count = graph.functions.len();
    let max_lattice_radius = graph
        .functions
        .values()
        .map(|n| n.pos.hex_distance())
        .max()
        .unwrap_or(0);

    writeln!(
        out,
        "Modules: {}  |  Functions: {}  |  Lattice radius: {}",
        module_count, function_count, max_lattice_radius
    )
    .ok();
    writeln!(out).ok();

    // Close-pair analysis: find function pairs with low lattice distance but high call frequency
    if function_count > 0 {
        let pairs = find_notable_pairs(graph, 10);
        for pair in &pairs {
            writeln!(
                out,
                "{} and {} are distance {} but call each other {}×.",
                pair.0.file.0, pair.1.file.0,
                pair.2.lattice_distance, pair.2.call_count
            )
            .ok();
            if pair.0.file == pair.1.file {
                writeln!(out, "Same module.").ok();
            } else {
                writeln!(
                    out,
                    "Different modules ({} ←→ {}). Consider merging.",
                    pair.0.file.0, pair.1.file.0
                )
                .ok();
            }
            writeln!(
                out,
                "─────────────\nLattice distance: {} hex  |  Call frequency: {}",
                pair.2.lattice_distance, pair.2.call_count
            )
            .ok();
            writeln!(
                out,
                "Coupling: {}",
                if pair.2.lattice_distance <= 1 {
                    "tight"
                } else if pair.2.lattice_distance <= 3 {
                    "moderate"
                } else {
                    "loose"
                }
            )
            .ok();
            writeln!(out).ok();
        }
    }

    // Bridge files
    writeln!(out, "Bridge files (top {}):", analysis.bridge_scores.len().min(5)).ok();
    for bridge in &analysis.bridge_scores[..analysis.bridge_scores.len().min(5)] {
        let cluster_info: Vec<String> = bridge
            .connections
            .iter()
            .map(|(c, n)| format!("cluster {} ({} edges)", c, n))
            .collect();
        writeln!(
            out,
            "  {:30} score: {:.2}  — connects {}",
            bridge.file.0,
            bridge.score,
            cluster_info.join(", ")
        )
        .ok();
    }
    writeln!(out).ok();

    // Reorg suggestions
    writeln!(out, "Reorg suggestions (top {}):", analysis.suggestions.len().min(3)).ok();
    for suggestion in &analysis.suggestions[..analysis.suggestions.len().min(3)] {
        let to_label = if suggestion.to_cluster == 1 {
            "positive"
        } else {
            "negative"
        };
        writeln!(
            out,
            "  {:30} → {} cluster ({:.0}% confidence)",
            suggestion.file.0,
            to_label,
            suggestion.confidence * 100.0
        )
        .ok();
    }
    writeln!(out).ok();

    // Module cycles
    let cycles = detect_cycles(graph);
    if !cycles.is_empty() {
        writeln!(out, "Module cycles:").ok();
        for cycle in cycles.iter().take(3) {
            let path: Vec<&str> = cycle.iter().map(|f| f.0.as_str()).collect();
            let joined = path.join(" → ") + " → " + &path[0];
            writeln!(out, "  {}", joined).ok();
        }
        writeln!(out).ok();
    }

    // Spectral stats
    writeln!(out, "Spectral stats:").ok();
    writeln!(
        out,
        "  Algebraic connectivity: {:.3}  |  Modularity: {:.3}",
        compute_algebraic_connectivity_from_analysis(analysis),
        analysis.modularity
    )
    .ok();
    writeln!(
        out,
        "  Natural clusters found: {}",
        analysis.component_count
    )
    .ok();

    writeln!(out).ok();
    writeln!(
        out,
        "─────────────────────────────────────────────────────────────"
    )
    .ok();

    out
}

fn generate_json_report(graph: &CodeGraph, analysis: &SpatialAnalysisResult) -> String {
    use serde_json::json;

    let pairs: Vec<serde_json::Value> = find_notable_pairs(graph, 20)
        .iter()
        .map(|(a, b, info)| {
            json!({
                "fn_a": a.name,
                "file_a": a.file.0,
                "fn_b": b.name,
                "file_b": b.file.0,
                "lattice_distance": info.lattice_distance,
                "call_count": info.call_count,
                "same_module": a.file == b.file
            })
        })
        .collect();

    let bridges: Vec<serde_json::Value> = analysis
        .bridge_scores
        .iter()
        .map(|b| {
            json!({
                "file": b.file.0,
                "score": b.score,
                "connections": b.connections
            })
        })
        .collect();

    let suggestions: Vec<serde_json::Value> = analysis
        .suggestions
        .iter()
        .map(|s| {
            json!({
                "file": s.file.0,
                "from_cluster": s.from_cluster,
                "to_cluster": s.to_cluster,
                "confidence": s.confidence,
                "reason": s.reason
            })
        })
        .collect();

    let cycles: Vec<Vec<String>> = detect_cycles(graph)
        .iter()
        .map(|cycle| cycle.iter().map(|f| f.0.clone()).collect())
        .collect();

    let report = json!({
        "summary": {
            "modules": graph.modules.len(),
            "functions": graph.functions.len(),
            "components": analysis.component_count,
            "modularity": analysis.modularity,
            "algebraic_connectivity": compute_algebraic_connectivity_from_analysis(analysis)
        },
        "notable_pairs": pairs,
        "bridges": bridges,
        "reorg_suggestions": suggestions,
        "module_cycles": cycles
    });

    serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".to_string())
}

/// Information about a notable function pair.
#[derive(Debug, Clone)]
struct PairInfo {
    lattice_distance: u32,
    call_count: usize,
}

/// Find pairs of functions with notable lattice distance / call frequency ratio.
fn find_notable_pairs(
    graph: &CodeGraph,
    max_pairs: usize,
) -> Vec<(FunctionId, FunctionId, PairInfo)> {
    let mut pairs = Vec::new();
    let mut seen = HashSet::new();

    for (id_a, node_a) in &graph.functions {
        for (id_b, node_b) in &graph.functions {
            if id_a == id_b { continue; }
            let pair_key = if id_a.name < id_b.name || (id_a.name == id_b.name && id_a.file.0 < id_b.file.0) {
                (id_a.name.clone(), id_b.name.clone())
            } else {
                (id_b.name.clone(), id_a.name.clone())
            };
            if !seen.insert(pair_key) { continue; }

            let lattice_distance = (node_a.pos - node_b.pos).hex_distance();
            if lattice_distance > 10 {
                continue; // too far apart, skip
            }

            // Count mutual calls
            let mut call_count = 0;
            for callee in &node_a.calls {
                if callee == id_b {
                    call_count += 1;
                }
            }
            for callee in &node_b.calls {
                if callee == id_a {
                    call_count += 1;
                }
            }

            if call_count > 0 {
                pairs.push((
                    id_a.clone(),
                    id_b.clone(),
                    PairInfo {
                        lattice_distance,
                        call_count,
                    },
                ));
            }
        }
    }

    // Sort by (high call count, low lattice distance) — interesting pairs first
    pairs.sort_by(|a, b| {
        let score_a = a.2.call_count as f64 / (a.2.lattice_distance + 1) as f64;
        let score_b = b.2.call_count as f64 / (b.2.lattice_distance + 1) as f64;
        score_b
            .partial_cmp(&score_a)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    pairs.truncate(max_pairs);
    pairs
}

/// Extract algebraic connectivity from analysis (min absolute Fiedler value).
fn compute_algebraic_connectivity_from_analysis(analysis: &SpatialAnalysisResult) -> f64 {
    let values: Vec<f64> = analysis.fiedler_values.values().copied().collect();
    if values.is_empty() {
        return 0.0;
    }
    // Algebraic connectivity ≈ spectral gap between λ₁=0 and λ₂=Fiedler
    // For bipartite-like partition, use min absolute value
    let abs_min = values.iter().map(|v| v.abs()).fold(f64::MAX, f64::min);
    abs_min.max(0.0).min(1.0)
}

/// Generate a short summary line for status bar or tooltip.
pub fn summary_line(graph: &CodeGraph) -> String {
    let analysis = analyze(graph);
    format!(
        "📐 {} files · {} funcs · {} clusters · Q={:.2} · {} bridges",
        graph.modules.len(),
        graph.functions.len(),
        analysis.component_count,
        analysis.modularity,
        analysis.bridge_scores.len()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FileId;
    use eisenstein::E12;

    fn make_test_graph() -> CodeGraph {
        let mut graph = CodeGraph::new();
        let auth = FileId("auth.rs".into());
        let user = FileId("user.rs".into());
        let db = FileId("db.rs".into());

        // Place at specific lattice positions
        let auth_fn = FunctionId {
            file: auth.clone(),
            name: "authenticate".into(),
        };
        let user_fn = FunctionId {
            file: user.clone(),
            name: "get_user".into(),
        };
        let db_fn = FunctionId {
            file: db.clone(),
            name: "connect".into(),
        };

        graph.register_module(auth.clone());
        graph.register_module(user.clone());
        graph.register_module(db.clone());

        graph.register_function(
            auth.clone(),
            "authenticate".into(),
            5, 30,
            Some(E12::new(0, 0)),
        );
        graph.register_function(
            user.clone(),
            "get_user".into(),
            3, 20,
            Some(E12::new(2, 0)),
        );
        graph.register_function(
            db.clone(),
            "connect".into(),
            2, 15,
            Some(E12::new(5, 0)),
        );

        graph.add_call(&auth_fn, &user_fn);
        graph.add_call(&user_fn, &auth_fn); // mutual calling
        graph.add_call(&auth_fn, &db_fn);
        graph.add_call(&user_fn, &db_fn);

        graph.add_import(&auth, &user);
        graph.add_import(&user, &db);
        graph.add_import(&auth, &db);

        graph
    }

    #[test]
    fn test_generate_text_report() {
        let graph = make_test_graph();
        let report = generate_report(&graph, ReportStyle::Normal);
        assert!(report.contains("Spatial Intelligence Report"));
        assert!(report.contains("auth.rs"));
        assert!(report.contains("user.rs"));
        assert!(report.contains("distance"));
        assert!(report.contains("Bridge files"));
    }

    #[test]
    fn test_generate_json_report() {
        let graph = make_test_graph();
        let report = generate_report(&graph, ReportStyle::Json);
        assert!(report.contains("notable_pairs"));
        assert!(report.contains("bridges"));
        assert!(report.contains("reorg_suggestions"));
        // Verify it's valid JSON
        let parsed: serde_json::Value = serde_json::from_str(&report).unwrap();
        assert!(parsed.is_object());
    }

    #[test]
    fn test_summary_line() {
        let graph = make_test_graph();
        let line = summary_line(&graph);
        assert!(line.contains("files"));
        assert!(line.contains("funcs"));
        assert!(line.contains("bridges"));
    }

    #[test]
    fn test_find_notable_pairs() {
        let graph = make_test_graph();
        let pairs = find_notable_pairs(&graph, 10);
        assert!(!pairs.is_empty(), "Should find some notable pairs");
        // auth->user mutual calls should be near the top
        let has_auth_user = pairs.iter().any(|(a, b, _)| {
            (a.name == "authenticate" && b.name == "get_user")
                || (a.name == "get_user" && b.name == "authenticate")
        });
        assert!(has_auth_user, "Should include auth↔user pair");
    }

    #[test]
    fn test_empty_graph_report() {
        let graph = CodeGraph::new();
        let report = generate_report(&graph, ReportStyle::Normal);
        assert!(report.contains("Modules: 0"));
    }
}
