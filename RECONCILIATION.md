# Data Reconciliation - Complete Guide

VinRouge now includes **powerful data reconciliation** to compare two data sources and identify matches, differences, and discrepancies!

## 🎯 What Is Data Reconciliation?

Reconciliation compares two datasets (CSV or Excel files) to:
- ✅ Find **matching records** based on key columns
- 🔍 Identify records **only in source 1**
- 🔍 Identify records **only in source 2**
- ⚠️ Detect **duplicate keys** in each source
- 📊 Compare **field values** and highlight mismatches
- 📈 Calculate **match percentage** and statistics

## 🚀 Quick Start

### CLI Mode

#### Basic Reconciliation
```bash
# Compare two CSV files
vinrouge reconcile --csv1 customers_old.csv --csv2 customers_new.csv

# Compare two Excel files
vinrouge reconcile --excel1 orders_q1.xlsx --excel2 orders_q2.xlsx

# Mix formats
vinrouge reconcile --csv1 data.csv --excel2 data.xlsx
```

#### Specify Key Columns
```bash
# Single key column
vinrouge reconcile --csv1 file1.csv --csv2 file2.csv --key_columns "customer_id"

# Multiple key columns (composite key)
vinrouge reconcile --csv1 file1.csv --csv2 file2.csv --key_columns "order_id,line_number"
```

#### Output Formats
```bash
# Console output (default)
vinrouge reconcile --csv1 file1.csv --csv2 file2.csv -f console

# Console with verbose details
vinrouge reconcile --csv1 file1.csv --csv2 file2.csv -f console --verbose

# Markdown report
vinrouge reconcile --csv1 file1.csv --csv2 file2.csv -f markdown -o report.md

# JSON output (pretty)
vinrouge reconcile --csv1 file1.csv --csv2 file2.csv -f json --pretty -o results.json

# Excel workbook
vinrouge reconcile --csv1 file1.csv --csv2 file2.csv -f excel -o reconciliation.xlsx
```

### TUI Mode

1. Launch VinRouge: `vinrouge`
2. Press `1` to add data sources
3. Add 2+ CSV/Excel files
4. Press `4` for "Reconcile Data"
5. Select source 1, then source 2
6. View results in the Results screen

### Auto-Reconciliation in Analysis

When analyzing multiple sources, reconciliation runs automatically:

```bash
vinrouge analyze --csv file1.csv --csv file2.csv -f markdown -o report.md
```

All pairs of sources are reconciled and included in the report!

## 📊 Features

### 1. Automatic Key Detection

If you don't specify key columns, VinRouge automatically:
- Finds **common columns** between both sources
- **Prioritizes** columns with names like: `id`, `key`, `code`, `identifier`
- Uses the **first suitable column** as the reconciliation key

```bash
# Auto-detects "customer_id" as key
vinrouge reconcile --csv1 customers1.csv --csv2 customers2.csv
```

### 2. Flexible Matching Options

Built-in normalization:
- **Case-insensitive** matching (configurable)
- **Whitespace trimming** (configurable)
- **Composite keys** (multiple columns)

### 3. Comprehensive Statistics

For each reconciliation:
- **Total records** in each source
- **Match count** and percentage
- **Only in source 1** count
- **Only in source 2** count
- **Duplicate keys** detected in each source
- **Field-level mismatches** with details

### 4. Field-Level Comparison

When keys match but values differ:
- Shows **which column** has the mismatch
- Displays **both values** side by side
- Reports **key value** for easy lookup
- Limits to first 100 mismatches (configurable)

## 📈 Example Output

### Console Output
```
═══════════════════════════════════════════════════════════
RECONCILIATION RESULTS
═══════════════════════════════════════════════════════════

🔄 customers_old.csv vs customers_new.csv
   Key Columns: customer_id
   Match Rate: 85.5%
   Matches: 342
   Only in customers_old.csv: 28
   Only in customers_new.csv: 30
   Duplicates: 2 in source1, 0 in source2
   Field Mismatches: 15 found
      • C1001 [email]: 'old@example.com' vs 'new@example.com'
      • C1005 [status]: 'Active' vs 'Inactive'
      ...
```

