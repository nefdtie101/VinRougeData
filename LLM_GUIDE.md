# VinRouge — LLM Assistant Guide

This guide is for AI assistants (Claude, ChatGPT, Copilot, etc.) helping a user run VinRouge.

**Core principle: the data never leaves the user's machine.**
The LLM generates commands and config files. The user runs them locally. The user pastes back only the output summary — never the raw data.

---

## How the workflow works

```
User describes their files
        ↓
LLM generates a command or config file  ← you are here
        ↓
User runs it locally on their machine
        ↓
User pastes the output back to the LLM
        ↓
LLM interprets the results
```

You never see the actual rows. You only need column names, file names, and what the user wants to achieve.

---

## What to ask the user (and nothing more)

To generate a command, you need:

| What you need | How to ask |
|---|---|
| File name(s) | "What are the file names?" |
| Column headers | "Can you paste just the header row (first line) of each file?" |
| Goal | "Are you reconciling, analysing, or exporting?" |
| Key column | "Which column uniquely identifies each row? (e.g. Employee ID, Order ID)" — only ask if reconciling |

**Never ask for sample rows, values, or any actual data.**

---

## Command reference

### Reconcile two files

```bash
vinrouge reconcile \
  --csv1 <file1.csv> \
  --csv2 <file2.csv> \
  --key-columns "<Column Name>" \
  -f console
```

- Omit `--key-columns` to let VinRouge auto-detect the join key.
- VinRouge automatically detects columns with different names that contain the same values (e.g. "Net Pay" vs "Debit") — no manual mapping needed.
- Add `-f markdown -o report.md` to write a report file instead of printing to screen.
- Add `-f excel -o report.xlsx` for a spreadsheet report.
- Add `--verbose` to show the full list of field mismatches in the terminal.

**Composite key (when no single column is unique):**
```bash
--key-columns "Order ID,Line Number"
```

**Manual column mapping (only if auto-detection fails):**
```bash
--column-mapping "Net Pay=Debit" --column-mapping "Employee Name=Beneficiary"
```

### Analyse one or more files

```bash
# Single file — profile, relationships, workflows
vinrouge analyze --csv data.csv -f console

# Write a markdown report
vinrouge analyze --csv data.csv -f markdown -o analysis.md

# Excel report
vinrouge analyze --csv data.csv -f excel -o analysis.xlsx

# Multiple sources (auto-reconciles all pairs)
vinrouge analyze --config vinrouge.json -f markdown -o report.md
```

### Generate a config file (for multiple sources)

```bash
vinrouge generate-config -o vinrouge.json
```

Then edit the generated `vinrouge.json` (see config format below).

### Launch interactive TUI

```bash
vinrouge
# or
vinrouge interactive
```

---

## Config file format (JSON)

Use when you have more than two sources, or want repeatable runs.

```json
{
  "sources": [
    {
      "type": "csv",
      "path": "payroll.csv",
      "delimiter": ",",
      "has_header": true
    },
    {
      "type": "csv",
      "path": "bank.csv",
      "delimiter": ",",
      "has_header": true
    },
    {
      "type": "excel",
      "path": "ledger.xlsx",
      "sheet": "Sheet1",
      "has_header": true
    },
    {
      "type": "mssql",
      "connection_string": "Server=localhost;Database=mydb;User=sa;Password=secret;"
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
    "min_confidence": 50
  }
}
```

TOML is also supported — name the file `vinrouge.toml`.

**Source type options:**

| type | required fields | optional fields |
|---|---|---|
| `csv` | `path` | `delimiter` (default `,`), `has_header` (default `true`) |
| `excel` | `path` | `sheet` (default first sheet), `has_header` (default `true`) |
| `mssql` | `connection_string` | `name` |
| `flatfile` | `path` | `delimiter`, `column_widths`, `column_names`, `has_header` |

---

## Output format options

| Flag | What you get |
|---|---|
| `-f console` | Printed to terminal (default) |
| `-f console --verbose` | Terminal output with full mismatch detail |
| `-f markdown -o report.md` | Markdown file |
| `-f json -o results.json` | Machine-readable JSON |
| `-f json --pretty -o results.json` | Formatted JSON |
| `-f excel -o report.xlsx` | Excel workbook with multiple sheets |

---

## Understanding reconciliation output

When the user pastes output, here is what the fields mean:

| Field | Meaning |
|---|---|
| `Matches` | Rows found in both sources with the same key |
| `Only in source1` | Rows whose key exists only in the first file (possibly deleted or missing) |
| `Only in source2` | Rows whose key exists only in the second file (possibly new or extra) |
| `Duplicates` | Keys that appear more than once in a source — a data quality warning |
| `Field Mismatches` | Rows that matched on key but have different values in other columns |
| `Match Rate` | `Matches / max(source1 keys, source2 keys) × 100` |

A **field mismatch** entry looks like:
```
• EMP007 [Net Pay vs Debit]: '3100.50' vs '3100.00'
```
This means: employee EMP007 matched by key, but the amount columns differ by 0.50.

---

## Common scenarios

### Payroll vs bank statement
```bash
vinrouge reconcile \
  --csv1 payroll_export.csv \
  --csv2 bank_statement.csv \
  --key-columns "Employee ID" \
  -f console --verbose
```
VinRouge will auto-detect that "Net Pay" (payroll) corresponds to "Debit" (bank) and flag amount mismatches.

### Data migration check
```bash
vinrouge reconcile \
  --csv1 legacy_system.csv \
  --csv2 new_system.csv \
  --key-columns "customer_id" \
  -f excel -o migration_check.xlsx
```

### Monthly comparison
```bash
vinrouge reconcile \
  --excel1 january.xlsx \
  --excel2 february.xlsx \
  --key-columns "transaction_id" \
  -f markdown -o monthly_diff.md
```

### Profile a single file
```bash
vinrouge analyze --csv data.csv -f console
```

---

## Privacy rules for the LLM

1. **Do not ask for row data.** Column headers are enough to build commands.
2. **Do not ask for connection string passwords** — tell the user to fill those in themselves.
3. **Generate the command or config, then stop.** Let the user run it and paste back the output.
4. **The output summary contains no customer records** — only counts, percentages, and mismatch column names with anonymised key values. It is safe to read and interpret.
5. If the user pastes an error message, diagnose from the error text alone — do not ask them to share file contents.

---

## Troubleshooting (diagnose from error text only)

| Error | Likely cause | Fix to suggest |
|---|---|---|
| `No key columns found` | No column with the same name in both files | Ask the user for both header rows, then suggest `--key-columns` |
| `Key columns not found in both sources` | `--key-columns` value doesn't match any header | Check spelling/case against the header row the user provides |
| `Failed to read config file` | Bad JSON/TOML syntax | Regenerate the config block carefully |
| Very low match rate (< 30%) | Wrong key column chosen | Ask which column uniquely identifies each row |
| High duplicate count | Key column is not unique | Suggest a composite key or ask which combination of columns is unique |
| `apple can't verify` (macOS) | Gatekeeper quarantine | Run: `xattr -d com.apple.quarantine ./vinrouge` then retry |
