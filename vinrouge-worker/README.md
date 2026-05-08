# vinrouge-worker

A headless HTTP server that loads VinRouge audit projects and exposes their DSL scripts via a REST API and rendered HTML dashboards.

## Overview

The worker is the server-side counterpart to the desktop app. It scans a directory for `.vrd` project files, builds in-memory datasources from each project's SQLite session data, and serves the results of DSL scripts over HTTP.

## Getting Started

```bash
# Build and run from this directory
cargo run
```

The server reads `config.toml` on startup:

```toml
port     = 3000
data_dir = "./projects"
```

Place `.vrd` project files in `data_dir` before starting. The worker extracts each archive and loads it automatically.

## Routes

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/` | HTML list of all loaded projects |
| `GET` | `/projects/:id` | JSON — project metadata and available scripts |
| `POST` | `/projects/:id/scripts/:script_id/run` | JSON — run a DSL script and return results |
| `GET` | `/projects/:id/scripts/:script_id/dashboard` | HTML — rendered dashboard for a script |
| `GET` | `/assets/*` | Static CSS/JS assets |

## Project Layout

```
vinrouge-worker/
├── config.toml          # Port and data directory
├── projects/            # Drop .vrd files here
├── assets/              # Static files served at /assets
├── templates/           # Minijinja HTML templates
└── src/
    ├── main.rs          # Server entry point
    ├── config.rs        # Config loading
    ├── state.rs         # Shared app state (projects, cached datasources)
    ├── executor/        # .vrd scanning, datasource building, DSL execution
    ├── renderer/        # HTML rendering of DSL results
    ├── session_reader/  # Reads imported data rows from project SQLite DB
    └── routes/
        ├── api.rs       # JSON endpoints
        └── dashboard.rs # HTML dashboard endpoints
```

## Result Types

DSL script results are serialised to JSON with a `kind` discriminant:

| Kind | Description |
|------|-------------|
| `assert` | Boolean test — `passed`, `lhs_value`, `rhs_value`, `op` |
| `sample` | Statistical sample — method, population size, selected indices |
| `chart` | Chart data — type, labels, values |
| `section` | Named group of nested results with pass/fail summary |
| `schema` | Table schema — name, row count, columns |
| `value` | Scalar value |
| `error` | DSL runtime error message |

## Environment Variables

Set `RUST_LOG` to control log verbosity:

```bash
RUST_LOG=debug cargo run
```

Default filter: `vinrouge_worker=info,warn`.
