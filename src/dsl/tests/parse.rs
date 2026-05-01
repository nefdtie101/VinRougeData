use rust_decimal_macros::dec;
use crate::dsl::*;

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
