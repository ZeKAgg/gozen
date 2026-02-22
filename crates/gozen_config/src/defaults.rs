use crate::schema::{
    AnalyzerConfig, FilesConfig, FormatterConfig, LinterConfig, RulesConfig, ShaderRulesConfig,
    VcsConfig,
};

impl Default for FilesConfig {
    fn default() -> Self {
        Self {
            includes: vec!["**/*.gd".into(), "**/*.gdshader".into()],
            ignore: vec![".godot".into(), "addons/gozen/**".into()],
        }
    }
}

impl Default for FormatterConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            indent_style: "tab".into(),
            indent_width: 4,
            line_width: 100,
            trailing_comma: true,
            end_of_line: "lf".into(),
        }
    }
}

impl Default for LinterConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            rules: RulesConfig::default(),
        }
    }
}

impl Default for VcsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            client_kind: "git".into(),
            use_ignore_file: false,
            default_branch: None,
        }
    }
}

impl Default for RulesConfig {
    fn default() -> Self {
        Self {
            recommended: true,
            correctness: Default::default(),
            style: Default::default(),
            performance: Default::default(),
            suspicious: Default::default(),
        }
    }
}

impl Default for AnalyzerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            project_graph: true,
        }
    }
}

impl Default for ShaderRulesConfig {
    fn default() -> Self {
        Self {
            recommended: true,
            shader: Default::default(),
        }
    }
}
