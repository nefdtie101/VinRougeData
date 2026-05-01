mod errors;
mod resolve;
mod schema;

pub use errors::ResolveError;
pub use resolve::Resolver;
pub use schema::Schema;

use crate::dsl::ast::Statement;

/// Resolve all column references in `statements` against `schema`.
///
/// Returns an empty `Vec` if everything is valid.
pub fn resolve(statements: &[Statement], schema: &Schema) -> Vec<ResolveError> {
    Resolver::new(schema).resolve(statements)
}
