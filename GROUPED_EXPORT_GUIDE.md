# Grouped Data Export - Separate Excel Files by Category!

VinRouge now includes a powerful **Grouped Data Export** feature that automatically splits your data into separate Excel sheets based on discovered grouping dimensions!

## 🎯 What Is Grouped Data Export?

When VinRouge analyzes your data, it discovers **grouping dimensions** like:
- Customer IDs
- Product categories
- Status values
- Regions
- Dates

The **Grouped Data Export** creates an Excel workbook where **each unique value becomes its own sheet** with all matching records!

## 📊 How It Works

### 1. Analysis Phase
```bash
vinrouge analyze --csv sales.csv
```

VinRouge finds grouping dimensions:
- `customer_id` (5 groups: C001, C002, C003, C004, C005)
- `product_category` (3 groups: Electronics, Furniture, Clothing)
- `region` (3 groups: East, West, South)
- `status` (3 groups: Shipped, Pending, Delivered)

### 2. Grouped Export
```bash
vinrouge analyze --csv sales.csv -f grouped-excel -o sales_grouped
```

Creates an Excel file with sheets like:
- **Summary** - Overview of all grouping dimensions
- **customer_id=C001** - All orders for customer C001
- **customer_id=C002** - All orders for customer C002
- **product_category=Electronics** - All electronics orders
- **product_category=Furniture** - All furniture orders
- **region=East** - All orders from East region
- **region=West** - All orders from West region
...and more!

## 🚀 Quick Start

### CLI Mode

```bash
# Basic grouped export
vinrouge analyze --csv data.csv -f grouped-excel -o grouped_data.xlsx

# Multiple sources (creates separate files)
vinrouge analyze --csv customers.csv --csv orders.csv -f grouped-excel -o analysis

# Result:
#   - analysis_customers.xlsx (grouped customer data)
#   - analysis_orders.xlsx (grouped order data)
```

### TUI Mode

1. Launch VinRouge: `vinrouge`
2. Add data sources (Press **1**)
3. Run analysis (Press **2**)
4. Export results (Press **5**)
5. Select **Grouped Data Excel** (Press **4**)
6. Enter filename
7. Press Enter → Grouped Excel file created!

## 📋 Excel Workbook Structure

### Sheet 1: Summary
Lists all grouping dimensions found:

| Grouping Dimensions | Groups | Type |
|---------------------|--------|------|
| customer_id | 5 | Identifier |
| product_category | 3 | Categorical |
| region | 3 | Geographic |
| status | 3 | Categorical |

### Grouped Data Sheets

Each group becomes a sheet with:
- **Sheet Name**: `dimension=value` (e.g., `customer_id=C001`)
- **Headers**: All original column names
- **Data**: Only records matching that group value

#### Example: `customer_id=C001` Sheet

| id | customer_id | product_category | region | amount | status | order_date |
|----|-------------|------------------|--------|--------|--------|------------|
| 1 | C001 | Electronics | East | 1000 | Shipped | 2024-01-15 |
| 2 | C001 | Electronics | East | 500 | Pending | 2024-01-16 |
| 7 | C001 | Electronics | East | 800 | Delivered | 2024-01-20 |

#### Example: `product_category=Furniture` Sheet

| id | customer_id | product_category | region | amount | status | order_date |
|----|-------------|------------------|--------|--------|--------|------------|
| 3 | C002 | Furniture | West | 2000 | Shipped | 2024-01-15 |
| 5 | C002 | Furniture | West | 1500 | Delivered | 2024-01-18 |
| 8 | C005 | Furniture | West | 1200 | Shipped | 2024-01-21 |

## 💡 Use Cases

### 1. Customer-Specific Reports

**Scenario**: Create separate reports for each customer

```bash
vinrouge analyze --csv orders.csv -f grouped-excel -o customer_reports

# Result: Excel file with sheets for each customer
# - customer_id=C001 (all C001's orders)
# - customer_id=C002 (all C002's orders)
# - customer_id=C003 (all C003's orders)
```

