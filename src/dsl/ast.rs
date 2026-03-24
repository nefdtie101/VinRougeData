use rust_decimal::Decimal;

/// Aggregate functions
#[derive(Debug, PartialEq, Clone)]
pub enum AggFunc { Sum, Avg, Count, Min, Max }

/// Sampling methods
#[derive(Debug, PartialEq, Clone)]
pub enum SampleMethod { Mus, Random, Systematic, Stratified }

/// Sample size — fixed count or percentage
#[derive(Debug, PartialEq, Clone)]
pub enum SampleSize {
    Count(Decimal),
    Percent(Decimal),
}

/// Binary arithmetic operators
#[derive(Debug, PartialEq, Clone)]
pub enum ArithOp { Add, Sub, Mul, Div }

/// Comparison operators
#[derive(Debug, PartialEq, Clone)]
pub enum CmpOp { Eq, NotEq, Gt, Gte, Lt, Lte }

/// Logical operators
#[derive(Debug, PartialEq, Clone)]
pub enum LogicOp { And, Or }

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

    /// Aggregate  e.g.  SUM(invoices.amount)  or  COUNT(invoices.id) WHERE status = "paid"
    Aggregate {
        func: AggFunc,
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
    IsNull {
        expr: Box<Expr>,
        negated: bool,
    },

    /// ASSERT  e.g.  ASSERT debtors_control = SUM(sub_ledger)
    Assert {
        label: Option<String>,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        op: CmpOp,
    },

    /// SAMPLE  e.g.  SAMPLE MUS invoices.amount 50 WHERE amount > 0
    Sample {
        method: SampleMethod,
        population: String,    // table name
        value_column: String,  // column for MUS weighting
        size: SampleSize,
        filter: Option<Box<Expr>>,
    },
}

/// A top-level statement — either an expression or a named assertion
#[derive(Debug, PartialEq, Clone)]
pub struct Statement {
    pub label: Option<String>, // optional "label: expr"
    pub expr: Expr,
}
