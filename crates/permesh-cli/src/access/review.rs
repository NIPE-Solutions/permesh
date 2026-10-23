// SPDX-License-Identifier: MIT
//! `access review`: plan every assignment change between a base and a head workspace
//! (the pull-request flow). After merge, `--reviewed` binds each regenerated plan to
//! the reviewed copy when its operations are unchanged, and asks for re-review otherwise.
use super::{
    Change,
    model::{self, AccessPlan, Kind},
    plan_one, words,
};
use crate::{
    artifact::{self, Artifact},
    error::AppError,
    offboard,
    output::safe,
};
use permesh_config::profiles::ProfileSet;
use permesh_core as core;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

pub(crate) const INDEX: &str = "index.json";
pub(crate) const SUMMARY: &str = "summary.md";
/// `index.json` format: `{"access_review_index_version": 1, "plans": [IndexRow…]}`.
pub(crate) const INDEX_VERSION: u32 = 1;

/// The index document as written; rows stay in plan order.
pub(crate) fn index_value(rows: &[&IndexRow]) -> Value {
    json!({"access_review_index_version": INDEX_VERSION, "plans": rows})
}
fn index_rows(index: Value) -> Result<Vec<IndexRow>, AppError> {
    let unsupported = || {
        AppError::input(
            "Unsupported index.json format; regenerate the reviewed plans with this Permesh version",
        )
    };
    let Value::Object(mut index) = index else {
        return Err(unsupported());
    };
    if index.remove("access_review_index_version") != Some(json!(INDEX_VERSION)) {
        return Err(unsupported());
    }
    let plans = index.remove("plans").ok_or_else(unsupported)?;
    if !index.is_empty() {
        return Err(unsupported());
    }
    serde_json::from_value(plans).map_err(|_| unsupported())
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IndexRow {
    pub file: String,
    pub identity: String,
    pub kind: Kind,
    pub plan_sha256: String,
    pub steps: usize,
}

/// Profiles of the base checkout; a checkout without `permesh.yaml` assigned nothing.
pub(crate) fn load_base(base: &Path) -> Result<Option<ProfileSet>, AppError> {
    let file = base.join("permesh.yaml");
    if std::fs::symlink_metadata(&file).is_err() {
        return Ok(None);
    }
    crate::profile_command::load(&file)
}

fn empty() -> ProfileSet {
    ProfileSet {
        enforce: false,
        profiles: BTreeMap::new(),
        people: BTreeMap::new(),
        left: BTreeSet::new(),
        directory_groups: vec![],
    }
}

/// Effective assignment (people files plus directory mappings) against one capture.
fn assigned(
    set: &ProfileSet,
    artifact: &Artifact,
) -> Result<(BTreeMap<String, BTreeSet<String>>, bool), AppError> {
    let input = crate::profile_command::input(set, artifact);
    let (snapshots, aliases) = crate::resource_command::domain(artifact);
    let assignments = core::assign_profiles(&snapshots, &aliases, &input)?;
    let complete = assignments.complete();
    Ok((assignments.profiles, complete))
}

/// One change per identity whose assignment or `left` status differs.
fn changes(
    base: &ProfileSet,
    head: &ProfileSet,
    before: &BTreeMap<String, BTreeSet<String>>,
    after: &BTreeMap<String, BTreeSet<String>>,
) -> Vec<Change> {
    let identities: BTreeSet<&String> = before
        .keys()
        .chain(after.keys())
        .chain(&base.left)
        .chain(&head.left)
        .collect();
    let none = BTreeSet::new();
    let mut result = Vec::new();
    for identity in identities {
        let (was, now) = (
            before.get(identity).unwrap_or(&none),
            after.get(identity).unwrap_or(&none),
        );
        let (was_left, is_left) = (base.left.contains(identity), head.left.contains(identity));
        let change = |kind, add: &BTreeSet<String>, remove: &BTreeSet<String>| Change {
            kind,
            person: identity.clone(),
            add: add.clone(),
            remove: remove.clone(),
        };
        if is_left {
            if !was_left {
                result.push(change(Kind::Leave, &none, &none));
            }
        } else if was_left {
            // Back with profiles is a join; a departed person's file deleted is no change.
            if !now.is_empty() {
                result.push(change(Kind::Join, now, &none));
            }
        } else if was != now {
            let add = now.difference(was).cloned().collect();
            let remove: BTreeSet<String> = was.difference(now).cloned().collect();
            let kind = if remove.is_empty() {
                Kind::Join
            } else {
                Kind::Move
            };
            result.push(change(kind, &add, &remove));
        }
    }
    result
}

/// Reviewed plans keyed by (identity, kind).
pub(crate) type Reviewed = BTreeMap<(String, Kind), (IndexRow, AccessPlan)>;
/// Execution plans by file name, and the re-review rows.
pub(crate) type Bound = (Vec<(String, AccessPlan)>, Vec<Value>);

pub(crate) struct Regenerated {
    pub plans: Vec<(IndexRow, AccessPlan)>,
    pub profiles_changed: bool,
    pub complete: bool,
}

pub(crate) fn regenerate(
    base: Option<&ProfileSet>,
    head: Option<&ProfileSet>,
    artifact: &Artifact,
    mode: offboard::Mode,
    hours: u32,
    now: time::OffsetDateTime,
) -> Result<Regenerated, AppError> {
    let (base, head) = (
        base.cloned().unwrap_or_else(empty),
        head.cloned().unwrap_or_else(empty),
    );
    let (before, before_complete) = assigned(&base, artifact)?;
    let (after, after_complete) = assigned(&head, artifact)?;
    // Profiles removed from the head are still planned with their base definition.
    let mut planning = head.clone();
    for (name, entries) in &base.profiles {
        planning
            .profiles
            .entry(name.clone())
            .or_insert_with(|| entries.clone());
    }
    let mut plans = Vec::new();
    for change in changes(&base, &head, &before, &after) {
        let plan = plan_one(&planning, artifact, &change, None, mode, hours, now)?;
        let row = IndexRow {
            file: format!("{:03}.json", plans.len()),
            identity: plan.identity.clone(),
            kind: plan.kind,
            plan_sha256: model::plan_sha256(&plan)?,
            steps: plan.steps.len(),
        };
        plans.push((row, plan));
    }
    if plans.len() > 999 {
        return Err(AppError::input(
            "More than 999 people changed in one review; split the change",
        ));
    }
    let complete = before_complete
        && after_complete
        && artifact.complete()
        && plans.iter().all(|(_, p)| p.complete);
    Ok(Regenerated {
        plans,
        profiles_changed: base.profiles != head.profiles,
        complete,
    })
}

fn valid_file(name: &str) -> bool {
    name.len() == 8 && name.ends_with(".json") && name[..3].bytes().all(|b| b.is_ascii_digit())
}

/// The reviewed plans by (identity, kind), checked against their index.
pub(crate) fn load_reviewed(dir: &Path) -> Result<Reviewed, AppError> {
    let mismatch = || AppError::input("Reviewed plans do not match their index.json");
    let rows = index_rows(artifact::load_document(&dir.join(INDEX))?)?;
    let mut reviewed = BTreeMap::new();
    for row in rows {
        if !valid_file(&row.file) {
            return Err(mismatch());
        }
        let plan: AccessPlan = artifact::load_document(&dir.join(&row.file))?;
        plan.validate()?;
        if model::plan_sha256(&plan)? != row.plan_sha256
            || plan.identity != row.identity
            || plan.kind != row.kind
            || reviewed
                .insert((row.identity.clone(), row.kind), (row, plan))
                .is_some()
        {
            return Err(mismatch());
        }
    }
    Ok(reviewed)
}

fn ids(plan: &AccessPlan) -> BTreeSet<&str> {
    plan.steps.iter().map(|s| s.id.as_str()).collect()
}

/// Execution plans for unchanged operations; re-review rows for everything else.
pub(crate) fn bind(
    plans: &[(IndexRow, AccessPlan)],
    mut reviewed: Reviewed,
) -> Result<Bound, AppError> {
    let mut executions = Vec::new();
    let mut rereview = Vec::new();
    let none = BTreeSet::new();
    for (row, plan) in plans {
        let earlier = reviewed.remove(&(row.identity.clone(), row.kind));
        let (now, before) = (
            ids(plan),
            earlier.as_ref().map_or(none.clone(), |(_, p)| ids(p)),
        );
        match earlier {
            Some((reviewed_row, _)) if now == before => {
                let mut execution = plan.clone();
                execution.reviewed_plan_sha256 = Some(reviewed_row.plan_sha256);
                execution.validate()?;
                executions.push((row.file.replace(".json", ".execution.json"), execution));
            }
            _ => rereview.push(json!({
                "identity": row.identity,
                "kind": row.kind,
                "added_steps": now.difference(&before).collect::<Vec<_>>(),
                "removed_steps": before.difference(&now).collect::<Vec<_>>(),
            })),
        }
    }
    // Reviewed plans with no regenerated counterpart also need a second look.
    for ((identity, kind), (_, plan)) in reviewed {
        rereview.push(json!({
            "identity": identity,
            "kind": kind,
            "added_steps": [],
            "removed_steps": ids(&plan).into_iter().collect::<Vec<_>>(),
        }));
    }
    Ok((executions, rereview))
}

/// Counts and step types only, unless `detail` opts in to names and resources.
pub(crate) fn summary(
    plans: &[(IndexRow, AccessPlan)],
    rereview: usize,
    profiles_changed: bool,
    complete: bool,
    detail: bool,
) -> String {
    let kinds = |kind: Kind| plans.iter().filter(|(r, _)| r.kind == kind).count();
    let steps: Vec<&model::Step> = plans.iter().flat_map(|(_, p)| &p.steps).collect();
    let typed = |f: fn(&core::Operation) -> bool| steps.iter().filter(|s| f(&s.operation)).count();
    use core::Operation as O;
    let mut text = format!(
        "# Permesh access plan\n\nPeople affected: {} (join {}, move {}, leave {})\nSteps: {} (add membership {}, add role {}, remove membership {}, remove role {}, account needed {}, review {})\nCannot tell: {}\n",
        plans.len(),
        kinds(Kind::Join),
        kinds(Kind::Move),
        kinds(Kind::Leave),
        steps.len(),
        typed(|o| matches!(o, O::AddMembership { .. })),
        typed(|o| matches!(o, O::AddRole { .. })),
        typed(|o| matches!(o, O::RemoveMembership { .. })),
        typed(|o| matches!(o, O::RemoveRole { .. })),
        typed(|o| matches!(o, O::AccountNeeded { .. })),
        typed(|o| {
            matches!(
                o,
                O::ReviewAccountLifecycle { .. }
                    | O::ReviewOwnership { .. }
                    | O::ReviewMachineDependency { .. }
            )
        }),
        steps.iter().filter(|s| s.state == "cannot_tell").count(),
    );
    if !complete {
        text.push_str("Incomplete: some access could not be observed; the plans may miss access\n");
    }
    if profiles_changed {
        text.push_str("Profile definitions changed; run permesh profile check\n");
    }
    if rereview > 0 {
        text.push_str(&format!(
            "Not comparable with the reviewed plan: {rereview} people; re-review required\n"
        ));
    }
    if detail {
        for (row, plan) in plans {
            let kind = serde_json::to_value(row.kind).unwrap_or_default();
            text.push_str(&format!(
                "\n## {} {}\n\n",
                safe(kind.as_str().unwrap_or_default()),
                safe(&row.identity)
            ));
            for step in &plan.steps {
                let operation = serde_json::to_value(&step.operation).unwrap_or_default();
                text.push_str(&format!("- {}\n", words(&operation)));
            }
        }
    }
    text
}

/// Fail before capture when the fixed output files already exist.
pub(crate) fn check_out(out: &Path, overwrite: bool) -> Result<(), AppError> {
    for name in [INDEX, SUMMARY] {
        artifact::check_destination(&out.join(name), overwrite)?;
    }
    Ok(())
}
