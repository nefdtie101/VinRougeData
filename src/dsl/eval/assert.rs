use crate::dsl::ast::{CmpOp, Expr};
use crate::dsl::value::{EvalResult, Row};

use super::result::AssertResult;
use super::Evaluator;

pub(super) fn cmp_op_display(op: &CmpOp) -> &'static str {
    match op {
        CmpOp::Eq    => "=",
        CmpOp::NotEq => "<>",
        CmpOp::Gt    => ">",
        CmpOp::Gte   => ">=",
        CmpOp::Lt    => "<",
        CmpOp::Lte   => "<=",
    }
}

impl<'ds> Evaluator<'ds> {
    pub fn eval_assert(
        &self,
        label: &Option<String>,
        lhs: &Expr,
        rhs: &Expr,
        op: &CmpOp,
    ) -> EvalResult<AssertResult> {
        let empty = Row::new();
        let lv = self.eval(lhs, &empty)?;
        let rv = self.eval(rhs, &empty)?;
        let passed = self.apply_cmp(op, &lv, &rv);
        Ok(AssertResult {
            label: label.clone(),
            passed,
            lhs_value: lv.to_string(),
            rhs_value: rv.to_string(),
            op: cmp_op_display(op).to_string(),
        })
    }
}