**Why?**
- Send each customer their own data
- Privacy-compliant reporting
- Easy to filter and review

### 2. Regional Analysis

**Scenario**: Analyze sales by geographic region

```bash
vinrouge analyze --csv sales.csv -f grouped-excel -o regional_sales

# Result: Sheets grouped by region
# - region=East (all Eastern sales)
# - region=West (all Western sales)
# - region=North (all Northern sales)
# - region=South (all Southern sales)
```

**Why?**
- Regional managers get their data
- Compare regional performance
- Identify geographic trends

### 3. Product Category Breakdown

**Scenario**: Separate data by product category

```bash
vinrouge analyze --csv inventory.csv -f grouped-excel -o products_by_category

# Result: Sheets for each category
# - product_category=Electronics
# - product_category=Furniture
# - product_category=Clothing
```

**Why?**
- Category managers see their products
- Calculate category-specific metrics
- Plan inventory by category

### 4. Status-Based Workflow

**Scenario**: Group tasks/orders by status

```bash
vinrouge analyze --csv tasks.csv -f grouped-excel -o tasks_by_status

# Result: Sheets for each status
# - status=New
# - status=In Progress
# - status=Completed
# - status=Blocked
```

**Why?**
- Team members see their active tasks
- Monitor workflow stages
- Identify bottlenecks

### 5. Time-Period Analysis

**Scenario**: Group data by date/month/quarter

```bash
vinrouge analyze --csv transactions.csv -f grouped-excel -o transactions_by_period

# Result: Sheets for each time period
# - order_date=2024-01
# - order_date=2024-02
# - order_date=2024-03
```

**Why?**
- Monthly reports automated
- Historical comparison
- Trend analysis by period

## 🔧 Advanced Features

### Multiple Grouping Dimensions

VinRouge creates sheets for **all discovered dimensions**:

```csv
id,customer_id,region,status
1,C001,East,Active
2,C001,East,Pending
3,C002,West,Active
```

Result includes sheets for:
- `customer_id=C001`, `customer_id=C002`
- `region=East`, `region=West`
- `status=Active`, `status=Pending`

### Limit to Top Groups

To avoid creating too many sheets:
- **Maximum 20 sheets** per dimension
- **Top groups by record count** (largest first)
- **Dimensions with >50 groups** are skipped

Example: If you have 100 customers, only the top 20 by order count are exported as separate sheets.

### Multiple Files for Multiple Sources

When analyzing multiple files:

```bash
vinrouge analyze --csv customers.csv --csv orders.csv -f grouped-excel -o report
```

Creates:
- `report_customers.xlsx` (customers grouped)
- `report_orders.xlsx` (orders grouped)

Each file maintains its own grouping dimensions!

### Sheet Naming Rules

Excel has restrictions on sheet names:
- **Max 31 characters**
- **No special characters**: `:`, `\`, `/`, `?`, `*`, `[`, `]`

VinRouge automatically:
- Replaces forbidden characters with `-` or `()`
- Truncates long names to 28 chars + "..."
- Example: `very_long_dimension_name=very_long_value` → `very_long_dimension_name=...`

## 📊 Comparison: Regular vs Grouped Export

### Regular Excel Export (`-f excel`)

**One workbook with multiple sheets:**
- Summary
- Tables
- Relationships
- Workflows
- Reconciliation

**Best for:** Overview, analysis summary, stakeholder reports

### Grouped Excel Export (`-f grouped-excel`)

**One workbook with data split by categories:**
- Summary (list of dimensions)
- One sheet per group value (customer, region, status, etc.)

**Best for:** Filtered data, per-entity reports, distribution

## 🎯 When to Use Each

| Use Case | Export Type |
|----------|-------------|
| Executive summary | Regular Excel |
| Data analysis overview | Regular Excel |
| Reconciliation results | Regular Excel |
| Customer-specific data | **Grouped Excel** |
| Regional reports | **Grouped Excel** |
| Status-based views | **Grouped Excel** |
| Category breakdown | **Grouped Excel** |
| Distribution to teams | **Grouped Excel** |

## 🧪 Example Workflow

### Sales Analysis Example

```bash
# 1. Create test data
cat > sales.csv << EOF
id,customer_id,product,region,amount,status
1,C001,Laptop,East,1200,Shipped
2,C001,Mouse,East,25,Shipped
3,C002,Monitor,West,350,Pending
4,C003,Keyboard,East,80,Shipped
5,C002,Laptop,West,1200,Delivered
EOF

