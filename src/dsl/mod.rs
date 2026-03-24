// dsl — Vin Rouge Audit DSL
// Lexer → Token stream → Parser → AST → Evaluator

mod ast;
mod datasource;
mod error;
pub mod eval;
mod lexer;
mod parser;
mod resolver;
mod token;
mod value;

// Re-export the public surface
pub use ast::{AggFunc, ArithOp, CmpOp, Expr, LogicOp, SampleMethod, SampleSize, Statement};
pub use datasource::{EvalDataSource, InMemoryDataSource};
pub use error::{ParseError, ParseResult};
pub use eval::{run_script, AssertResult, Evaluator, SampleResult, StatementResult};
pub use resolver::{resolve, ResolveError, Resolver, Schema};
pub use lexer::Lexer;
pub use parser::Parser;
pub use value::{parse_value, EvalError, EvalResult, Row, Value};

/// Parse a DSL string into a list of [`Statement`]s.
///
/// # Example
/// ```rust
/// let stmts = vinrouge::dsl::parse(
///     r#"
///     debtors_check: ASSERT "Debtors reconciliation"
///         SUM(invoices.amount) WHERE status = "open"
///         = debtors_control
///
///     sample_large: SAMPLE MUS invoices.amount 50
///         WHERE amount > 10000
///     "#
/// ).unwrap();
/// ```
pub fn parse(input: &str) -> ParseResult<Vec<Statement>> {
    let tokens = Lexer::new(input).tokenise()?;
    Parser::new(tokens).parse_script()
}

#[cfg(test)]
mod resolver_tests {
    use super::*;

    fn schema() -> Schema {
        let mut s = Schema::new();
        s.add_table("invoices", ["amount", "status", "id"]);
        s.add_table("sub_ledger", ["balance", "account"]);
        s
    }

    #[test]
    fn test_resolve_clean() {
        let s = schema();
        let stmts = parse("SUM(invoices.amount)").unwrap();
        assert!(resolve(&stmts, &s).is_empty());
    }

    #[test]
    fn test_resolve_unknown_table() {
        let s = schema();
        let stmts = parse("SUM(missing.amount)").unwrap();
        let errs = resolve(&stmts, &s);
        assert_eq!(errs.len(), 1);
        assert!(matches!(&errs[0], ResolveError::UnknownTable { table, .. } if table == "missing"));
    }

    #[test]
    fn test_resolve_unknown_column() {
        let s = schema();
        let stmts = parse("SUM(invoices.nope)").unwrap();
        let errs = resolve(&stmts, &s);
        assert_eq!(errs.len(), 1);
        assert!(matches!(&errs[0], ResolveError::UnknownColumn { column, .. } if column == "nope"));
    }

    #[test]
    fn test_resolve_bare_column_in_aggregate() {
        let s = schema();
        let stmts = parse("SUM(amount)").unwrap();
        let errs = resolve(&stmts, &s);
        assert_eq!(errs.len(), 1);
        assert!(matches!(&errs[0], ResolveError::BareColumnInAggregate(_)));
    }

    #[test]
    fn test_resolve_assert_both_sides() {
        let s = schema();
        let stmts = parse("ASSERT SUM(invoices.amount) = SUM(sub_ledger.balance)").unwrap();
        assert!(resolve(&stmts, &s).is_empty());
    }

    #[test]
    fn test_resolve_assert_bad_rhs() {
        let s = schema();
        let stmts = parse("ASSERT SUM(invoices.amount) = SUM(sub_ledger.nope)").unwrap();
        let errs = resolve(&stmts, &s);
        assert_eq!(errs.len(), 1);
        assert!(matches!(&errs[0], ResolveError::UnknownColumn { column, .. } if column == "nope"));
    }

    #[test]
    fn test_resolve_sample_valid() {
        let s = schema();
        let stmts = parse("SAMPLE MUS invoices.amount 50").unwrap();
        assert!(resolve(&stmts, &s).is_empty());
    }

    #[test]
    fn test_resolve_sample_unknown_table() {
        let s = schema();
        let stmts = parse("SAMPLE MUS ghost.amount 50").unwrap();
        let errs = resolve(&stmts, &s);
        assert!(errs.iter().any(|e| matches!(e, ResolveError::UnknownTable { table, .. } if table == "ghost")));
    }

    #[test]
    fn test_resolve_sample_unknown_column() {
        let s = schema();
        let stmts = parse("SAMPLE MUS invoices.nope 50").unwrap();
        let errs = resolve(&stmts, &s);
        assert!(errs.iter().any(|e| matches!(e, ResolveError::UnknownColumn { column, .. } if column == "nope")));
    }

