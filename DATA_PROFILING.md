# Data Profiling & Pattern Detection

VinRouge now includes **intelligent data profiling** that analyzes the actual values in your spreadsheets and CSV files to find patterns, relationships, and workflow insights!

## 🎯 What It Does

Unlike just looking at column names and types, the data profiler:

- **Reads actual cell values** from your files
- **Detects patterns** in the data (sequences, categories, IDs)
- **Finds correlations** between columns
- **Discovers workflows** from data relationships
- **Identifies data quality** issues

## 📊 Types of Analysis

### 1. Column Profiling

For each column, VinRouge analyzes:

**Basic Statistics:**
- Total values
- Unique values count
- Null/empty cells
- Distinct ratio (uniqueness percentage)
- Top 10 most frequent values

**Pattern Detection:**
- ✅ **Sequential** - Data like 1, 2, 3, 4...
- ✅ **Unique Identifiers** - Order IDs, Customer IDs
- ✅ **Categories** - Status values, types, groups
- ✅ **Numeric** - All numbers
- ✅ **Boolean** - True/False, Yes/No, 0/1
- ✅ **Email Addresses** - Contains @ and .
- ✅ **URLs** - Starts with http:// or https://
- ✅ **Phone Numbers** - Detected by format
- ✅ **Date/Time** - Temporal data

### 2. Column Correlations

Detects relationships between columns:

**One-to-One (1:1)**
```
CustomerID → Email
1001 → john@example.com
1002 → jane@example.com
1003 → bob@example.com
```
*Each customer has exactly one email*

**One-to-Many (1:M)**
```
CustomerID → OrderID
1001 → 5001, 5002, 5003
1002 → 5004
1003 → 5005, 5006
```
*Each customer has multiple orders*

**Many-to-One (M:1)**
```
OrderID → Status
5001, 5002 → "Shipped"
5003, 5004 → "Pending"
5005 → "Delivered"
```
*Multiple orders share the same status*

**Functional Dependency**
```
Price → TaxAmount
100 → 10
200 → 20
50 → 5
```
*Tax appears calculated from price*

### 3. Workflow Pattern Detection

**Auto-Increment Sequences:**
```
OrderID: 1001, 1002, 1003, 1004...
```
→ "OrderID appears to be auto-incrementing"

**Status Flows:**
```
Status column contains: "New", "Processing", "Shipped", "Delivered"
```
→ "Status appears to track workflow states"

**Hierarchies:**
```
Category → Subcategory
Electronics → Phones
Electronics → Laptops
Furniture → Chairs
```
→ "Parent-child relationship detected"

**Time Sequences:**
```
OrderDate column shows sequential dates
```
→ "Time-based sequence detected"

## 🔍 Real-World Examples

### Example 1: E-Commerce Orders

**Your Data:**
```csv
OrderID, CustomerID, ProductID, Status, OrderDate, TotalAmount
1001, C001, P123, Shipped, 2024-01-15, 99.99
1002, C001, P456, Pending, 2024-01-16, 149.50
1003, C002, P123, Shipped, 2024-01-16, 99.99
```

**VinRouge Detects:**
- ✅ OrderID is **sequential** (auto-increment)
- ✅ CustomerID is a **category** (C001 repeats)
- ✅ **1:Many** - One customer has multiple orders
- ✅ **Many:1** - Multiple orders can have same status
- ✅ Status is a **workflow** - tracks order lifecycle
- ✅ OrderDate shows **time sequence pattern**
- ✅ ProductID **correlates** with TotalAmount (same product = same price)

### Example 2: Employee Database

**Your Data:**
```csv
EmployeeID, Name, DepartmentID, ManagerID, Salary, Status
E001, John Smith, D01, M01, 75000, Active
E002, Jane Doe, D01, M01, 82000, Active
E003, Bob Wilson, D02, M02, 68000, Inactive
```

**VinRouge Detects:**
- ✅ EmployeeID is **unique identifier**
- ✅ **Hierarchy** - ManagerID references EmployeeID
- ✅ **Many:1** - Multiple employees share same manager
- ✅ DepartmentID is **category** (low cardinality)
- ✅ Status is **workflow state** (Active/Inactive)
- ✅ Salary is **numeric** data

### Example 3: Sales Pipeline

**Your Data:**
```csv
LeadID, Source, Stage, ContactDate, Value, Owner
L001, Website, Qualified, 2024-01-10, 5000, Sales1
L001, Website, Proposal, 2024-01-15, 5000, Sales1
L001, Website, Won, 2024-01-20, 5000, Sales1
```

