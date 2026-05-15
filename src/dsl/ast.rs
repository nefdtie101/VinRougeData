use rust_decimal::Decimal;

/// Aggregate functions
#[derive(Debug, PartialEq, Clone)]
pub enum AggFunc {
    Sum,
    Avg,
    Count,
    Min,
    Max,
}

/// String scalar functions
#[derive(Debug, PartialEq, Clone)]
pub enum StringFunc {
    Upper,
    Lower,
    Trim,
    Length,
}

/// Math scalar functions
#[derive(Debug, PartialEq, Clone)]
pub enum MathFunc {
    Abs,
    Round,
}

/// Sampling methods
#[derive(Debug, PartialEq, Clone)]
pub enum SampleMethod {
    Mus,
    Random,
    Systematic,
    Stratified,
}

/// Sample size — fixed count or percentage
#[derive(Debug, PartialEq, Clone)]
pub enum SampleSize {
    Count(Decimal),
    Percent(Decimal),
}

/// Binary arithmetic operators
#[derive(Debug, PartialEq, Clone)]
pub enum ArithOp {
    Add,
    Sub,
    Mul,
    Div,
}

/// Comparison operators
#[derive(Debug, PartialEq, Clone)]
pub enum CmpOp {
    Eq,
    NotEq,
    Gt,
    Gte,
    Lt,
    Lte,
}

/// Logical operators
#[derive(Debug, PartialEq, Clone)]
pub enum LogicOp {
    And,
    Or,
}

/// Full expression tree
#[derive(Debug, PartialEq, Clone)]
pub enum Expr {
    /// Numeric literal  e.g.  42.50
    Number(Decimal),

    /// Boolean literal
    Bool(bool),

    /// String literal  e.g.  "ZAR"
    Str(String),

    /// Null literal
    Null,

    /// Column or variable reference  e.g.  invoices.amount  or  total
    ColumnRef(String),

    /// Arithmetic  e.g.  a + b
    BinOp {
        op: ArithOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },

    /// Aggregate  e.g.  SUM(invoices.amount)  or  COUNT(DISTINCT invoices.id) WHERE status = "paid"
    Aggregate {
        func: AggFunc,
        distinct: bool,
        expr: Box<Expr>,
        filter: Option<Box<Expr>>,
    },

    /// Comparison  e.g.  amount > 0
    Compare {
        op: CmpOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },

    /// Logical  e.g.  status = "paid" AND amount > 0
    Logical {
        op: LogicOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },

    /// NOT expr
    Not(Box<Expr>),

    /// IN list  e.g.  status IN ("paid", "approved")
    InList {
        expr: Box<Expr>,
        values: Vec<Expr>,
        negated: bool,
    },

    /// BETWEEN  e.g.  amount BETWEEN 1000 AND 50000
    Between {
        expr: Box<Expr>,
        low: Box<Expr>,
        high: Box<Expr>,
        negated: bool,
    },

    /// IS NULL / IS NOT NULL
    IsNull { expr: Box<Expr>, negated: bool },

    /// LIKE pattern match  e.g.  table.col LIKE "INV-%"
    Like {
        expr: Box<Expr>,
        pattern: Box<Expr>,
        negated: bool,
    },

    /// String function  e.g.  UPPER(table.col)
    StringFn { func: StringFunc, expr: Box<Expr> },

    /// SUBSTR() extraction  e.g.  SUBSTR(table.col, 1, 3)
    SubStr {
        expr: Box<Expr>,
        start: Box<Expr>,
        length: Option<Box<Expr>>,
    },

    /// CONCAT() concatenation  e.g.  CONCAT("20", SUBSTR(table.col, 1, 2))
    Concat { exprs: Vec<Box<Expr>> },

    /// DATE() normalisation  e.g.  DATE(table.col) >= DATE("2024-01-01")
    DateFn { expr: Box<Expr> },

    /// CASE WHEN cond THEN val … ELSE default END
    Case {
        branches: Vec<(Box<Expr>, Box<Expr>)>,
        else_expr: Option<Box<Expr>>,
    },

    /// COALESCE(a, b, …) — first non-null value
    Coalesce { exprs: Vec<Box<Expr>> },

    /// NULLIF(a, b) — returns NULL when a = b, else a
    NullIf { expr: Box<Expr>, compare: Box<Expr> },

    /// Math function  e.g.  ABS(table.col)  or  ROUND(table.col, 2)
    MathFn {
        func: MathFunc,
        expr: Box<Expr>,
        scale: Option<Box<Expr>>,
    },

    /// IS_BLANK(col) — true when value is NULL or the empty string
    IsBlank { expr: Box<Expr>, negated: bool },

    /// IS_NUMERIC(col) — true when value can be parsed as a decimal number
    IsNumeric { expr: Box<Expr>, negated: bool },

    /// IS_DATE(col) — true when value matches a recognisable date format
    IsDate { expr: Box<Expr>, negated: bool },

    /// DUPLICATED(col1, col2, …) — true when the composite key formed by the
    /// given columns occurs more than once in the underlying table
    Duplicated { exprs: Vec<Box<Expr>> },

    /// SA_ID_VALID(col) — true when value is a valid 13-digit South African
    /// ID number (passes Luhn checksum)
    SaIdValid { expr: Box<Expr> },

    /// col NOT IN table.column — cross-table membership test
    InTableCol {
        expr: Box<Expr>,
        table: String,
        column: String,
        negated: bool,
    },

    /// RELATION table1.col -> table2.col — declarative FK mapping (metadata only)
    RelationDecl {
        from_table: String,
        from_col: String,
        to_table: String,
        to_col: String,
    },

    /// ASSERT  e.g.  ASSERT debtors_control = SUM(sub_ledger)
    Assert {
        label: Option<String>,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        op: CmpOp,
        show_failures: bool,
    },

    /// SAMPLE  e.g.  SAMPLE MUS invoices.amount 50 WHERE amount > 0
    Sample {
        method: SampleMethod,
        population: String,   // table name
        value_column: String, // column for MUS weighting
        size: SampleSize,
        filter: Option<Box<Expr>>,
    },

    /// CHART  e.g.  CHART bar SUM(invoices.amount) BY invoices.status
    Chart {
        chart_type: String,
        aggregate: Box<Expr>,
        dimension: Box<Expr>,
    },

    /// SECTION  e.g.  SECTION "Reconciliation" { ASSERT ... CHART ... }
    Section {
        title: String,
        statements: Vec<Statement>,
    },

    /// SCHEMA — prints the names and columns of every imported table
    Schema,

    /// SHOW ROWS FROM table [WHERE condition] — display matching rows as a table
    ShowRows {
        table: String,
        filter: Option<Box<Expr>>,
    },

    /// CSS "..." — inject custom CSS into the result output
    Css {
        styles: String,
    },
}

/// A top-level statement — either an expression or a named assertion
#[derive(Debug, PartialEq, Clone)]
pub struct Statement {
    pub label: Option<String>, // optional "label: expr"
    pub expr: Expr,
}
