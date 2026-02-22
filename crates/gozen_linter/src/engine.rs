use gozen_config::{LinterConfig, RuleSeverity, ShaderConfig};
use gozen_diagnostics::{Diagnostic, Severity};
use gozen_parser::Tree;
use gozen_project::ProjectGraph;

use crate::context::LintContext;
use crate::rule::{ProjectRule, Rule};
use crate::rules;
use crate::rules::style::file_naming;
use crate::shader_rule::ShaderRule;
use crate::shader_rules;

pub struct LintEngine {
    rules: Vec<Box<dyn Rule>>,
    project_rules: Vec<Box<dyn ProjectRule>>,
    shader_rules: Vec<Box<dyn ShaderRule>>,
    config: LinterConfig,
    shader_config: ShaderConfig,
}

fn recommended_severity(rule_id: &str) -> Severity {
    match rule_id {
        // Correctness
        "correctness/noUnusedVariables" => Severity::Warning,
        "correctness/noUnreachableCode" => Severity::Error,
        "correctness/invalidPreloadPath" => Severity::Error,
        "correctness/missingSignalHandler" => Severity::Error,
        "correctness/invalidResourceType" => Severity::Warning,
        "correctness/missingClassName" => Severity::Warning,
        "correctness/missingParentNodeContract" => Severity::Warning,
        "correctness/missingParentSignalContract" => Severity::Warning,
        "correctness/missingParentMethodContract" => Severity::Warning,
        "correctness/noOnreadyWithExport" => Severity::Error,
        "correctness/noSelfAssignment" => Severity::Error,
        "correctness/duplicateDictionaryKey" => Severity::Error,
        "correctness/noAccessAfterFree" => Severity::Error,
        "correctness/noUnusedParameter" => Severity::Warning,
        "correctness/noDeprecatedApi" => Severity::Warning,
        "correctness/superReadyFirst" => Severity::Warning,
        "correctness/noDeprecatedSyntax" => Severity::Error,
        "correctness/noStringSignalConnect" => Severity::Warning,
        "correctness/unnecessaryPass" => Severity::Warning,
        "correctness/duplicatedLoad" => Severity::Warning,
        // Style
        "style/namingConvention" => Severity::Warning,
        "style/noUntypedDeclaration" => Severity::Warning,
        "style/booleanOperators" => Severity::Warning,
        "style/lineLength" => Severity::Warning,
        "style/commentSpacing" => Severity::Warning,
        "style/exportTypeHint" => Severity::Warning,
        "style/noBoolComparison" => Severity::Warning,
        "style/fileNaming" => Severity::Warning,
        "style/signalParameterTypes" => Severity::Warning,
        "style/preferPreload" => Severity::Warning,
        "style/classDefinitionsOrder" => Severity::Warning,
        "style/noUnnecessaryElse" => Severity::Warning,
        "style/functionArgumentsNumber" => Severity::Warning,
        "style/cognitiveComplexity" => Severity::Warning,
        "style/cyclomaticComplexity" => Severity::Warning,
        // Performance
        "performance/noExpensiveProcess" => Severity::Warning,
        "performance/noStringConcatLoop" => Severity::Warning,
        "performance/noPreloadInLoop" => Severity::Warning,
        "performance/noAddChildInProcess" => Severity::Warning,
        // Suspicious
        "suspicious/noShadowedVariable" => Severity::Warning,
        "suspicious/noDuplicateBranch" => Severity::Error,
        "suspicious/noSelfComparison" => Severity::Warning,
        "suspicious/noShadowingBuiltin" => Severity::Warning,
        "suspicious/expressionNotAssigned" => Severity::Warning,
        // Shader rules
        "shader/missingShaderType" => Severity::Error,
        "shader/invalidShaderType" => Severity::Error,
        "shader/uninitializedVariable" => Severity::Warning,
        "shader/unusedUniform" => Severity::Warning,
        "shader/unusedVarying" => Severity::Warning,
        "shader/unusedFunction" => Severity::Warning,
        "shader/invalidRenderMode" => Severity::Error,
        "shader/namingConvention" => Severity::Warning,
        "shader/floatLiteralStyle" => Severity::Warning,
        "shader/commentSpacing" => Severity::Warning,
        "shader/codeOrder" => Severity::Warning,
        "shader/oneStatementPerLine" => Severity::Warning,
        "shader/avoidDiscard" => Severity::Warning,
        "shader/precisionHints" => Severity::Warning,
        "shader/cognitiveComplexity" => Severity::Warning,
        "shader/cyclomaticComplexity" => Severity::Warning,
        _ => Severity::Warning,
    }
}