**VinRouge Detects:**
- ✅ LeadID **repeats** - tracking same lead over time
- ✅ Stage shows **status flow**: Qualified → Proposal → Won
- ✅ ContactDate is **sequential** for same lead
- ✅ **Temporal workflow** - stages progress over time
- ✅ Value remains **constant** (same lead)
- ✅ Source and Owner are **categories**

## 💡 Use Cases

### 1. Understanding Unknown Data

You receive a spreadsheet with no documentation:
```
VinRouge tells you:
- Column A is sequential IDs
- Column B correlates 1:Many with Column C
- Column D is a status field with 5 states
- Column E contains email addresses
```

### 2. Finding Data Quality Issues

```
VinRouge detects:
- "CustomerID should be unique but has duplicates"
- "OrderDate has gaps in sequence"
- "Status contains unexpected values: 'Shiiped' (typo)"
- "Email column has 15% non-email values"
```

### 3. Discovering Hidden Workflows

```
VinRouge reveals:
- Orders flow: New → Processing → Shipped → Delivered
- Leads progress: Cold → Warm → Hot → Closed
- Tickets follow: Open → In Progress → Resolved → Closed
```

### 4. Reverse Engineering Systems

Analyzing data from legacy systems:
```
VinRouge shows you:
- How entities relate to each other
- Which fields are auto-generated
- What business rules exist (price = quantity × unitprice)
- Workflow stages in the system
```

## 🎮 How to Use in VinRouge

### In TUI Mode:

1. Add your CSV/Excel file (uses file browser)
2. Run analysis (press `3`)
3. **View results with data insights!**

Results now include:
```
═══ DATA INSIGHTS ═══

Column Patterns:
• OrderID: Sequential, Unique Identifier
• Status: Category (4 distinct values)
• Email: Email Addresses (98% valid)

Correlations:
• CustomerID → OrderID (1:Many, 95% strength)
• ProductID → Price (Functional, 100% strength)

Workflows Detected:
• Status Flow: New → Processing → Shipped
• Auto-increment: OrderID
```

### In CLI Mode:

```bash
vinrouge analyze --csv orders.csv -f markdown -o report.md
```

The markdown report will include a new "Data Insights" section!

## 🔬 Technical Details

### Sampling Strategy

- Analyzes up to **10,000 rows** per file (configurable)
- Takes representative samples for large files
- Balances accuracy vs. performance

### Pattern Recognition

Uses deterministic rules:
- **No AI/ML** - Pure logic and statistics
- **No false positives** - High confidence thresholds
- **Explainable** - Clear reasoning for each detection

### Correlation Detection

- Builds value mappings between columns
- Calculates relationship strengths (0-100%)
- Identifies directionality (A → B vs. B → A)

### Performance

- Fast even on large files
- Streaming analysis where possible
- Minimal memory footprint

## 📈 Benefits

**For Data Analysts:**
- Quickly understand new datasets
- Find relationships you didn't know existed
- Validate data quality

**For Developers:**
- Reverse engineer database structures
- Document legacy systems
- Plan data migrations

**For Business Users:**
- Discover workflows in operational data
- Identify process bottlenecks
- Understand data lineage

## 🚀 Future Enhancements

Planned features:
- Statistical anomaly detection
- Trend analysis over time
- Data quality scoring
- Suggested data cleaning steps
- Export data dictionary

## 📊 Example Output

**CSV File: sales_data.csv**

```
=== DATA PROFILE ===

Table: sales_data
Rows analyzed: 10,000

Column: order_id
├─ Pattern: Sequential, Unique Identifier
├─ Total: 10,000
├─ Unique: 10,000 (100%)
└─ Top value: (all unique)

Column: customer_id
├─ Pattern: Category
├─ Total: 10,000
├─ Unique: 2,547 (25%)
└─ Top values: C1001 (15), C1002 (12), C1003 (11)

Column: status
├─ Pattern: Category, Workflow State
├─ Total: 10,000
├─ Unique: 4 (0.04%)
└─ States: Pending, Processing, Shipped, Delivered

=== CORRELATIONS ===

customer_id → order_id
├─ Type: One-to-Many
├─ Strength: 100%
└─ Description: Each customer has multiple orders

product_id → price
├─ Type: Functional
├─ Strength: 100%
└─ Description: Price appears derived from product

=== WORKFLOWS ===

Status Flow: order_status
├─ States: Pending → Processing → Shipped → Delivered
└─ Description: Order lifecycle workflow detected

Auto-Increment: order_id
└─ Description: Sequential numbering pattern
```

---

**Now VinRouge doesn't just look at your data structure - it understands the meaning IN your data!** 🎯📊