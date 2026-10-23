// SPDX-License-Identifier: MIT
//! Tool file arguments resolve inside the workspace root, never outside it.
use crate::error::AppError;
use std::path::{Component, Path, PathBuf};

const RELATIVE: &str = "Paths must be relative to the workspace";
const OUTSIDE: &str = "Path is outside the workspace";
const MISSING: &str = "File not found in the workspace";
const EXISTS: &str = "Output file already exists";
const PLACEMENT: &str =
    "Plan files must be .json files outside people/, profiles/ and hidden folders";

/// Plan JSON is valid YAML: inside people/ or profiles/ it would break every profile load.
fn plan_placement(path: &Path) -> Result<(), AppError> {
    let mut names = path.components().filter_map(|component| match component {
        Component::Normal(name) => Some(name.to_string_lossy()),
        _ => None,
    });
    let first = names.next();
    let reserved = matches!(first.as_deref(), Some("people" | "profiles"));
    let hidden = first
        .into_iter()
        .chain(names)
        .any(|name| name.starts_with('.'));
    let json = path
        .extension()
        .is_some_and(|extension| extension == "json");
    if reserved || hidden || !json {
        Err(AppError::input(PLACEMENT))
    } else {
        Ok(())
    }
}

/// Refuses absolute paths and `..` that climbs above the root before touching the file system.
fn lexical(relative: &str) -> Result<&Path, AppError> {
    let path = Path::new(relative);
    if relative.is_empty() {
        return Err(AppError::input(RELATIVE));
    }
    let mut depth = 0usize;
    for component in path.components() {
        match component {
            Component::Normal(_) => depth += 1,
            Component::CurDir => {}
            Component::ParentDir => {
                depth = depth
                    .checked_sub(1)
                    .ok_or_else(|| AppError::input(OUTSIDE))?;
            }
            Component::RootDir | Component::Prefix(_) => return Err(AppError::input(RELATIVE)),
        }
    }
    Ok(path)
}
fn canonical_root(root: &Path) -> Result<PathBuf, AppError> {
    std::fs::canonicalize(root).map_err(|_| AppError::input(MISSING))
}

/// An input file inside the root; symlinks are followed and must stay inside.
pub(crate) fn existing(root: &Path, relative: &str) -> Result<PathBuf, AppError> {
    let path = lexical(relative)?;
    let root = canonical_root(root)?;
    let resolved = std::fs::canonicalize(root.join(path)).map_err(|_| AppError::input(MISSING))?;
    if resolved.starts_with(&root) {
        Ok(resolved)
    } else {
        Err(AppError::input(OUTSIDE))
    }
}

/// A plan file to create inside the root: a `.json` file outside workspace inputs and hidden
/// folders, whose parent exists inside the root and which does not exist yet.
pub(crate) fn new_file(root: &Path, relative: &str) -> Result<PathBuf, AppError> {
    let path = lexical(relative)?;
    let root = canonical_root(root)?;
    let joined = root.join(path);
    // A dangling symlink counts as existing, so nothing is written through it.
    if std::fs::symlink_metadata(&joined).is_ok() {
        return Err(AppError::input(EXISTS));
    }
    plan_placement(path)?;
    let (Some(parent), Some(name)) = (joined.parent(), joined.file_name()) else {
        return Err(AppError::input(EXISTS));
    };
    let parent = std::fs::canonicalize(parent).map_err(|_| AppError::input(MISSING))?;
    if parent.starts_with(&root) {
        Ok(parent.join(name))
    } else {
        Err(AppError::input(OUTSIDE))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use std::fs;

    fn message(result: Result<PathBuf, AppError>) -> String {
        let error = result.unwrap_err();
        assert_eq!(error.code, 2);
        error.message
    }

    #[test]
    fn accepts_relative_file_inside_root() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("snapshots")).unwrap();
        fs::write(dir.path().join("snapshots/a.json"), "{}").unwrap();
        let root = fs::canonicalize(dir.path()).unwrap();
        assert_eq!(
            existing(dir.path(), "snapshots/a.json").unwrap(),
            root.join("snapshots/a.json")
        );
    }

    #[test]
    fn rejects_escaping_and_absolute_paths() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("a")).unwrap();
        for (path, expected) in [
            ("../x", "Path is outside the workspace"),
            ("a/../../x", "Path is outside the workspace"),
            ("/etc/passwd", "Paths must be relative to the workspace"),
            ("", "Paths must be relative to the workspace"),
            ("missing.json", "File not found in the workspace"),
        ] {
            let text = message(existing(dir.path(), path));
            assert_eq!(text, expected, "{path}");
            assert!(path.is_empty() || !text.contains(path), "{path}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_out_of_root() {
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("file"), "secret").unwrap();
        let dir = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path().join("file"), dir.path().join("link")).unwrap();
        std::os::unix::fs::symlink(outside.path(), dir.path().join("out")).unwrap();
        assert_eq!(
            message(existing(dir.path(), "link")),
            "Path is outside the workspace"
        );
        assert_eq!(
            message(new_file(dir.path(), "out/new.json")),
            "Path is outside the workspace"
        );
    }

    #[test]
    fn new_file_refuses_workspace_inputs_and_non_json() {
        let dir = tempfile::tempdir().unwrap();
        for folder in ["people", "profiles", ".git", "plans", "plans/.hidden"] {
            fs::create_dir_all(dir.path().join(folder)).unwrap();
        }
        for path in [
            "people/carol.yaml",
            "people/carol.json",
            "profiles/x.json",
            ".git/x.json",
            "plans/.hidden/x.json",
            "plans/x.yaml",
            "plans/x",
            "./people/x.json",
        ] {
            assert_eq!(
                message(new_file(dir.path(), path)),
                "Plan files must be .json files outside people/, profiles/ and hidden folders",
                "{path}"
            );
        }
        assert!(new_file(dir.path(), "plans/x.json").is_ok());
    }

    #[test]
    fn new_file_requires_existing_parent_inside_root_and_absent_target() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("plans")).unwrap();
        let root = fs::canonicalize(dir.path()).unwrap();
        assert_eq!(
            new_file(dir.path(), "plans/new.json").unwrap(),
            root.join("plans/new.json")
        );
        fs::write(dir.path().join("plans/old.json"), "{}").unwrap();
        assert_eq!(
            message(new_file(dir.path(), "plans/old.json")),
            "Output file already exists"
        );
        assert_eq!(
            message(new_file(dir.path(), "../new.json")),
            "Path is outside the workspace"
        );
        assert_eq!(
            message(new_file(dir.path(), "nodir/new.json")),
            "File not found in the workspace"
        );
        assert_eq!(
            message(new_file(dir.path(), "plans/..")),
            "Output file already exists"
        );
    }
}
