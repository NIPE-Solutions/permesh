// SPDX-License-Identifier: MIT
//! `init --starter`: write the scheduled-check starter layout (mirrors examples/starter).
use crate::{error::AppError, report::Outcome};
use std::{fs, path::Path};

const TEMPLATE_ORGANIZATION: &str = "  name: \"Example organization\"\n";
const FILES: [(&str, &str); 8] = [
    (
        "permesh.yaml",
        include_str!("../../../examples/starter/permesh.yaml"),
    ),
    (
        "README.md",
        include_str!("../../../examples/starter/README.md"),
    ),
    (
        ".gitignore",
        include_str!("../../../examples/starter/.gitignore"),
    ),
    (
        "ci/providers.txt",
        include_str!("../../../examples/starter/ci/providers.txt"),
    ),
    (
        "ci/bootstrap.sh",
        include_str!("../../../examples/starter/ci/bootstrap.sh"),
    ),
    (
        "policies/review-policy.json",
        include_str!("../../../examples/starter/policies/review-policy.json"),
    ),
    (
        ".github/workflows/permesh-check.yml",
        include_str!("../../../examples/starter/.github/workflows/permesh-check.yml"),
    ),
    (
        ".gitlab-ci.yml",
        include_str!("../../../examples/starter/.gitlab-ci.yml"),
    ),
];

fn workspace(organization: &str) -> Result<String, AppError> {
    // A JSON string is a valid YAML double-quoted scalar.
    let quoted = serde_json::to_string(organization)
        .map_err(|_| AppError::input("Invalid organization name"))?;
    let yaml = FILES[0]
        .1
        .replacen(TEMPLATE_ORGANIZATION, &format!("  name: {quoted}\n"), 1);
    permesh_config::Config::from_bytes(yaml.as_bytes())?;
    Ok(yaml)
}

pub fn init(root: &Path, organization: &str) -> Result<Outcome, AppError> {
    let config = workspace(organization)?;
    for (file, _) in FILES {
        if fs::symlink_metadata(root.join(file)).is_ok() {
            return Err(AppError::input(
                "Starter files already exist here; choose an empty directory",
            ));
        }
    }
    for (file, contents) in FILES {
        let path = root.join(file);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|_| AppError::new(5, "Cannot create starter directory"))?;
        }
        let bytes = if file == "permesh.yaml" {
            config.as_bytes()
        } else {
            contents.as_bytes()
        };
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(if file.ends_with(".sh") { 0o755 } else { 0o644 });
        }
        std::io::Write::write_all(
            &mut options
                .open(&path)
                .map_err(|_| AppError::input("Cannot create starter file"))?,
            bytes,
        )
        .map_err(|_| AppError::new(5, "Cannot write starter file"))?;
    }
    Outcome::new(
        "init",
        serde_json::json!({
            "message": "Created starter workspace",
            "file": "permesh.yaml",
            "files": FILES.iter().map(|(f, _)| *f).collect::<Vec<_>>(),
            "next": "permesh provider add github --portable; see README.md",
        }),
    )
}