fn is_shader_rule_enabled(rule_id: &str, config: &ShaderConfig) -> bool {
    if !config.enabled {
        return false;
    }
    let name = rule_id.strip_prefix("shader/").unwrap_or(rule_id);

    if let Some(sev) = config.rules.shader.get(name) {
        return !matches!(sev, RuleSeverity::Off);
    }
    // Opt-in rules: disabled by default
    const SHADER_OPT_IN: &[&str] = &[
        "shader/avoidDiscard",
        "shader/precisionHints",
        "shader/cognitiveComplexity",
        "shader/cyclomaticComplexity",
    ];
    if SHADER_OPT_IN.contains(&rule_id) {
        return false;
    }
    config.rules.recommended
}

fn get_shader_configured_severity(rule_id: &str, config: &ShaderConfig) -> Option<Severity> {
    let name = rule_id.strip_prefix("shader/").unwrap_or(rule_id);
    match config.rules.shader.get(name)? {
        RuleSeverity::Error => Some(Severity::Error),
        RuleSeverity::Warn => Some(Severity::Warning),
        RuleSeverity::Off => None,
    }
}

fn is_rule_enabled(rule_id: &str, config: &LinterConfig) -> bool {
    if !config.enabled {
        return false;
    }
    let (group, name) = if let Some((g, n)) = rule_id.split_once('/') {
        (g, n)
    } else {
        return true;
    };
    let rules_map = match group {
        "correctness" => &config.rules.correctness,
        "style" => &config.rules.style,
        "performance" => &config.rules.performance,
        "suspicious" => &config.rules.suspicious,
        _ => return true,
    };
    if let Some(sev) = rules_map.get(name) {
        return !matches!(sev, RuleSeverity::Off);
    }
    // Opt-in rules: disabled by default unless explicitly configured
    const OPT_IN_RULES: &[&str] = &[
        "style/noUntypedDeclaration",
        "style/lineLength",
        "style/fileNaming",
        "style/signalParameterTypes",
        "style/preferPreload",
        "style/functionArgumentsNumber",
        "style/cognitiveComplexity",
        "style/cyclomaticComplexity",
    ];
    if OPT_IN_RULES.contains(&rule_id) {
        return false;
    }
    config.rules.recommended
}

fn get_configured_severity(rule_id: &str, config: &LinterConfig) -> Option<Severity> {
    let (group, name) = rule_id.split_once('/')?;
    let rules_map = match group {
        "correctness" => &config.rules.correctness,
        "style" => &config.rules.style,
        "performance" => &config.rules.performance,
        "suspicious" => &config.rules.suspicious,
        _ => return None,
    };
    match rules_map.get(name)? {
        RuleSeverity::Error => Some(Severity::Error),
        RuleSeverity::Warn => Some(Severity::Warning),
        RuleSeverity::Off => None,
    }
}

fn is_project_rule_enabled(
    rule_id: &str,
    config: &LinterConfig,
    project_graph_enabled: bool,
) -> bool {
    if !config.enabled || !project_graph_enabled {
        return false;
    }
    is_rule_enabled(rule_id, config)
}

impl LintEngine {
    pub fn new(config: &LinterConfig) -> Self {
        Self::new_with_project(config, true)
    }

    pub fn new_with_project(config: &LinterConfig, project_graph_enabled: bool) -> Self {
        Self::new_full(config, project_graph_enabled, &ShaderConfig::default())
    }

    pub fn new_full(
        config: &LinterConfig,
        project_graph_enabled: bool,
        shader_config: &ShaderConfig,
    ) -> Self {
        let all = rules::all_rules();
        let rules: Vec<Box<dyn Rule>> = all
            .into_iter()
            .filter(|r| is_rule_enabled(r.metadata().id, config))
            .collect();
        let all_project = rules::all_project_rules();
        let project_rules: Vec<Box<dyn ProjectRule>> = all_project
            .into_iter()
            .filter(|r| is_project_rule_enabled(r.metadata().id, config, project_graph_enabled))
            .collect();
        let all_shader = shader_rules::all_shader_rules();
        let shader_rules: Vec<Box<dyn ShaderRule>> = all_shader
            .into_iter()
            .filter(|r| is_shader_rule_enabled(r.metadata().id, shader_config))
            .collect();
        Self {
            rules,
            project_rules,
            shader_rules,
            config: config.clone(),
            shader_config: shader_config.clone(),
        }
    }

