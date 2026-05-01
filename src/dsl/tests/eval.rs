use rust_decimal_macros::dec;
use crate::dsl::*;

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
