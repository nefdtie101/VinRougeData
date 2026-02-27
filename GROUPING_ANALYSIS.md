# Grouping Analysis - Find All Ways to Analyze Your Data

VinRouge automatically discovers **ALL possible ways to group and analyze** your data! It finds every dimension, hierarchy, and aggregation pattern hidden in your spreadsheets.

## 🎯 The Problem It Solves

When you get a dataset, you might wonder:
- "What are all the ways I can group this data?"
- "Can I analyze by customer? By date? By product?"
- "What hierarchies exist? (Year → Month → Day?)"
- "What aggregations make sense?"

**VinRouge answers all these questions automatically!**

## 🔍 What It Discovers

### 1. Grouping Dimensions

Every column that can meaningfully group your data:

**Temporal Dimensions:**
```
OrderDate: 365 groups (daily grouping)
├─ Can group 10,000 records into 365 groups
├─ Average: 27 records per day
└─ Time-based analysis possible: trends, seasonality
```

**Identifier Dimensions:**
```
CustomerID: 2,547 groups
├─ Can group 10,000 records by customer
├─ Uneven distribution: top customer has 45 orders
└─ Analyze per customer: lifetime value, behavior patterns
```

**Categorical Dimensions:**
```
Status: 4 groups (New, Processing, Shipped, Delivered)
├─ Even distribution: ~2,500 records per status
└─ Compare across status: performance metrics
```

### 2. Hierarchical Groupings

Multi-level drill-down paths:

**Temporal Hierarchy:**
```
Year (3 groups)
  └─ Month (36 groups)
      └─ Day (1,095 groups)

Analysis: Start with yearly view, drill to monthly, then daily
```

**Categorical Hierarchy:**
```
Category (5 groups: Electronics, Furniture...)
  └─ Subcategory (23 groups: Phones, Laptops...)
      └─ Product (156 groups)

Analysis: Compare categories, then drill into subcategories
```

**Geographic Hierarchy:**
```
Country (15 groups)
  └─ State (52 groups)
      └─ City (347 groups)

Analysis: Regional performance from country down to city
```

### 3. Suggested Analyses

Actionable insights on how to analyze your data:

```
📊 Group by OrderDate: Analyze trends over time
   └─ Sum Amount per OrderDate (daily revenue)

👤 Group by CustomerID: Analyze per customer
   └─ Count orders per customer (frequency analysis)

📂 Group by Status: Compare categories
   └─ Average Amount by Status

🗺️  Group by Region: Regional analysis
   └─ Sum sales by Region (geographic performance)

🔀 Multi-dimensional analysis:
   └─ Group by CustomerID and Status (customer lifecycle)
   └─ Group by OrderDate and Region (temporal + geographic)
```

## 📊 Real-World Examples

### Example 1: E-Commerce Orders

**Your Data:**
```csv
OrderID,OrderDate,CustomerID,ProductID,Category,Amount,Status,Region
1001,2024-01-15,C001,P123,Electronics,999,Shipped,West
1002,2024-01-15,C001,P456,Furniture,1495,Pending,West
1003,2024-01-16,C002,P123,Electronics,999,Shipped,East
...10,000 more rows
```

**VinRouge Discovers:**

**7 Grouping Dimensions:**

1. **OrderDate** (Temporal)
   - 365 unique days
   - 27 records per day average
   - Suggestion: Daily/weekly/monthly trends

2. **CustomerID** (Identifier)
   - 2,547 unique customers
   - Range: 1-45 orders per customer
   - Suggestion: Customer segmentation, lifetime value

3. **ProductID** (Identifier)
   - 156 unique products
   - Range: 2-287 orders per product
   - Suggestion: Product popularity ranking

4. **Category** (Categorical)
   - 5 groups: Electronics, Furniture, Clothing, Books, Sports
   - Even distribution
   - Suggestion: Category performance comparison

5. **Status** (Categorical - Workflow)
   - 4 groups: New, Processing, Shipped, Delivered
   - Workflow states detected
   - Suggestion: Funnel analysis

6. **Region** (Geographic)
   - 4 groups: West, East, North, South
   - Uneven distribution (West: 4,200, East: 3,100...)
   - Suggestion: Regional sales analysis