    pub fn lint(
        &self,
        tree: &Tree,
        source: &str,
        file_path: &str,
        context: Option<&LintContext>,
        graph: Option<&ProjectGraph>,
        script_res_path: Option<&str>,
    ) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        for rule in &self.rules {
            let mut results = rule.check(tree, source, context);
            let meta = rule.metadata();
            for diag in &mut results {
                diag.file_path = Some(file_path.to_string());
                diag.rule_id = Some(meta.id.to_string());
                if let Some(sev) = get_configured_severity(meta.id, &self.config) {
                    diag.severity = sev;
                } else {
                    diag.severity = recommended_severity(meta.id);
                }
            }
            diagnostics.extend(results);
        }
        if let (Some(graph), Some(script_res_path)) = (graph, script_res_path) {
            for rule in &self.project_rules {
                let mut results = rule.check(tree, source, graph, script_res_path);
                let meta = rule.metadata();
                for diag in &mut results {
                    diag.file_path = Some(file_path.to_string());
                    diag.rule_id = Some(meta.id.to_string());
                    if let Some(sev) = get_configured_severity(meta.id, &self.config) {
                        diag.severity = sev;
                    } else {
                        diag.severity = recommended_severity(meta.id);
                    }
                }
                diagnostics.extend(results);
            }
        }
        // Run the fileNaming check separately since it needs the file_path
        // which is not available through the Rule trait's check() method
        if is_rule_enabled("style/fileNaming", &self.config) {
            let mut file_diags = file_naming::check_filename(file_path);
            for diag in &mut file_diags {
                if let Some(sev) = get_configured_severity("style/fileNaming", &self.config) {
                    diag.severity = sev;
                } else {
                    diag.severity = recommended_severity("style/fileNaming");
                }
            }
            diagnostics.extend(file_diags);
        }

        diagnostics
    }

    /// Lint a GDShader file.
    pub fn lint_shader(&self, tree: &Tree, source: &str, file_path: &str) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        for rule in &self.shader_rules {
            let mut results = rule.check(tree, source);
            let meta = rule.metadata();
            for diag in &mut results {
                diag.file_path = Some(file_path.to_string());
                diag.rule_id = Some(meta.id.to_string());
                if let Some(sev) = get_shader_configured_severity(meta.id, &self.shader_config) {
                    diag.severity = sev;
                } else {
                    diag.severity = recommended_severity(meta.id);
                }
            }
            diagnostics.extend(results);
        }
        diagnostics
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gozen_config::LinterConfig;
    use gozen_parser::GDScriptParser;

    fn lint_source(source: &str) -> Vec<Diagnostic> {
        let config = LinterConfig::default();
        let engine = LintEngine::new(&config);
        let mut parser = GDScriptParser::new();
        let tree = parser.parse(source).expect("Failed to parse");
        engine.lint(&tree, source, "test.gd", None, None, None)
    }

    #[test]
    fn test_self_assignment_detected() {
        let source = "extends Node\n\nfunc _ready():\n\tx = x\n";
        let diags = lint_source(source);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("assigned to itself")),
            "Expected self-assignment diagnostic, got: {:?}",
            diags.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_duplicate_dictionary_key() {
        let source = "extends Node\n\nvar d = {\"a\": 1, \"a\": 2}\n";
        let diags = lint_source(source);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("Duplicate dictionary key")),
            "Expected duplicate key diagnostic, got: {:?}",
            diags.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_deprecated_syntax_setget() {
        let source = "extends Node\n\nvar hp setget set_hp, get_hp\n";
        let diags = lint_source(source);
        assert!(
            diags.iter().any(|d| d.message.contains("setget")),
            "Expected setget diagnostic, got: {:?}",
            diags.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_no_false_positive_on_non_self_assignment() {
        let source = "extends Node\n\nfunc _ready():\n\tx = y\n";
        let diags = lint_source(source);
        assert!(
            !diags
                .iter()
                .any(|d| d.message.contains("assigned to itself")),
            "Should not flag x = y as self-assignment"
        );
    }

    #[test]
    fn test_contains_word_operator_precedence() {
        // This tests that our fix to contains_word works correctly
        let source = "extends KinematicBody2D\n";
        let diags = lint_source(source);
        assert!(
            diags.iter().any(|d| d.message.contains("renamed")),
            "Expected deprecated API diagnostic for KinematicBody2D, got: {:?}",
            diags.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }
}
