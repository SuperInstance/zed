# spatial — Eisenstein Spatial Intelligence for Zed

A Rust crate that maps code structure to the **Eisenstein hexagonal lattice**,
enabling spatial reasoning about codebases: function proximity, module
clustering, bridge detection, and Pythagorean cursor jumps.

## Why It Exists

Code relationships aren't linear. Functions in the same module have 6
"close" neighbors (adjacent hexes). Cross-module calls are further away.
Import coupling is **lattice distance**.

Traditional metrics (fan-in, fan-out, cyclomatic complexity) are scalar.
Spatial intelligence gives you **geometry** — a structured way to ask
"how close are these functions *really*?"

## Modules

| Module | What it does |
|---|---|
| `code_graph` | Functions as nodes on the Eisenstein lattice, calls as edges, modules as Voronoï cells |
| `snap_navigation` | Pythagorean snap cursor jumps, Eisenstein distance for related-code finding |
| `spatial_analysis` | File dependency → Fiedler vector, bridge file detection, reorganization suggestions |
| `report` | Show-don't-sell diagnostic output |

## Quick Start

```rust
use spatial::{CodeGraph, FileId, LatticePos};
use spatial::report::{generate_report, ReportStyle};

let mut graph = CodeGraph::new();

// Register modules
let auth = FileId("auth.rs".into());
let user = FileId("user.rs".into());

// Register functions with positions
graph.register_function(auth.clone(), "login".into(), 5, 30, None);
graph.register_function(user.clone(), "get_profile".into(), 3, 20, None);

// Add call edges
graph.add_call(&fn_login, &fn_get_profile);

// Generate report
let report = generate_report(&graph, ReportStyle::Normal);
println!("{}", report);
```

## Lattice Layout

Functions are placed on the Eisenstein lattice via a hexagonal spiral.
Modules form Voronoï cells. Cross-module distance = hex distance.

- Distance 1 → same module (shared hex edge)
- Distance 2 → reachable via one hop
- Distance 3+ → increasingly distant coupling

## Analysis

### Fiedler Vector

The Laplacian of the file-dependency graph is computed, and the second
eigenvector (Fiedler vector) reveals natural partitions. Files with
positive values go one way, negative the other.

### Bridge Detection

Files whose Fiedler-adjacency spans both clusters are structural bridges.
Removing them increases the shortest path between modules.

### Pythagorean Snap Jumps

Cursor positions snap to Eisenstein triple distances — positions
(a, b) where a² - ab + b² = c². This creates "quantized" navigation
between related code locations.

## Dependencies

- `eisenstein` (v0.3) — Eisenstein integer lattice
- `snapkit` (v0.1) — Voronoï snap, spectral analysis
- `nalgebra` (v0.33) — Linear algebra for spectral embedding
- `serde` / `serde_json` — JSON report output

## License

MIT
