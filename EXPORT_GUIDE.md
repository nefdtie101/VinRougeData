# Export Guide - All Formats Available!

VinRouge supports **4 powerful export formats** for your data analysis results, including **Excel**!

## 📊 Available Export Formats

### 1. JSON Export (.json)
**Perfect for:** APIs, automation, data integration

**Features:**
- ✅ Complete data structure
- ✅ Machine-readable
- ✅ Pretty formatting option
- ✅ Works with any JSON parser

**CLI Example:**
```bash
# Pretty formatted
vinrouge analyze --csv data.csv -f json --pretty -o results.json

# Compact
vinrouge analyze --csv data.csv -f json -o results.json

# Reconciliation to JSON
vinrouge reconcile --csv1 old.csv --csv2 new.csv -f json --pretty -o recon.json
```

**TUI: Press 5 → Press 1 → Enter filename → Press Enter**

---

### 2. Markdown Export (.md)
**Perfect for:** Documentation, GitHub, reports, sharing

**Features:**
- ✅ Human-readable tables
- ✅ GitHub-flavored markdown
- ✅ Easy to version control
- ✅ Can be converted to PDF/HTML

**CLI Example:**
```bash
vinrouge analyze --csv data.csv -f markdown -o report.md
vinrouge reconcile --csv1 old.csv --csv2 new.csv -f markdown -o recon.md
```

**TUI: Press 5 → Press 2 → Enter filename → Press Enter**

---

### 3. Excel Export (.xlsx) ⭐ NEW!
**Perfect for:** Business users, stakeholders, spreadsheet analysis

**Features:**
- ✅ Multiple worksheets (Summary, Tables, Relationships, Workflows, Reconciliation)
- ✅ Formatted columns with proper widths
- ✅ Bold headers with colors
- ✅ Ready for pivot tables and charts
- ✅ Field mismatches in separate detailed section

**Worksheets Created:**
1. **Summary** - Overview statistics with counts
2. **Tables** - All discovered tables with row/column counts
3. **Relationships** - Detected relationships between tables
4. **Workflows** - Identified workflows and patterns
5. **Data Profiling** - Column patterns and correlations (if available)
6. **Grouping Analysis** - Dimension analysis (if available)
7. **Reconciliation** - Comparison results with mismatches

**CLI Example:**
```bash
# Analysis to Excel
vinrouge analyze --csv data.csv -f excel -o analysis.xlsx

# Reconciliation to Excel
vinrouge reconcile --csv1 old.csv --csv2 new.csv -f excel -o comparison.xlsx

# Multiple sources analyzed together
vinrouge analyze --csv data1.csv --csv data2.csv -f excel -o multi_source.xlsx
```

**TUI: Press 5 → Press 3 → Enter filename → Press Enter**

---

### 4. Console Output
**Perfect for:** Quick checks, terminal workflows, scripting

**Features:**
- ✅ Color-coded output
- ✅ Verbose mode for details
- ✅ No file creation needed
- ✅ Pipe to other commands

**CLI Example:**
```bash
# Basic console output
vinrouge analyze --csv data.csv -f console

# Verbose mode with details
vinrouge analyze --csv data.csv -f console --verbose

# Pipe to less for scrolling
vinrouge analyze --csv data.csv -f console | less
```

**TUI: Automatically shown in Results view (Press 3)**

---

## 🎮 TUI Navigation

### Main Menu
```
┌─────────────────────────────────────┐
│         Main Menu                   │
├─────────────────────────────────────┤
│  1. Manage Data Sources            │
│  2. Run Analysis                   │
│  3. View Results                   │
│  4. Reconcile Data                 │
│  5. Export Results     ⬅️ HERE     │
│                                     │
│  ?. Help                           │
│  q. Quit                           │
└─────────────────────────────────────┘
```

### Export Menu (Press 5)
```
┌─────────────────────────────────────┐
│      Select export format:          │
├─────────────────────────────────────┤
│         Formats                     │
├─────────────────────────────────────┤
│  1. JSON (.json)                   │
│  2. Markdown (.md)                 │
│  3. Excel (.xlsx)      ⬅️ EXCEL!   │
└─────────────────────────────────────┘

Press 1-3 to select format, Esc to cancel
```

### Filename Input
```
┌─────────────────────────────────────┐
│      Enter filename:                │
├─────────────────────────────────────┤
│  analysis_results_                  │
└─────────────────────────────────────┘

Enter filename (without extension)
Press Enter to export, Esc to go back
```

---

## 📋 What Gets Exported

### All Formats Include:

#### Summary Section
- Number of tables discovered
- Number of relationships detected
- Number of workflows identified
- Number of data profiles generated
- Number of grouping analyses performed
- Number of reconciliations completed

#### Tables Section
- Table names
- Source type (CSV, Excel, MSSQL)
- Source location (file path)
- Column count
- Row count
- Column details (name, type, nullable, PK, FK)

#### Relationships Section
- From table.column
- To table.column
- Relationship type (Foreign Key, Name Match, Value Overlap, etc.)
- Confidence scores

#### Workflows Section
- Workflow type (Import, Staging, Aggregation, etc.)
- Description
- Confidence percentage
- Steps involved

#### Data Profiling (if performed)
- Column patterns (Sequential, Unique, Category, etc.)
- Unique value counts
- Null counts
- Top values
- Column correlations (1:1, 1:Many, Many:1, Functional)

