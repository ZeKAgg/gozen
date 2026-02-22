use std::path::PathBuf;

/// Context passed to rules that need project-level information.
#[derive(Debug, Clone, Default)]
pub struct LintContext {
    /// Project root directory (e.g. where project.godot or gozen.json lives).
    /// Used to resolve res:// paths for invalidPreloadPath.
    pub project_root: Option<PathBuf>,
}
