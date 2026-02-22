mod avoid_discard;
mod code_order;
mod cognitive_complexity;
mod comment_spacing;
mod complexity;
mod cyclomatic_complexity;
mod float_literal_style;
mod invalid_render_mode;
mod invalid_shader_type;
mod missing_shader_type;
mod naming_convention;
mod one_statement_per_line;
mod precision_hints;
mod uninitialized_variable;
mod unused_function;
mod unused_uniform;
mod unused_varying;

use crate::shader_rule::ShaderRule;

pub fn all_shader_rules() -> Vec<Box<dyn ShaderRule>> {
    vec![
        // Correctness
        Box::new(missing_shader_type::MissingShaderType),
        Box::new(invalid_shader_type::InvalidShaderType),
        Box::new(uninitialized_variable::UninitializedVariable),
        Box::new(unused_uniform::UnusedUniform),
        Box::new(unused_varying::UnusedVarying),
        Box::new(unused_function::UnusedFunction),
        Box::new(invalid_render_mode::InvalidRenderMode),
        // Style
        Box::new(naming_convention::ShaderNamingConvention),
        Box::new(float_literal_style::FloatLiteralStyle),
        Box::new(comment_spacing::ShaderCommentSpacing),
        Box::new(code_order::CodeOrder),
        Box::new(one_statement_per_line::OneStatementPerLine),
        Box::new(cognitive_complexity::CognitiveComplexity),
        Box::new(cyclomatic_complexity::CyclomaticComplexity),
        // Performance
        Box::new(avoid_discard::AvoidDiscard),
        Box::new(precision_hints::PrecisionHints),
    ]
}
