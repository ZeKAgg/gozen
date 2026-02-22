use std::path::Path;

use gozen_config::GozenConfig;

pub fn run(force: bool, start_dir: &Path) -> anyhow::Result<()> {
    let config_path = start_dir.join("gozen.json");
    if config_path.exists() && !force {
        anyhow::bail!("gozen.json already exists. Use --force to overwrite.");
    }
    if let Ok(meta) = std::fs::symlink_metadata(&config_path) {
        if meta.file_type().is_symlink() {
            anyhow::bail!("Refusing to overwrite symlinked path: {}", config_path.display());
        }
    }
    let default = GozenConfig::default();
    let json = serde_json::to_string_pretty(&default)?;
    std::fs::write(&config_path, json)?;
    println!("Created gozen.json. Run gozen check . to get started.");
    Ok(())
}
