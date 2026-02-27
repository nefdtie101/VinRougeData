# VinRouge

An **interactive terminal application** for analyzing and understanding data structures and workflows from multiple input sources - similar to Claude Code!

## 🎯 Modes

- **🖥️ Interactive TUI Mode**: Full-featured terminal UI with menu navigation (default)
- **⚡ CLI Mode**: Command-line interface for automation and scripting

## Features

### Data Source Support
- **MSSQL**: Full schema extraction including tables, columns, data types, keys, and constraints
- **CSV**: Automatic type inference and potential key detection
- **Excel**: Multi-sheet support with type detection
- **Flat Files**: Delimited and fixed-width file parsing

### Analysis Capabilities
- **Schema Discovery**: Automatically extract table and column metadata
- **Relationship Detection**:
  - Explicit foreign key relationships from databases
  - Heuristic detection based on column names and patterns
  - Data value overlap analysis
- **Workflow Detection**: Identify data processing patterns including:
  - File import pipelines
  - Staging-to-production flows
  - Aggregation patterns
  - Lookup/reference tables

### Export Formats
- **JSON**: Structured data output
- **Markdown**: Human-readable reports
- **Console**: CLI-friendly formatted output

## Installation

```bash
cargo build --release
```

The binary will be available at `target/release/vinrouge`

## Usage

### 🎨 Interactive TUI Mode (Default)

Simply run the application to launch the interactive terminal UI:

```bash
./target/release/vinrouge
```

**TUI Features:**
- 📋 Main menu with numbered options
- 🗂️ **Visual file browser** for CSV/Excel files (no typing paths!)
- ➕ Add multiple data sources (CSV, Excel, MSSQL)
- 📊 View and manage configured sources
- 🔍 Run analysis with live updates
- 📈 Scrollable results viewer
- ❓ Built-in help screen (press `?`)

**Keyboard Shortcuts:**
```
Global:
  q, Ctrl+C  - Quit application
  Esc        - Go back / Cancel current action
  ?          - Show help screen

Main Menu:
  1          - Add data source
  2          - View sources
  3          - Run analysis
  4          - View results

Lists:
  ↑/k        - Move up
  ↓/j        - Move down
  d          - Delete selected item

Results:
  ↑↓/jk      - Scroll up/down
  PgUp/PgDn  - Page up/down
  Home       - Jump to top
```

### 📟 CLI Mode

For automation and scripting, use the CLI commands:

**Analyze a CSV file:**
```bash
vinrouge analyze --csv data/customers.csv
```

**Analyze an Excel file:**
```bash
vinrouge analyze --excel data/sales.xlsx -f markdown -o report.md
```

**Connect to MSSQL:**
```bash
vinrouge analyze --mssql "Server=localhost;Database=mydb;User=sa;Password=***;TrustServerCertificate=true"
```

### Using Configuration Files

Generate a sample configuration:
```bash
vinrouge generate-config -o config.json
```

Run analysis with configuration:
```bash
vinrouge analyze -c config.json -f json -o results.json
```

### Configuration Format

```json
{
  "sources": [
    {
      "type": "mssql",
      "connection_string": "Server=localhost;Database=mydb;...",
      "name": "Production DB"
    },
    {
      "type": "csv",
      "path": "data/customers.csv",
      "delimiter": ",",
      "has_header": true
    },
    {
      "type": "excel",
      "path": "data/sales.xlsx",
      "sheet": null,
      "has_header": true
    }
  ],
  "export": {
    "format": "markdown",
    "output_path": "report.md",
    "pretty": true,
    "verbose": false
  },
  "analysis": {
    "detect_relationships": true,
    "detect_workflows": true,
    "min_confidence": 70
  }
}
```

## Command Reference

### `vinrouge` (no arguments)

Launch interactive TUI mode (default).

### `interactive`

Explicitly launch interactive TUI mode:
```bash
vinrouge interactive
```

### `analyze`

Analyze data sources and generate reports.

**Options:**
- `-c, --config <FILE>` - Path to configuration file
- `-f, --format <FORMAT>` - Output format: json, markdown, console (default: console)
- `-o, --output <FILE>` - Output file path (stdout if not specified)
- `-p, --pretty` - Pretty print output
- `--mssql <CONNECTION>` - MSSQL connection string
- `--csv <FILE>` - CSV file path
- `--excel <FILE>` - Excel file path
- `-v, --verbose` - Enable verbose logging

### `generate-config`

Generate a sample configuration file.

**Options:**
- `-o, --output <FILE>` - Output path for config file (default: vinrouge.json)

## Architecture

```
src/
├── main.rs              # CLI entry point + TUI launcher
├── lib.rs               # Library interface
├── tui/                 # Interactive TUI
│   ├── mod.rs          # TUI setup & event loop
│   ├── app.rs          # Application state & logic
│   ├── ui.rs           # UI rendering
│   └── events.rs       # Keyboard event handling
├── config/              # Configuration management
├── sources/             # Data source connectors
│   ├── mssql.rs        # SQL Server
│   ├── csv_source.rs   # CSV files
│   ├── excel.rs        # Excel files
│   └── flatfile.rs     # Flat files
├── schema/              # Schema representation
│   ├── table.rs        # Table metadata
│   ├── column.rs       # Column metadata
│   └── relationship.rs # Relationships
├── analysis/            # Analysis logic
│   ├── relationship_detector.rs
│   └── workflow_detector.rs
└── export/              # Output generation
    ├── json.rs
    ├── markdown.rs
    └── console.rs
```

## Design Philosophy

VinRouge is built on deterministic, rule-based analysis. It does not use AI or machine learning - all inferences are based on:

- Explicit metadata from databases
- Pattern matching on names and structures
- Statistical analysis of data values
- Well-defined heuristics

This ensures:
- Transparent, explainable results
- Consistent behavior
- Predictable performance
- Easy debugging and maintenance

## Future Enhancements (Planned)

Phase 2 will introduce:
- Intelligent workflow inference
- Semantic table classification
- Automated documentation generation
- Data quality scoring

## License

[Your License Here]

## Contributing

[Your Contributing Guidelines Here]
