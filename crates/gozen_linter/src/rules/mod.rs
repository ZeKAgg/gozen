use crate::rule::{ProjectRule, Rule};

pub mod complexity;
pub mod correctness;
pub mod performance;
pub mod project;
pub mod style;
pub mod suspicious;

/// Check whether a tree-sitter node kind represents a block / body container.
/// Shared helper used by multiple rules to avoid inconsistent ad-hoc checks.
pub fn is_block_node(kind: &str) -> bool {
    matches!(
        kind,
        "body" | "block" | "compound_statement" | "statement_list"
    )
}

pub fn all_rules() -> Vec<Box<dyn Rule>> {
    vec![
        // Correctness
        Box::new(correctness::NoUnusedVariables),
        Box::new(correctness::NoUnreachableCode),
        Box::new(correctness::InvalidPreloadPath),
        Box::new(correctness::NoOnreadyWithExport),
        Box::new(correctness::NoSelfAssignment),
        Box::new(correctness::DuplicateDictionaryKey),
        Box::new(correctness::NoAccessAfterFree),
        Box::new(correctness::NoUnusedParameter),
        Box::new(correctness::NoDeprecatedApi),
        Box::new(correctness::SuperReadyFirst),
        Box::new(correctness::NoDeprecatedSyntax),
        Box::new(correctness::NoStringSignalConnect),
        Box::new(correctness::UnnecessaryPass),
        Box::new(correctness::DuplicatedLoad),
        // Style
        Box::new(style::NamingConvention),
        Box::new(style::NoUntypedDeclaration),
        Box::new(style::BooleanOperators),
        Box::new(style::LineLength),
        Box::new(style::CommentSpacing),
        Box::new(style::ExportTypeHint),
        Box::new(style::NoBoolComparison),
        Box::new(style::FileNaming),
        Box::new(style::SignalParameterTypes),
        Box::new(style::PreferPreload),
        Box::new(style::ClassDefinitionsOrder),
        Box::new(style::NoUnnecessaryElse),
        Box::new(style::FunctionArgumentsNumber),
        Box::new(style::CognitiveComplexity),
        Box::new(style::CyclomaticComplexity),
        // Performance
        Box::new(performance::NoExpensiveProcess),
        Box::new(performance::NoStringConcatLoop),
        Box::new(performance::NoPreloadInLoop),
        Box::new(performance::NoAddChildInProcess),
        Box::new(performance::NoRepeatedGroupLookup),
        Box::new(performance::NoLoopAllocation),
        // Suspicious
        Box::new(suspicious::NoShadowedVariable),
        Box::new(suspicious::NoDuplicateBranch),
        Box::new(suspicious::NoSelfComparison),
        Box::new(suspicious::NoShadowingBuiltin),
        Box::new(suspicious::ExpressionNotAssigned),
    ]
}

pub fn all_project_rules() -> Vec<Box<dyn ProjectRule>> {
    project::all_project_rules()
}
