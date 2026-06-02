# Spectral Analysis Integration Guide

## Overview

The `spectral_analysis` crate provides spectral graph analysis of codebase dependency graphs. It uses [`cathedral_probe`](https://crates.io/crates/cathedral-probe) to compute:

- **Fiedler value** — algebraic connectivity of your codebase
- **Cheeger bounds** — bottleneck detection
- **Community detection** — natural module boundaries
- **Effective resistance / importance** — critical files

## Integration Points

### 1. Rust API (crate)

```rust
use spectral_analysis::{
    CodeGraph, Entity, DependencyEdge, DependencyWeight,
    analyze_code_graph, format_summary, SpectralReport,
};

// Build a dependency graph
let mut graph = CodeGraph::new();
graph.add_edge(DependencyEdge {
    from: Entity::File("src/main.rs".into()),
    to: Entity::File("src/lib.rs".into()),
    weight: DependencyWeight::Import,
});

// Analyze
let report = analyze_code_graph(&graph);
println!("{}", format_summary(&report));
```

### 2. JSON Export

```rust
use spectral_analysis::{report_to_json, report_from_json};

let json = report_to_json(&report);
let parsed = report_from_json(&json.to_string())?;
```

### 3. File-level Analysis

```rust
use std::collections::HashMap;
use spectral_analysis::analyze_file_dependencies;

let mut deps: HashMap<String, Vec<String>> = HashMap::new();
deps.insert("main.rs".into(), vec!["utils.rs".into(), "config.rs".into()]);
let report = analyze_file_dependencies(deps);
```

### 4. Zed Extension

The `extensions/spectral_analysis/` directory contains a Zed extension that provides spectral analysis as a language server. The LSP binary is built from `crates/spectral_analysis/src/bin/spectral_analysis_lsp.rs`.

### 5. CLI Binary

Build and run the analysis on any project:

```bash
cargo run --bin spectral_analysis_lsp
```

## Dependencies

- `cathedral-probe` — spectral graph computations (crates.io)
- `serde` / `serde_json` — serialization
- Standard Zed workspace crate deps

## Architecture

```
User code
    │
    ▼
spectral_analysis (Rust crate)
    │
    ├── CodeGraph + CatalogProbe
    │       │
    │       ├── Fiedler eigenvalue
    │       ├── Cheeger bounds
    │       ├── Community detection
    │       └── Importance scores
    │
    ├── SpectralReport (JSON-serializable)
    │
    └── zed_extension (LSP adapter)
            │
            └── zed ui panel
```

## Adding to Zed

In `crates/zed/src/zed.rs`, register the spectral analysis panel alongside other panels:

```rust
use spectral_analysis::SpectralAnalysisPanel;

// In the workspace initialization:
let spectral_panel = SpectralAnalysisPanel::load(workspace_handle.clone(), cx.clone());
```

## Testing

```bash
cargo test -p spectral_analysis
```
