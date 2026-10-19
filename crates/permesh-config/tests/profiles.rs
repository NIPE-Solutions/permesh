// SPDX-License-Identifier: MIT
#![allow(clippy::unwrap_used)]
use permesh_config::{
    Config,
    profiles::{ProfileEntry, ProfileSet, load_profiles},
};
use std::{fs, path::Path};

const BASE: &str = "version: 1\norganization: {name: Test}\nproviders: [{id: demo, type: demo}]\n";
const TEAM: &str = "profile: base\ngrants:\n  - {instance: demo, group: team-10}\n";
const BACKEND: &str = "profile: backend\nincludes: [base]\ngrants:\n  - {instance: demo, group: team-10}\n  - {instance: demo, role: Read, resource: repo-21}\n";

fn workspace(extra: &str, files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("permesh.yaml"), format!("{BASE}{extra}")).unwrap();
    for (name, body) in files {
        let path = dir.path().join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, body).unwrap();
    }
    dir
}
fn load(dir: &Path) -> Result<Option<ProfileSet>, permesh_config::Error> {
    let config = Config::load(&dir.join("permesh.yaml")).unwrap();
    load_profiles(dir, &config)
}

#[test]
fn includes_are_expanded_and_deduplicated() {
    let dir = workspace(
        "",
        &[
            ("profiles/base.yaml", TEAM),
            ("profiles/backend.yaml", BACKEND),
        ],
    );
    let set = load(dir.path()).unwrap().unwrap();
    let backend = &set.profiles["backend"];
    assert_eq!(backend.len(), 2);
    assert!(backend.contains(&ProfileEntry::Group {
        instance: "demo".into(),
        group: "team-10".into()
    }));
    assert!(backend.contains(&ProfileEntry::Role {
        instance: "demo".into(),
        role: "Read".into(),
        resource: "repo-21".into()
    }));
    assert!(!set.enforce);
}

#[test]
fn include_cycle_is_input_error() {
    let dir = workspace(
        "",
        &[
            ("profiles/a.yaml", "profile: a\nincludes: [b]\ngrants: []\n"),
            ("profiles/b.yaml", "profile: b\nincludes: [a]\ngrants: []\n"),
        ],
    );
    assert!(load(dir.path()).is_err());
}

#[test]
fn unknown_include_or_unknown_profile_in_people_is_error() {
    let dir = workspace(
        "",
        &[(
            "profiles/a.yaml",
            "profile: a\nincludes: [nope]\ngrants: []\n",
        )],
    );
    assert!(load(dir.path()).is_err());
    let dir = workspace(
        "",
        &[
            ("profiles/base.yaml", TEAM),
            (
                "people/a.yaml",
                "identity: a@example.com\nprofiles: [nope]\n",
            ),
        ],
    );
    assert!(load(dir.path()).is_err());
}

#[test]
fn duplicate_profile_name_is_error() {
    let dir = workspace(
        "",
        &[("profiles/one.yaml", TEAM), ("profiles/two.yaml", TEAM)],
    );
    assert!(load(dir.path()).is_err());
}

#[test]
fn entry_with_group_and_role_is_error() {
    let dir = workspace(
        "",
        &[(
            "profiles/a.yaml",
            "profile: a\ngrants:\n  - {instance: demo, group: g, role: Read, resource: r}\n",
        )],
    );
    assert!(load(dir.path()).is_err());
}

#[test]
fn role_without_resource_is_error() {
    let dir = workspace(
        "",
        &[(
            "profiles/a.yaml",
            "profile: a\ngrants:\n  - {instance: demo, role: Read}\n",
        )],
    );
    assert!(load(dir.path()).is_err());
}

#[test]
fn unknown_instance_is_error() {
    let dir = workspace(
        "",
        &[(
            "profiles/a.yaml",
            "profile: a\ngrants:\n  - {instance: github-x, group: g}\n",
        )],
    );
    let error = load(dir.path()).unwrap_err().to_string();
    assert!(error.contains("profiles/a.yaml"), "{error}");
}

#[test]
fn people_file_lists_identity_inside() {
    let dir = workspace(
        "",
        &[
            ("profiles/base.yaml", TEAM),
            ("profiles/backend.yaml", BACKEND),
            (
                "people/alice.yaml",
                "identity: alice@example.com\nprofiles: [backend]\n",
            ),
        ],
    );
    let set = load(dir.path()).unwrap().unwrap();
    assert_eq!(
        set.people["alice@example.com"],
        ["backend".to_string()].into()
    );
}

#[test]
fn duplicate_person_is_error() {
    let dir = workspace(
        "",
        &[
            ("profiles/base.yaml", TEAM),
            (
                "people/a.yaml",
                "identity: a@example.com\nprofiles: [base]\n",
            ),
            (
                "people/b.yaml",
                "identity: a@example.com\nprofiles: [base]\n",
            ),
        ],
    );
    assert!(load(dir.path()).is_err());
}

#[test]
fn non_yaml_files_are_ignored() {
    let dir = workspace(
        "",
        &[
            ("profiles/base.yaml", TEAM),
            ("profiles/README.md", "# not a profile\n"),
        ],
    );
    assert_eq!(load(dir.path()).unwrap().unwrap().profiles.len(), 1);
}

#[test]
fn absent_layout_is_none() {
    let dir = workspace("", &[]);
    assert!(load(dir.path()).unwrap().is_none());
}

#[test]
fn directory_group_must_name_known_profile() {
    let section = "profiles:\n  enforce: true\n  directory_groups:\n    - {instance: demo, group: team-10, profiles: [base]}\n";
    let dir = workspace(section, &[("profiles/base.yaml", TEAM)]);
    let set = load(dir.path()).unwrap().unwrap();
    assert!(set.enforce);
    assert_eq!(set.directory_groups.len(), 1);
    let dir = workspace(section, &[]);
    assert!(load(dir.path()).is_err());
}

#[test]
fn readme_only_directories_are_not_configured() {
    let dir = workspace(
        "",
        &[
            ("profiles/README.md", "# x\n"),
            ("people/README.md", "# x\n"),
        ],
    );
    assert!(load(dir.path()).unwrap().is_none());
}
