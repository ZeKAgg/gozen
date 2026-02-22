use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GozenConfig {
    #[serde(default)]
    pub files: FilesConfig,
    #[serde(default)]
    pub vcs: VcsConfig,
    #[serde(default)]
    pub formatter: FormatterConfig,
    #[serde(default)]
    pub linter: LinterConfig,
    #[serde(default)]
    pub analyzer: AnalyzerConfig,
    #[serde(default)]
    pub shader: ShaderConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilesConfig {
    #[serde(default = "default_includes")]
    pub includes: Vec<String>,
    #[serde(default)]
    pub ignore: Vec<String>,
}

fn default_includes() -> Vec<String> {
    vec!["**/*.gd".into(), "**/*.gdshader".into()]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VcsConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_client_kind")]
    pub client_kind: String,
    #[serde(default)]
    pub use_ignore_file: bool,
    #[serde(default)]
    pub default_branch: Option<String>,
}

fn default_client_kind() -> String {
    "git".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormatterConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_indent_style")]
    pub indent_style: String,
    #[serde(default = "default_indent_width")]
    pub indent_width: usize,
    #[serde(default = "default_line_width")]
    pub line_width: usize,
    #[serde(default = "default_true")]
    pub trailing_comma: bool,
    #[serde(default = "default_end_of_line")]
    pub end_of_line: String,
}

fn default_true() -> bool {
    true
}
fn default_indent_style() -> String {
    "tab".into()
}
fn default_indent_width() -> usize {
    4
}
fn default_line_width() -> usize {
    100
}
fn default_end_of_line() -> String {
    "lf".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinterConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub rules: RulesConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RulesConfig {
    #[serde(default = "default_true")]
    pub recommended: bool,
    #[serde(default)]
    pub correctness: HashMap<String, RuleSeverity>,
    #[serde(default)]
    pub style: HashMap<String, RuleSeverity>,
    #[serde(default)]
    pub performance: HashMap<String, RuleSeverity>,
    #[serde(default)]
    pub suspicious: HashMap<String, RuleSeverity>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuleSeverity {
    Error,
    Warn,
    Off,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzerConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub project_graph: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShaderConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub rules: ShaderRulesConfig,
}

impl Default for ShaderConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            rules: ShaderRulesConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShaderRulesConfig {
    #[serde(default = "default_true")]
    pub recommended: bool,
    #[serde(default)]
    pub shader: HashMap<String, RuleSeverity>,
}