7. **Amount** (Numeric - Can be binned)
   - Can create ranges: $0-$50, $50-$100, etc.
   - Suggestion: Price point analysis

**3 Hierarchies Detected:**

```
Hierarchy 1: Time-based
Year (2024, 2023)
  └─ Month (Jan, Feb, Mar...)
      └─ Day (1, 2, 3...)

Hierarchy 2: Geographic
Region → (could drill to State → City if data existed)

Hierarchy 3: Product
Category → ProductID
```

**Suggested Analyses (15 total):**

```
📊 TEMPORAL ANALYSIS
├─ Group by OrderDate: Daily sales trends
├─ Sum Amount by Month: Monthly revenue
└─ Count orders by Day of Week: Weekly patterns

👤 CUSTOMER ANALYSIS
├─ Group by CustomerID: Customer lifetime value
├─ Count orders per Customer: Frequency segmentation
└─ Average Amount per Customer: Spending patterns

📦 PRODUCT ANALYSIS
├─ Group by ProductID: Product performance
├─ Sum Amount by Category: Category revenue
└─ Count orders by Product: Popularity ranking

🗺️  GEOGRAPHIC ANALYSIS
├─ Group by Region: Regional performance
└─ Sum Amount by Region: Sales by geography

🔄 WORKFLOW ANALYSIS
├─ Group by Status: Funnel analysis
└─ Count records by Status: Pipeline health

🔀 MULTI-DIMENSIONAL
├─ CustomerID × Category: What do customers buy?
├─ Region × Category: Regional preferences
├─ OrderDate × Status: Time-based workflow
└─ CustomerID × OrderDate: Customer activity over time
```

### Example 2: Employee Database

**Your Data:**
```csv
EmployeeID,Name,DepartmentID,Department,TeamID,Team,ManagerID,HireDate,Salary,Status
E001,John,D01,Engineering,T01,Backend,M01,2020-05-15,75000,Active
E002,Jane,D01,Engineering,T02,Frontend,M01,2021-03-20,82000,Active
E003,Bob,D02,Sales,T03,Enterprise,M02,2019-11-10,68000,Active
```

**VinRouge Discovers:**

**Grouping Dimensions:**
- DepartmentID (5 groups) - Categorical
- TeamID (12 groups) - Categorical/Hierarchical
- ManagerID (8 groups) - Identifier
- HireDate (847 days) - Temporal
- Status (3 groups: Active, Leave, Terminated) - Categorical

**Hierarchies:**
```
Organization:
Department → Team → Employee

Time:
Year → Month → HireDate

Management:
ManagerID → EmployeeID (who reports to whom)
```

**Suggested Analyses:**
```
📊 Group by Department: Compare department sizes, salaries
📊 Group by Team: Team performance metrics
👤 Group by ManagerID: Team sizes, span of control
📅 Group by HireDate: Hiring trends, tenure analysis
📂 Group by Status: Workforce composition

🔀 Multi-dimensional:
├─ Department × HireDate: Department growth over time
├─ Manager × Team: Team distribution
└─ Status × Department: Attrition by department
```

### Example 3: Sales Pipeline

**Your Data:**
```csv
LeadID,ContactDate,Source,Stage,Value,Owner,Industry,Region
L001,2024-01-10,Website,Qualified,5000,Sales1,Tech,West
L001,2024-01-15,Website,Proposal,5000,Sales1,Tech,West
L001,2024-01-20,Website,Won,5000,Sales1,Tech,West
L002,2024-01-11,Referral,Qualified,8000,Sales2,Finance,East
```

**VinRouge Discovers:**

**Grouping Dimensions:**
- LeadID (3,421 unique) - Identifier (tracks lead journey)
- ContactDate (156 days) - Temporal
- Source (5: Website, Referral, Email, Phone, Event) - Categorical
- Stage (6: Cold, Warm, Qualified, Proposal, Negotiation, Won/Lost) - Workflow
- Owner (8 sales reps) - Identifier
- Industry (12 industries) - Categorical
- Region (4 regions) - Geographic

**Hierarchies:**
```
Sales Funnel:
Stage progression (Workflow states detected)

Time:
Year → Month → Week → Day

Geographic:
Region (→ could drill to State if available)
```

