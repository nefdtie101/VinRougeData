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

#[test]
fn test_parse_chart_simple() {
    let stmts = parse("CHART bar SUM(invoices.amount) BY invoices.status").unwrap();
    assert_eq!(stmts.len(), 1);
    let Expr::Chart { chart_type, aggregate, dimension } = &stmts[0].expr else { panic!("expected Chart") };
    assert_eq!(chart_type, "bar");
    assert!(matches!(aggregate.as_ref(), Expr::Aggregate { func: AggFunc::Sum, .. }));
    assert!(matches!(dimension.as_ref(), Expr::ColumnRef(s) if s == "invoices.status"));
}

#[test]
fn test_parse_chart_with_label() {
    let stmts = parse("revenue: CHART pie SUM(invoices.amount) BY invoices.category").unwrap();
    assert_eq!(stmts[0].label.as_deref(), Some("revenue"));
    let Expr::Chart { chart_type, .. } = &stmts[0].expr else { panic!("expected Chart") };
    assert_eq!(chart_type, "pie");
}

#[test]
fn test_parse_screen_block() {
    let stmts = parse(r#"SCREEN "Dashboard" {
        revenue: CHART bar SUM(invoices.amount) BY invoices.status
        breakdown: CHART pie COUNT(invoices.id) BY invoices.category
    }"#).unwrap();
    assert_eq!(stmts.len(), 1);
    let Expr::Screen { title, charts } = &stmts[0].expr else { panic!("expected Screen") };
    assert_eq!(title, "Dashboard");
    assert_eq!(charts.len(), 2);
    assert_eq!(charts[0].label.as_deref(), Some("revenue"));
    assert_eq!(charts[0].chart_type, "bar");
    assert_eq!(charts[1].label.as_deref(), Some("breakdown"));
    assert_eq!(charts[1].chart_type, "pie");
}

#[test]
fn test_parse_section_block() {
    let stmts = parse(r#"SECTION "Reconciliation" {
        acb_only: ASSERT SUM(invoices.amount) = 1500
        total: SUM(invoices.amount)
    }"#).unwrap();
    assert_eq!(stmts.len(), 1);
    let Expr::Section { title, statements } = &stmts[0].expr else { panic!("expected Section") };
    assert_eq!(title, "Reconciliation");
    assert_eq!(statements.len(), 2);
    assert_eq!(statements[0].label.as_deref(), Some("acb_only"));
    assert!(matches!(&statements[0].expr, Expr::Assert { .. }));
    assert_eq!(statements[1].label.as_deref(), Some("total"));
    assert!(matches!(&statements[1].expr, Expr::Aggregate { .. }));
}

#[test]
fn test_parse_nested_section() {
    let stmts = parse(r#"SECTION "Outer" {
        SECTION "Inner" {
            ASSERT SUM(invoices.amount) = 1500
        }
    }"#).unwrap();
    assert_eq!(stmts.len(), 1);
    let Expr::Section { title, statements } = &stmts[0].expr else { panic!("expected Section") };
    assert_eq!(title, "Outer");
    assert_eq!(statements.len(), 1);
    let Expr::Section { title: inner_title, .. } = &statements[0].expr else { panic!("expected nested Section") };
    assert_eq!(inner_title, "Inner");
}
