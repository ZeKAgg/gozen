mod expression_not_assigned;
mod no_duplicate_branch;
mod no_self_comparison;
mod no_shadowed_variable;
mod no_shadowing_builtin;

pub use expression_not_assigned::ExpressionNotAssigned;
pub use no_duplicate_branch::NoDuplicateBranch;
pub use no_self_comparison::NoSelfComparison;
pub use no_shadowed_variable::NoShadowedVariable;
pub use no_shadowing_builtin::NoShadowingBuiltin;
