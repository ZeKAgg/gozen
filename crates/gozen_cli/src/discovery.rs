use globset::{GlobSet, GlobSetBuilder};
use ignore::WalkBuilder;
use gozen_config::GozenConfig;
use std::fs;
use std::path::{Path, PathBuf};

pub fn discover_files(paths: &[PathBuf], config: &GozenConfig) -> Vec<PathBuf> {
    let allowed_roots = build_allowed_roots(paths);
    let include_set = build_glob_set(&config.files.includes, true);
    let ignore_set = build_glob_set(&config.files.ignore, false);
    let mut files = Vec::new();
    for path in paths {
        if path.is_file() {
            if is_allowed_file(path, &allowed_roots)
                && matches_include(
                    path,
                    path.parent().unwrap_or(Path::new(".")),
                    &include_set,
                    &ignore_set,
                )
            {
                files.push(path.clone());
            }
        } else if path.is_dir() {
            let walker = WalkBuilder::new(path)
                .hidden(true)
                .git_ignore(config.vcs.use_ignore_file)
                .build();
            for entry in walker.flatten() {
                let p = entry.path();
                if p.is_file()
                    && is_allowed_file(p, &allowed_roots)
                    && p.extension().is_some_and(|e| e == "gd" || e == "gdshader")
                    && matches_include(p, path, &include_set, &ignore_set)
                {
                    files.push(p.to_path_buf());
                }
            }
        }
    }
    files.sort();
    files.dedup();
    files
}

#[derive(Debug)]
struct AllowedRoot {
    path: PathBuf,
    is_file: bool,
}

fn build_allowed_roots(paths: &[PathBuf]) -> Vec<AllowedRoot> {
    let mut roots = Vec::new();
    for path in paths {
        if let Ok(canonical) = fs::canonicalize(path) {
            roots.push(AllowedRoot {
                path: canonical,
                is_file: path.is_file(),
            });
        }
    }
    roots
}

fn is_allowed_file(path: &Path, roots: &[AllowedRoot]) -> bool {
    // Reject symlinked files to prevent writes outside the selected roots.
    let metadata = match fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(_) => return false,
    };
    if metadata.file_type().is_symlink() {
        return false;
    }

    let canonical = match fs::canonicalize(path) {
        Ok(p) => p,
        Err(_) => return false,
    };

    roots.iter().any(|root| {
        if root.is_file {
            canonical == root.path
        } else {
            canonical.starts_with(&root.path)
        }
    })
}

fn build_glob_set(patterns: &[String], required: bool) -> Option<GlobSet> {
    if patterns.is_empty() {
        return if required {
            None
        } else {
            Some(GlobSet::empty())
        };
    }
    let mut builder = GlobSetBuilder::new();
    for pat in patterns {
        let glob = globset::Glob::new(pat).ok()?;
        builder.add(glob);
    }
    builder.build().ok()
}

fn matches_include(
    path: &Path,
    root: &Path,
    include_set: &Option<GlobSet>,
    ignore_set: &Option<GlobSet>,
) -> bool {
    let relative = path
        .strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");
    if let Some(ref set) = ignore_set {
        if set.is_match(&relative) {
            return false;
        }
    }
    if let Some(ref set) = include_set {
        set.is_match(&relative)
    } else {
        path.extension()
            .is_some_and(|e| e == "gd" || e == "gdshader")
    }
}