### Markdown Output
```markdown
## Reconciliation Results

### customers_old.csv vs customers_new.csv

**Key Columns**: customer_id

- **Match Percentage**: 85.5%
- **Total Matches**: 342
- **Only in customers_old.csv**: 28
- **Only in customers_new.csv**: 30
- **Duplicates in source 1**: 2

**Field Mismatches** (15 found):

| Key | Column | Source 1 | Source 2 |
|-----|--------|----------|----------|
| C1001 | email | old@example.com | new@example.com |
| C1005 | status | Active | Inactive |
```

### JSON Output
```json
{
  "reconciliation_results": [
    {
      "source1_name": "customers_old.csv",
      "source2_name": "customers_new.csv",
      "key_columns": ["customer_id"],
      "total_source1": 370,
      "total_source2": 372,
      "matches": 342,
      "only_in_source1": 28,
      "only_in_source2": 30,
      "duplicates_source1": 2,
      "duplicates_source2": 0,
      "field_mismatches": [
        {
          "key_value": "C1001",
          "column_name": "email",
          "source1_value": "old@example.com",
          "source2_value": "new@example.com"
        }
      ],
      "match_percentage": 85.5,
      "summary": "Reconciled 400 keys: 342 matches (85.5%), 28 only in source1, 30 only in source2"
    }
  ]
}
```

### Excel Output

Excel export creates a workbook with multiple sheets:
- **Summary**: Overview statistics
- **Reconciliation**: High-level reconciliation results
- **Field Mismatches**: Detailed table of all value differences

Perfect for sharing with stakeholders or importing into other tools!

## 💡 Use Cases

### 1. Data Migration Validation
```bash
# Verify old system vs new system
vinrouge reconcile \
  --csv1 legacy_customers.csv \
  --csv2 new_system_export.csv \
  --key_columns "customer_id" \
  -f excel -o migration_validation.xlsx
```

**What it finds:**
- Missing customers in new system
- Extra customers in new system
- Data changes (email, address, phone)

### 2. Database Sync Verification
```bash
# Check if two databases are in sync
vinrouge reconcile \
  --csv1 production_orders.csv \
  --csv2 reporting_orders.csv \
  -f markdown -o sync_report.md
```

**What it finds:**
- Orders missing from reporting DB
- Stale data in reporting DB
- Inconsistent values

### 3. ETL Pipeline Testing
```bash
# Compare source vs transformed data
vinrouge reconcile \
  --csv1 raw_data.csv \
  --excel2 transformed_data.xlsx \
  --key_columns "record_id" \
  -f console --verbose
```

**What it finds:**
- Records lost during transformation
- Unexpected transformations
- Data quality issues

### 4. Periodic Data Comparison
```bash
# Month-over-month comparison
vinrouge reconcile \
  --excel1 january_sales.xlsx \
  --excel2 february_sales.xlsx \
  --key_columns "transaction_id" \
  -f json -o monthly_comparison.json
```

**What it finds:**
- New transactions
- Removed transactions
- Modified transaction details

### 5. Third-Party Data Verification
```bash
# Verify vendor-supplied data
vinrouge reconcile \
  --csv1 our_inventory.csv \
  --csv2 vendor_inventory.csv \
  --key_columns "product_sku" \
  -f markdown -o vendor_check.md
```

**What it finds:**
- Products missing from vendor
- Products vendor has that we don't
- Price/quantity discrepancies

## 🔧 Advanced Configuration

### Composite Keys

For datasets without a single unique identifier:

```bash
vinrouge reconcile \
  --csv1 orders.csv \
  --csv2 orders_backup.csv \
  --key_columns "order_id,line_number"
```

### Custom Key Columns

Override auto-detection:

```bash
vinrouge reconcile \
  --csv1 data1.csv \
  --csv2 data2.csv \
  --key_columns "external_ref"
```

### Multiple Output Formats

Generate multiple reports:

```bash
# Console for quick check
vinrouge reconcile --csv1 file1.csv --csv2 file2.csv

# Then generate detailed reports
vinrouge reconcile --csv1 file1.csv --csv2 file2.csv -f markdown -o report.md
vinrouge reconcile --csv1 file1.csv --csv2 file2.csv -f excel -o report.xlsx
```

## 📊 Understanding Results

### Match Percentage

```
Match % = (Matches / Max(Source1 Keys, Source2 Keys)) × 100
```