# 2. Run analysis with grouped export
vinrouge analyze --csv sales.csv -f grouped-excel -o sales_grouped.xlsx

# 3. Open in Excel
open sales_grouped.xlsx

# 4. See sheets:
#    - Summary
#    - customer_id=C001 (Laptop, Mouse)
#    - customer_id=C002 (Monitor, Laptop)
#    - customer_id=C003 (Keyboard)
#    - region=East (Laptop, Mouse, Keyboard)
#    - region=West (Monitor, Laptop)
#    - status=Shipped (Laptop, Mouse, Keyboard)
#    - status=Pending (Monitor)
#    - status=Delivered (Laptop)
```

## 💡 Pro Tips

### 1. Check Grouping Dimensions First

Before exporting, run analysis to see dimensions:

```bash
# Console output shows grouping dimensions
vinrouge analyze --csv data.csv -f console

# Look for "GROUPING ANALYSIS" section
# Shows which dimensions will be used for grouping
```

### 2. Clean Data for Better Grouping

Ensure consistent values:
- **Before**: "East", "east", "EAST", " East "
- **After**: "East" (consistent)

### 3. Combine with Regular Export

```bash
# Create both for complete analysis
vinrouge analyze --csv data.csv -f excel -o analysis.xlsx
vinrouge analyze --csv data.csv -f grouped-excel -o grouped.xlsx
```

### 4. Automate Daily Reports

```bash
#!/bin/bash
DATE=$(date +%Y%m%d)

# Export grouped data daily
vinrouge analyze \
  --csv /data/orders_${DATE}.csv \
  -f grouped-excel \
  -o reports/orders_grouped_${DATE}.xlsx

# Distribute to team
# ...
```

### 5. Use with Pivot Tables

Grouped data is perfect for pivot tables:
1. Export grouped data
2. Open sheet for your dimension
3. Insert Pivot Table
4. Analyze just that subset!

## ⚙️ Configuration

Currently configured limits (hardcoded):
- **Max sheets per dimension**: 20
- **Skip if groups > 50**: Yes
- **Sample size**: 1000 rows (for dimension detection)

Future: These will be configurable via command-line flags.

## 🚧 Limitations

Current limitations:
1. **Max 20 sheets per dimension** (avoid file size issues)
2. **Dimensions with >50 unique values** are skipped
3. **Sheet names limited to 31 characters** (Excel restriction)
4. **Only CSV and Excel sources** supported (no database yet)

## 🔮 Future Enhancements

Planned features:
- [ ] Custom dimension selection (`--group-by customer_id,status`)
- [ ] Configurable sheet limits
- [ ] Cross-dimension grouping (e.g., region × status)
- [ ] Summary statistics per sheet
- [ ] Conditional formatting by default
- [ ] Chart generation per group

## 📖 Summary

**Grouped Data Export** turns this:

```csv
id,customer,amount
1,C001,100
2,C002,200
3,C001,150
4,C003,300
```

Into an Excel file with sheets:
- **customer=C001** (rows 1, 3)
- **customer=C002** (row 2)
- **customer=C003** (row 4)

Perfect for distribution, filtering, and focused analysis! 🎯📊

---

**Try it now!**

```bash
vinrouge analyze --csv your_data.csv -f grouped-excel -o grouped_output.xlsx
```

Then press **5** and select **4** in TUI mode! 🚀