#### Grouping Analysis (if performed)
- Grouping dimensions found
- Dimension types (Temporal, Categorical, Geographic, etc.)
- Group counts and statistics
- Hierarchies detected
- Suggested analyses

#### Reconciliation Results
- Source names being compared
- Key columns used
- Match percentage
- Total matches
- Records only in source 1
- Records only in source 2
- Duplicate keys in each source
- Field-level mismatches with details

---

## 🎯 Use Cases by Format

### When to Use JSON
✅ Building APIs or integrations
✅ Automating data pipelines
✅ Machine processing required
✅ Need programmatic access

**Example Flow:**
```bash
vinrouge analyze --csv data.csv -f json -o results.json
python process_results.py results.json
```

### When to Use Markdown
✅ Creating documentation
✅ Sharing in GitHub/GitLab
✅ Including in wiki/docs site
✅ Version controlling reports
✅ Converting to PDF/HTML later

**Example Flow:**
```bash
vinrouge analyze --csv data.csv -f markdown -o report.md
git add report.md && git commit -m "Add analysis report"
pandoc report.md -o report.pdf
```

### When to Use Excel ⭐
✅ Sharing with business users
✅ Non-technical stakeholders
✅ Need spreadsheet analysis
✅ Creating charts/pivots
✅ Combining with other Excel data
✅ Email attachments

**Example Flow:**
```bash
vinrouge analyze --csv data.csv -f excel -o analysis.xlsx
# Open in Excel, create pivot tables, add charts
# Share via email or OneDrive
```

### When to Use Console
✅ Quick verification
✅ Terminal-based workflows
✅ Shell scripting
✅ No file needed

**Example Flow:**
```bash
vinrouge analyze --csv data.csv -f console | grep "WARNING"
vinrouge reconcile --csv1 a.csv --csv2 b.csv -f console --verbose | tee log.txt
```

---

## 🔥 Pro Tips

### 1. Generate Multiple Formats
```bash
# Create all three file formats for different audiences
vinrouge analyze --csv data.csv -f excel -o analysis.xlsx
vinrouge analyze --csv data.csv -f markdown -o analysis.md
vinrouge analyze --csv data.csv -f json -o analysis.json
```

### 2. Reconciliation to Excel for Stakeholders
```bash
# Perfect for audit reports
vinrouge reconcile \
  --csv1 production_db.csv \
  --csv2 backup_db.csv \
  --key_columns "customer_id" \
  -f excel -o audit_report.xlsx
```

The Excel file will have:
- **Summary sheet** with high-level stats
- **Reconciliation sheet** with match rates
- **Field Mismatches sheet** with all differences

### 3. Automated Daily Reports
```bash
#!/bin/bash
DATE=$(date +%Y%m%d)

# Generate daily reconciliation report
vinrouge reconcile \
  --csv1 /data/production_${DATE}.csv \
  --csv2 /data/reporting_${DATE}.csv \
  -f excel -o reports/sync_check_${DATE}.xlsx

# Email to team
mail -s "Daily Sync Report" team@company.com -A reports/sync_check_${DATE}.xlsx
```

### 4. TUI Quick Export Workflow
```
1. vinrouge              (Launch TUI)
2. Press 1               (Add sources)
3. Add CSV/Excel files
4. Press Esc             (Back to main)
5. Press 2               (Run analysis)
6. Press 5               (Export)
7. Press 3               (Choose Excel)
8. Type: daily_report
9. Press Enter           (Export complete!)
```

### 5. Combine with Other Tools
```bash
# Export to JSON, then query with jq
vinrouge analyze --csv data.csv -f json -o results.json
cat results.json | jq '.reconciliation_results[0].match_percentage'

# Export to Markdown, convert to PDF
vinrouge analyze --csv data.csv -f markdown -o report.md
pandoc report.md -o report.pdf

# Excel for pivot tables and charts
vinrouge analyze --csv data.csv -f excel -o analysis.xlsx
# Open in Excel, create visualizations
```

---

## 📊 Excel Export Details

### Sheet 1: Summary
| Metric | Count |
|--------|-------|
| Tables | 3 |
| Relationships | 5 |
| Workflows | 2 |
| Reconciliations | 1 |

### Sheet 2: Reconciliation
| Source 1 | Source 2 | Key Columns | Match % | Matches | Only S1 | Only S2 | Dups S1 | Dups S2 | Mismatches |
|----------|----------|-------------|---------|---------|---------|---------|---------|---------|------------|
| old.csv | new.csv | id | 85.5 | 342 | 28 | 30 | 2 | 0 | 15 |

### Sheet 3: Field Mismatches
| Key Value | Column | Source 1 Value | Source 2 Value |
|-----------|--------|----------------|----------------|
| C1001 | email | old@example.com | new@example.com |
| C1005 | status | Active | Inactive |

All formatted with:
- ✅ Bold headers
- ✅ Proper column widths
- ✅ Color-coded sections
- ✅ Ready for analysis

---

## ✅ Quick Reference

| Format | Extension | CLI Flag | TUI Option | Best For |
|--------|-----------|----------|------------|----------|
| JSON | .json | `-f json` | Press 1 | APIs, automation |
| Markdown | .md | `-f markdown` | Press 2 | Documentation |
| Excel | .xlsx | `-f excel` | Press 3 | Business users |
| Console | - | `-f console` | View Results | Quick checks |

---

**All export formats are fully implemented and tested!** 🚀

Choose the format that best fits your workflow and audience!