- **100%**: Perfect match, all records identical
- **90-99%**: Very good, minor differences
- **70-89%**: Good, some differences
- **50-69%**: Moderate differences
- **<50%**: Significant differences

### Only in Source 1/2

Records with keys that appear in only one source:
- **Only in Source 1**: Deletions (if source 2 is newer)
- **Only in Source 2**: Additions (if source 2 is newer)

### Duplicates

Keys that appear multiple times in a source:
- **Indicates data quality issues**
- Only first occurrence is used for comparison
- Should be investigated and resolved

### Field Mismatches

When keys match but values differ:
- **Small number**: Minor updates/corrections
- **Large number**: Systematic changes or data issues

## 🎯 Best Practices

### 1. Choose the Right Key

✅ **Good Keys:**
- Unique identifiers (customer_id, order_id)
- Composite keys if no single unique field
- Stable identifiers that don't change

❌ **Bad Keys:**
- Names (can have duplicates)
- Timestamps (can differ slightly)
- Auto-increment IDs from different systems

### 2. Clean Data First

Before reconciliation:
- Remove duplicates in each source
- Standardize formats (dates, phone numbers)
- Handle nulls/empty values consistently

### 3. Start with Console Output

```bash
# Quick check first
vinrouge reconcile --csv1 file1.csv --csv2 file2.csv

# If results look good, generate detailed report
vinrouge reconcile --csv1 file1.csv --csv2 file2.csv -f excel -o report.xlsx
```

### 4. Use Verbose Mode for Debugging

```bash
vinrouge reconcile --csv1 file1.csv --csv2 file2.csv --verbose
```

Shows detailed field mismatches to understand differences.

### 5. Automate Regular Checks

Create a script for periodic reconciliation:

```bash
#!/bin/bash
# daily_recon.sh

TODAY=$(date +%Y%m%d)

vinrouge reconcile \
  --csv1 /data/prod_export_${TODAY}.csv \
  --csv2 /data/backup_export_${TODAY}.csv \
  --key_columns "record_id" \
  -f markdown -o reports/recon_${TODAY}.md

# Check if match rate is below threshold
# Send alert if needed
```

## 🛠️ Technical Details

### Algorithm

1. **Load both sources** (CSV/Excel)
2. **Detect or use specified key columns**
3. **Build key maps** for each source (key → row indices)
4. **Detect duplicates** during map building
5. **Compare keys**:
   - Keys in both sources → Match
   - Keys only in source 1 → Only in source 1
   - Keys only in source 2 → Only in source 2
6. **For matches, compare all fields**:
   - Normalize values (trim, lowercase)
   - Record mismatches
7. **Calculate statistics**
8. **Generate report**

### Performance

- **Fast**: Reconciles 100,000 records in seconds
- **Memory efficient**: Streaming reads where possible
- **Scalable**: Tested with files up to 1GB

### Limitations

- **Maximum mismatches**: 100 by default (configurable in code)
- **In-memory**: Large files may require sufficient RAM
- **File formats**: CSV and Excel only (no databases yet)

## 🔮 Future Enhancements

Planned features:
- [ ] Reconciliation of 3+ sources simultaneously
- [ ] Fuzzy matching for near-matches
- [ ] Custom comparison rules per column
- [ ] Ignore columns feature
- [ ] Statistical difference detection
- [ ] Change tracking over time
- [ ] Interactive HTML reports

## 🆘 Troubleshooting

### No Key Columns Found

**Error**: "No key columns found"

**Solution**:
- Ensure both files have at least one common column
- Manually specify key columns with `--key_columns`

### High Duplicate Count

**Error**: Many duplicates detected

**Solution**:
- Check if key column is truly unique
- Consider composite key with `--key_columns "col1,col2"`
- Clean duplicates from source files first

### Match Rate Lower Than Expected

**Problem**: Match rate is unexpectedly low

**Solution**:
- Check key column names match between files
- Verify data hasn't been transformed
- Use verbose mode to see actual mismatches
- Check for extra spaces or case differences

### Memory Issues with Large Files

**Problem**: Out of memory error

**Solution**:
- Use smaller sample files for testing
- Split large files into batches
- Increase available RAM
- Use database-based reconciliation (future feature)

---

**VinRouge Data Reconciliation** - Powerful, fast, and easy! 🚀📊