**Suggested Analyses:**
```
🔄 FUNNEL ANALYSIS
├─ Group by Stage: Pipeline health
├─ Count leads by Stage: Conversion funnel
└─ Average Value by Stage: Deal size at each stage

📊 PERFORMANCE ANALYSIS
├─ Group by Owner: Sales rep performance
├─ Sum Value by Owner: Revenue by rep
└─ Win rate by Owner: Effectiveness

📅 TREND ANALYSIS
├─ Group by ContactDate: Lead flow over time
├─ Wins by Month: Monthly targets
└─ Source effectiveness over time

🎯 SEGMENTATION
├─ Group by Source: Channel effectiveness
├─ Group by Industry: Vertical performance
├─ Group by Region: Geographic patterns

🔀 ADVANCED ANALYSIS
├─ Source × Stage: Which channels convert best?
├─ Industry × Stage: Which industries close faster?
├─ Owner × Source: Rep specialization by channel
└─ ContactDate × Stage: Typical sales cycle length
```

## 💡 Key Insights You Get

### Distribution Analysis
```
"CustomerID shows uneven distribution:
 - Top customer: 45 orders
 - Average: 4 orders
 - This suggests power users vs. occasional buyers"
```

### Cardinality Insights
```
"Status has low cardinality (4 values)
 → Good for comparison analysis

ProductID has medium cardinality (156 values)
 → Good for ranking/top-N analysis

Email has high cardinality (unique)
 → Not useful for grouping"
```

### Hierarchy Detection
```
"Category contains Subcategory
 → 2-level drill-down possible
 → Start broad, drill into details"
```

### Suggested Metrics
```
"For numeric columns found:
 - Amount: Sum, Average, Min/Max
 - Quantity: Sum, Average

Best groupings:
 - Sum Amount by Category (revenue by category)
 - Average Quantity by Product (typical order size)
 - Count orders by Customer (purchase frequency)"
```

## 🎮 How to Use

### In TUI:
1. Add your CSV/Excel file
2. Run analysis
3. **See "Grouping Dimensions" section in results!**

### Example Output:
```
═══ GROUPING ANALYSIS ═══

Found 7 ways to group your data:

📅 OrderDate (Temporal)
   ├─ 365 unique values
   ├─ 27 records per group (avg)
   └─ Suggestion: Analyze daily/weekly trends

👤 CustomerID (Identifier)
   ├─ 2,547 unique values
   ├─ 4 records per group (avg)
   └─ Suggestion: Customer lifetime value analysis

📂 Status (Categorical)
   ├─ 4 unique values
   ├─ 2,500 records per group (avg)
   └─ Suggestion: Compare across statuses

═══ HIERARCHIES ═══

🔺 Time-based hierarchy:
   Year → Month → Day

🔺 Category hierarchy:
   Category → Product

═══ SUGGESTED ANALYSES ═══

📊 Sum Amount by OrderDate (revenue over time)
👤 Count orders per CustomerID (frequency)
📂 Average Amount by Status (value by stage)
🗺️  Sum Amount by Region (geographic performance)

🔀 Multi-dimensional:
   CustomerID × Status (customer lifecycle)
   OrderDate × Region (time + geography)
```

## 🚀 Benefits

**For Data Analysts:**
- Instantly see all analysis dimensions
- Don't miss important groupings
- Get suggested analyses

**For Business Users:**
- Understand data without SQL
- See all possible reports
- Get actionable insights

**For Developers:**
- Document data structure
- Design reporting features
- Plan database optimizations

## 🎯 Technical Details

**How It Works:**
1. Scans each column for unique values
2. Filters columns suitable for grouping (2-1000 unique values)
3. Classifies dimension types (Temporal, Categorical, etc.)
4. Calculates group statistics (min, max, avg records per group)
5. Detects hierarchical relationships
6. Suggests meaningful analyses

**Performance:**
- Analyzes up to 10,000 rows
- Fast even on large files
- Smart sampling for huge datasets

**Deterministic:**
- No AI/ML needed
- Pure logic and statistics
- Consistent results

---

**Now you'll NEVER miss an important way to analyze your data!** 📊✨

VinRouge shows you EVERY dimension, EVERY hierarchy, and EVERY useful grouping automatically!