    #[test]
    fn test_resolve_collects_all_errors() {
        let s = schema();
        // Two bad refs — both should be reported
        let stmts = parse(
            "SUM(bad_table.amount)\nSUM(invoices.bad_col)",
        ).unwrap();
        let errs = resolve(&stmts, &s);
        assert_eq!(errs.len(), 2);
    }

    #[test]
    fn test_resolve_filter_column() {
        let s = schema();
        let stmts = parse(r#"SUM(invoices.amount) WHERE invoices.status = "paid""#).unwrap();
        assert!(resolve(&stmts, &s).is_empty());
    }

    #[test]
    fn test_resolve_filter_unknown_column() {
        let s = schema();
        let stmts = parse(r#"SUM(invoices.amount) WHERE invoices.nope = "paid""#).unwrap();
        let errs = resolve(&stmts, &s);
        assert_eq!(errs.len(), 1);
        assert!(matches!(&errs[0], ResolveError::UnknownColumn { column, .. } if column == "nope"));
    }
}

#[cfg(test)]
mod eval_tests {
    use rust_decimal_macros::dec;

    use super::*;

    // ── helpers ───────────────────────────────────────────────────────────────

    fn ds_invoices() -> InMemoryDataSource {
        let mut ds = InMemoryDataSource::new();
        ds.insert_table(
            "invoices",
            vec![
                [("amount".into(), Value::Decimal(dec!(100))), ("status".into(), Value::Text("paid".into()))].into(),
                [("amount".into(), Value::Decimal(dec!(200))), ("status".into(), Value::Text("open".into()))].into(),
                [("amount".into(), Value::Decimal(dec!(300))), ("status".into(), Value::Text("paid".into()))].into(),
                [("amount".into(), Value::Decimal(dec!(400))), ("status".into(), Value::Text("open".into()))].into(),
                [("amount".into(), Value::Decimal(dec!(500))), ("status".into(), Value::Text("paid".into()))].into(),
            ],
        );
        ds
    }

    fn eval_str(input: &str, ds: &dyn EvalDataSource) -> StatementResult {
        let stmts = parse(input).expect("parse failed");
        run_script(&stmts, ds).into_iter().next().unwrap()
    }

    // ── scalar arithmetic ─────────────────────────────────────────────────────

    #[test]
    fn test_eval_arithmetic() {
        let ds = InMemoryDataSource::new();
        let r = eval_str("2 + 3 * 4", &ds);
        // 3*4=12, 12+2=14
        assert!(matches!(r, StatementResult::Value(s) if s == "14"));
    }

    #[test]
    fn test_eval_division() {
        let ds = InMemoryDataSource::new();
        let r = eval_str("10 / 4", &ds);
        // rust_decimal may produce "2.5" or "2.50…" — check numeric equality
        let StatementResult::Value(s) = r else { panic!("expected Value") };
        let result: rust_decimal::Decimal = s.parse().expect("not a decimal");
        assert_eq!(result, dec!(2.5));
    }

    #[test]
    fn test_eval_division_by_zero() {
        let ds = InMemoryDataSource::new();
        let r = eval_str("1 / 0", &ds);
        assert!(matches!(r, StatementResult::Error(s) if s.contains("division by zero")));
    }

    #[test]
    fn test_eval_unary_minus() {
        let ds = InMemoryDataSource::new();
        let r = eval_str("-5 + 3", &ds);
        assert!(matches!(r, StatementResult::Value(s) if s == "-2"));
    }

    // ── aggregates ────────────────────────────────────────────────────────────

    #[test]
    fn test_eval_sum() {
        let ds = ds_invoices();
        let r = eval_str("SUM(invoices.amount)", &ds);
        assert!(matches!(r, StatementResult::Value(s) if s == "1500"));
    }

    #[test]
    fn test_eval_sum_with_filter() {
        let ds = ds_invoices();
        let r = eval_str(r#"SUM(invoices.amount) WHERE status = "paid""#, &ds);
        assert!(matches!(r, StatementResult::Value(s) if s == "900"));
    }

    #[test]
    fn test_eval_count() {
        let ds = ds_invoices();
        let r = eval_str("COUNT(invoices.amount)", &ds);
        assert!(matches!(r, StatementResult::Value(s) if s == "5"));
    }

    #[test]
    fn test_eval_avg() {
        let ds = ds_invoices();
        let r = eval_str("AVG(invoices.amount)", &ds);
        assert!(matches!(r, StatementResult::Value(s) if s == "300"));
    }

    #[test]
    fn test_eval_min_max() {
        let ds = ds_invoices();
        let min = eval_str("MIN(invoices.amount)", &ds);
        let max = eval_str("MAX(invoices.amount)", &ds);
        assert!(matches!(min, StatementResult::Value(s) if s == "100"));
        assert!(matches!(max, StatementResult::Value(s) if s == "500"));
    }

    // ── assert ────────────────────────────────────────────────────────────────

    #[test]
    fn test_assert_pass() {
        let ds = ds_invoices();
        let r = eval_str("ASSERT SUM(invoices.amount) = 1500", &ds);
        let StatementResult::Assert(a) = r else { panic!("expected Assert") };
        assert!(a.passed);
        assert_eq!(a.op, "=");
    }

    #[test]
    fn test_assert_fail() {
        let ds = ds_invoices();
        let r = eval_str("ASSERT SUM(invoices.amount) = 999", &ds);
        let StatementResult::Assert(a) = r else { panic!("expected Assert") };
        assert!(!a.passed);
        assert_eq!(a.lhs_value, "1500");
        assert_eq!(a.rhs_value, "999");
    }

    #[test]
    fn test_assert_with_label() {
        let ds = ds_invoices();
        let r = eval_str(r#"ASSERT "Total check" SUM(invoices.amount) > 1000"#, &ds);
        let StatementResult::Assert(a) = r else { panic!("expected Assert") };
        assert!(a.passed);
        assert_eq!(a.label.as_deref(), Some("Total check"));
    }

    // ── sample ────────────────────────────────────────────────────────────────

    #[test]
    fn test_sample_random_count() {
        let ds = ds_invoices();
        let r = eval_str("SAMPLE RANDOM invoices.amount 3", &ds);
        let StatementResult::Sample(s) = r else { panic!("expected Sample") };
        assert_eq!(s.selected.len(), 3);
        assert_eq!(s.population_size, 5);
    }

    #[test]
    fn test_sample_percent() {
        let ds = ds_invoices();
        let r = eval_str("SAMPLE RANDOM invoices.amount 40%", &ds);
        let StatementResult::Sample(s) = r else { panic!("expected Sample") };
        // 40% of 5 = 2 (ceil)
        assert_eq!(s.selected.len(), 2);
    }

    #[test]
    fn test_sample_mus_count() {
        let ds = ds_invoices();
        let r = eval_str("SAMPLE MUS invoices.amount 3", &ds);
        let StatementResult::Sample(s) = r else { panic!("expected Sample") };
        assert_eq!(s.selected.len(), 3);
    }

    #[test]
    fn test_sample_systematic_count() {
        let ds = ds_invoices();
        let r = eval_str("SAMPLE SYSTEMATIC invoices.amount 3", &ds);
        let StatementResult::Sample(s) = r else { panic!("expected Sample") };
        assert_eq!(s.selected.len(), 3);
    }

    #[test]
    fn test_sample_stratified_count() {
        let ds = ds_invoices();
        let r = eval_str("SAMPLE STRATIFIED invoices.amount 4", &ds);
        let StatementResult::Sample(s) = r else { panic!("expected Sample") };
        assert_eq!(s.selected.len(), 4);
    }

    #[test]
    fn test_sample_with_filter() {
        let ds = ds_invoices();
        let r = eval_str(r#"SAMPLE RANDOM invoices.amount 2 WHERE status = "paid""#, &ds);
        let StatementResult::Sample(s) = r else { panic!("expected Sample") };
        assert_eq!(s.population_size, 3); // only paid rows
        assert_eq!(s.selected.len(), 2);
    }

    // ── errors ────────────────────────────────────────────────────────────────

    #[test]
    fn test_unknown_table() {
        let ds = InMemoryDataSource::new();
        let r = eval_str("SUM(missing.amount)", &ds);
        assert!(matches!(r, StatementResult::Error(s) if s.contains("unknown table")));
    }

    #[test]
    fn test_unknown_column() {
        let mut ds = InMemoryDataSource::new();
        ds.insert_table("t", vec![[("x".into(), Value::Decimal(dec!(1)))].into()]);
        let stmts = parse("t.missing").unwrap();
        let ev = Evaluator::new(&ds);
        let row: Row = [("x".into(), Value::Decimal(dec!(1)))].into();
        let err = ev.eval(&stmts[0].expr, &row).unwrap_err();
        assert!(matches!(err, EvalError::UnknownColumn(_)));
    }

    #[test]
    fn test_type_mismatch() {
        let ds = InMemoryDataSource::new();
        let r = eval_str(r#""hello" + 1"#, &ds);
        assert!(matches!(r, StatementResult::Error(s) if s.contains("type mismatch")));
    }

    // ── multi-statement script ────────────────────────────────────────────────

    #[test]
    fn test_run_script_multiple_statements() {
        let ds = ds_invoices();
        let stmts = parse(
            "SUM(invoices.amount)\nASSERT SUM(invoices.amount) = 1500",
        )
        .unwrap();
        let results = run_script(&stmts, &ds);
        assert_eq!(results.len(), 2);
        assert!(matches!(&results[0], StatementResult::Value(s) if s == "1500"));
        assert!(matches!(&results[1], StatementResult::Assert(a) if a.passed));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_simple_sum() {
        let stmts = parse("SUM(invoices.amount)").unwrap();
        assert_eq!(stmts.len(), 1);
        assert!(matches!(&stmts[0].expr, Expr::Aggregate { func: AggFunc::Sum, .. }));
    }

    #[test]
    fn test_sum_with_where() {
        let stmts = parse(r#"SUM(invoices.amount) WHERE status = "paid""#).unwrap();
        let Expr::Aggregate { filter, .. } = &stmts[0].expr else { panic!() };
        assert!(filter.is_some());
    }

    #[test]
    fn test_assert_equality() {
        let stmts = parse("ASSERT debtors_control = SUM(sub_ledger.balance)").unwrap();
        assert!(matches!(&stmts[0].expr, Expr::Assert { op: CmpOp::Eq, .. }));
    }

    #[test]
    fn test_assert_with_label_string() {
        let stmts = parse(r#"ASSERT "Debtors recon" debtors_control = SUM(sub_ledger.balance)"#).unwrap();
        let Expr::Assert { label, .. } = &stmts[0].expr else { panic!() };
        assert_eq!(label.as_deref(), Some("Debtors recon"));
    }

    #[test]
    fn test_sample_mus() {
        let stmts = parse("SAMPLE MUS invoices.amount 50").unwrap();
        let Expr::Sample { method, size, .. } = &stmts[0].expr else { panic!() };
        assert_eq!(*method, SampleMethod::Mus);
        assert_eq!(*size, SampleSize::Count(dec!(50)));
    }

    #[test]
    fn test_sample_percent() {
        let stmts = parse("SAMPLE RANDOM invoices.id 10%").unwrap();
        let Expr::Sample { size, .. } = &stmts[0].expr else { panic!() };
        assert_eq!(*size, SampleSize::Percent(dec!(10)));
    }

    #[test]
    fn test_arithmetic() {
        let stmts = parse("total_vat = net_sales * 0.15").unwrap();
        assert!(matches!(&stmts[0].expr, Expr::Compare { op: CmpOp::Eq, .. }));
    }

    #[test]
    fn test_between() {
        let stmts = parse("invoices.amount BETWEEN 1000 AND 50000").unwrap();
        assert!(matches!(&stmts[0].expr, Expr::Between { negated: false, .. }));
    }

    #[test]
    fn test_in_list() {
        let stmts = parse(r#"status IN ("paid", "approved", "posted")"#).unwrap();
        let Expr::InList { values, .. } = &stmts[0].expr else { panic!() };
        assert_eq!(values.len(), 3);
    }

    #[test]
    fn test_labeled_statement() {
        let stmts = parse("vat_check: SUM(vat.amount) WHERE period = 3").unwrap();
        assert_eq!(stmts[0].label.as_deref(), Some("vat_check"));
    }

    #[test]
    fn test_is_null() {
        let stmts = parse("invoices.approval IS NULL").unwrap();
        assert!(matches!(&stmts[0].expr, Expr::IsNull { negated: false, .. }));
    }

    #[test]
    fn test_is_not_null() {
        let stmts = parse("invoices.approval IS NOT NULL").unwrap();
        assert!(matches!(&stmts[0].expr, Expr::IsNull { negated: true, .. }));
    }

    #[test]
    fn test_line_comment() {
        let stmts = parse("-- this is a comment\nSUM(invoices.amount)").unwrap();
        assert_eq!(stmts.len(), 1);
    }

    #[test]
    fn test_error_unexpected_char() {
        let err = parse("SUM(invoices.amount) @").unwrap_err();
        assert!(err.message.contains("unexpected character"));
    }

    #[test]
    fn test_error_missing_paren() {
        let err = parse("SUM(invoices.amount").unwrap_err();
        assert!(err.message.contains("expected ')'"));
    }
}
