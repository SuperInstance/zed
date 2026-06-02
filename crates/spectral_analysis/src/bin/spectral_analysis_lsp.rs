//! Spectral Analysis LSP server.
//!
//! A minimal LSP that provides codebase dependency graph analysis via
//! spectral methods. Connects to zed's LSP client and returns diagnostics
//! about module coupling, bottlenecks, and suggested module boundaries.

use spectral_analysis::{
    Entity, DependencyEdge, DependencyWeight, CodeGraph,
    analyze_code_graph, format_summary,
};
use std::io::{self, BufRead, Write};

/// Very simple JSON-RPC style LSP. In production this would use the full
/// LSP protocol (lsp-server crate, tower-lsp, etc.).
fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stdout = stdout.lock();

    // Initialize response
    let init_response = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 0,
        "result": {
            "capabilities": {
                "textDocumentSync": 1,
                "spectralAnalysis": {
                    "version": "0.1.0",
                    "methods": [
                        "fiedlerValue",
                        "cheegerBounds",
                        "communityDetection",
                        "bottleneckAnalysis",
                        "effectiveResistance"
                    ]
                }
            }
        }
    });

    let _ = writeln!(stdout, "Content-Length: {}\r\n\r\n{}",
        serde_json::to_string(&init_response).map(|s| s.len()).unwrap_or(0),
        serde_json::to_string(&init_response).unwrap_or_default()
    );
    let _ = stdout.flush();

    // Read and process messages
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        // Try to extract JSON body after Content-Length header
        if !line.starts_with('{') {
            continue;
        }

        if let Ok(msg) = serde_json::from_str::<serde_json::Value>(&line) {
            if let Some(method) = msg.get("method").and_then(|m| m.as_str()) {
                match method {
                    "textDocument/didOpen" | "textDocument/didChange" => {
                        // Analyze the codebase — in a real implementation we'd
                        // build the dependency graph from the project data.
                        let graph = build_sample_graph();
                        let report = analyze_code_graph(&graph);

                        // Send diagnostic notification
                        let summary = format_summary(&report);
                        let diag = serde_json::json!({
                            "jsonrpc": "2.0",
                            "method": "textDocument/publishDiagnostics",
                            "params": {
                                "uri": "file:///codebase",
                                "diagnostics": [
                                    {
                                        "range": {
                                            "start": { "line": 0, "character": 0 },
                                            "end": { "line": 0, "character": 0 }
                                        },
                                        "severity": 2,
                                        "message": summary.trim().lines().next().unwrap_or("Spectral analysis complete")
                                    }
                                ]
                            }
                        });
                        let _ = writeln!(stdout, "Content-Length: {}\r\n\r\n{}",
                            serde_json::to_string(&diag).map(|s| s.len()).unwrap_or(0),
                            serde_json::to_string(&diag).unwrap_or_default()
                        );
                        let _ = stdout.flush();
                    }
                    _ => {}
                }
            }
        }
    }
}

/// Build a sample graph for demonstration purposes.
fn build_sample_graph() -> CodeGraph {
    let mut graph = CodeGraph::new();

    // Editor core
    graph.add_edge(DependencyEdge {
        from: Entity::File("src/editor.rs".into()),
        to: Entity::File("src/buffer.rs".into()),
        weight: DependencyWeight::Import,
    });
    graph.add_edge(DependencyEdge {
        from: Entity::File("src/editor.rs".into()),
        to: Entity::File("src/gpui.rs".into()),
        weight: DependencyWeight::Import,
    });
    graph.add_edge(DependencyEdge {
        from: Entity::File("src/editor.rs".into()),
        to: Entity::File("src/project.rs".into()),
        weight: DependencyWeight::Import,
    });

    // Buffer
    graph.add_edge(DependencyEdge {
        from: Entity::File("src/buffer.rs".into()),
        to: Entity::File("src/text.rs".into()),
        weight: DependencyWeight::Import,
    });
    graph.add_edge(DependencyEdge {
        from: Entity::File("src/buffer.rs".into()),
        to: Entity::File("src/language.rs".into()),
        weight: DependencyWeight::Module,
    });

    // GPUI
    graph.add_edge(DependencyEdge {
        from: Entity::File("src/gpui.rs".into()),
        to: Entity::File("src/window.rs".into()),
        weight: DependencyWeight::Import,
    });
    graph.add_edge(DependencyEdge {
        from: Entity::File("src/gpui.rs".into()),
        to: Entity::File("src/scene.rs".into()),
        weight: DependencyWeight::Import,
    });

    // Project
    graph.add_edge(DependencyEdge {
        from: Entity::File("src/project.rs".into()),
        to: Entity::File("src/language.rs".into()),
        weight: DependencyWeight::Module,
    });
    graph.add_edge(DependencyEdge {
        from: Entity::File("src/project.rs".into()),
        to: Entity::File("src/worktree.rs".into()),
        weight: DependencyWeight::Import,
    });

    // Collab
    graph.add_edge(DependencyEdge {
        from: Entity::File("src/collab.rs".into()),
        to: Entity::File("src/project.rs".into()),
        weight: DependencyWeight::Module,
    });
    graph.add_edge(DependencyEdge {
        from: Entity::File("src/collab.rs".into()),
        to: Entity::File("src/rpc.rs".into()),
        weight: DependencyWeight::Import,
    });
    graph.add_edge(DependencyEdge {
        from: Entity::File("src/rpc.rs".into()),
        to: Entity::File("src/buffer.rs".into()),
        weight: DependencyWeight::Module,
    });

    graph
}
