use crate::dsl::*;

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

#[test]
fn test_resolve_section() {
    let s = schema();
    let stmts = parse(r#"SECTION "Reconciliation" {
        ASSERT SUM(invoices.amount) = SUM(sub_ledger.balance)
        bad: SUM(invoices.nope)
    }"#).unwrap();
    let errs = resolve(&stmts, &s);
    assert_eq!(errs.len(), 1);
    assert!(matches!(&errs[0], ResolveError::UnknownColumn { column, .. } if column == "nope"));
}
