// SPDX-License-Identifier: MIT
//! Read-only departure review. Imported plans are data, never executable instructions.
#[path = "offboard_assessment.rs"]
mod assessment;
#[path = "offboard_model.rs"]
mod model;
#[path = "offboard_validation.rs"]
mod validation;
#[path = "offboard_verification.rs"]
mod verification;
use assessment::{assess, recommendations};
use verification::verify;
#[path = "offboard_view.rs"]
mod view;
use crate::{
    args::Cli,
    artifact::{self, Artifact, State},
    blocking::BlockingPool,
    cancellation::Cancellation,
    error::AppError,
    report::Outcome,
};
use clap::Subcommand;
use model::*;
use std::{collections::BTreeSet, path::PathBuf};
#[derive(Clone, Subcommand)]
pub enum OffboardCommand {
    Assess {
        person: String,
        #[arg(long)]
        snapshot: Option<PathBuf>,
        #[arg(long)]
        annotations: Option<PathBuf>,
        #[arg(long)]
        html: Option<PathBuf>,
    },
    Plan {
        person: String,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        snapshot: Option<PathBuf>,
        #[arg(long)]
        annotations: Option<PathBuf>,
        #[arg(long, default_value_t = 24)]
        expires_in_hours: u32,
        #[arg(long)]
        html: Option<PathBuf>,
    },
    Verify {
        plan: PathBuf,
        #[arg(long)]
        snapshot: Option<PathBuf>,
        #[arg(long)]
        html: Option<PathBuf>,
    },
}
fn report(operation: Operation, mode: Mode, assessment: Assessment) -> Result<Report, AppError> {
    Ok(Report{verification_sources:vec![],assessment_origin:AssessmentOrigin::CurrentCapture,format:"permesh_offboard_report".into(),format_version:1,operation,mode,generated_at:crate::report::now()?,complete:assessment.gaps.is_empty(),gaps:assessment.gaps.clone(),assessment,checks:vec![],plan_sha256:None,limitations:vec!["Advisory review only. No remote mutations are performed.".into(),"No longer observed means absent within comparable visible scope, never proof of revoked or denied access.".into(),"Organizational ownership assertions are not provider facts. Review transfers before account changes.".into(),"Invitation, token, session and secret-read history are unsupported in these observations; verify manually through appropriate systems.".into()]})
}
async fn capture(
    cli: &Cli,
    snapshot: &Option<PathBuf>,
    blocking: &BlockingPool,
    cancel: &Cancellation,
) -> Result<(Artifact, Mode, u8), AppError> {
    if let Some(path) = snapshot {
        let path = path.clone();
        let artifact = blocking.run(move || artifact::load(&path)).await??;
        let code = if artifact.providers.iter().all(|p| p.state == State::Failed) {
            3
        } else if artifact.complete() {
            0
        } else {
            4
        };
        Ok((artifact, Mode::SnapshotReplay, code))
    } else {
        let (outcome, artifact) = crate::snapshot_command::capture(cli, blocking, cancel).await?;
        Ok((artifact, Mode::FreshDiscovery, outcome.code))
    }
}
fn prepared(
    a: Artifact,
    mode: Mode,
    person: String,
    annotations: Option<Annotations>,
    hours: Option<u32>,
) -> Result<(Report, Option<Plan>), AppError> {
    let mut assessment = assess(&a, &person, annotations.as_ref())?;
    let mut report = report(
        if hours.is_some() {
            Operation::Plan
        } else {
            Operation::Assess
        },
        mode,
        assessment.clone(),
    )?;
    let plan = if let Some(hours) = hours {
        let expires = (evidence_start(a.providers.iter())?
            + time::Duration::hours(i64::from(hours)))
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|_| invalid())?;
        if time(&expires)? <= time(&report.generated_at)? {
            return Err(AppError::input(
                "Evidence is too old for this plan freshness policy; capture fresh observations",
            ));
        }
        assessment
            .gaps
            .retain(|g| g.reason != GapReason::PlanExpired);
        report
            .assessment
            .gaps
            .retain(|g| g.reason != GapReason::PlanExpired);
        report.gaps.retain(|g| g.reason != GapReason::PlanExpired);
        report.complete = report.gaps.is_empty();
        let plan = Plan {
            max_age_hours: hours,
            format: "permesh_offboard_plan".into(),
            format_version: 1,
            created_at: report.generated_at.clone(),
            expires_at: expires,
            mode,
            source_snapshot_sha256: hash(&a)?,
            identity_context_sha256: hash(&a.identity)?,
            assessment,
        };
        plan.validate()?;
        report.plan_sha256 = Some(hash(&plan)?);
        Some(plan)
    } else {
        None
    };
    report.validate()?;
    Ok((report, plan))
}
fn cancelled(cancel: &Cancellation) -> Result<(), AppError> {
    if cancel.is_cancelled() {
        Err(AppError::new(130, "Cancelled"))
    } else {
        Ok(())
    }
}
pub async fn run(
    cli: &Cli,
    command: &OffboardCommand,
    blocking: &BlockingPool,
    cancel: &Cancellation,
) -> Result<Outcome, AppError> {
    cancelled(cancel)?;
    let destinations: Vec<&PathBuf> = match command {
        OffboardCommand::Plan {
            output,
            html,
            expires_in_hours,
            ..
        } => {
            if *expires_in_hours == 0 || *expires_in_hours > 168 {
                return Err(AppError::input(
                    "Plan freshness must be 1 through 168 hours",
                ));
            }
            std::iter::once(output).chain(html.iter()).collect()
        }
        OffboardCommand::Assess { html, .. } | OffboardCommand::Verify { html, .. } => {
            html.iter().collect()
        }
    };
    let mut targets = BTreeSet::new();
    for path in destinations {
        artifact::check_destination(path, false)?;
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| std::path::Path::new("."));
        let parent = parent
            .canonicalize()
            .map_err(|_| AppError::input("Report destination directory must exist"))?;
        if !targets.insert(parent.join(path.file_name().ok_or_else(invalid)?)) {
            return Err(AppError::input(
                "Plan and HTML outputs require different destinations",
            ));
        }
    }
    let (report, html, collection_code) = match command {
        OffboardCommand::Assess {
            person,
            snapshot,
            annotations,
            html,
        }
        | OffboardCommand::Plan {
            person,
            snapshot,
            annotations,
            html,
            ..
        } => {
            let annotations = if let Some(path) = annotations {
                let path = path.clone();
                Some(
                    blocking
                        .run(move || artifact::load_document::<Annotations>(&path))
                        .await??,
                )
            } else {
                None
            };
            cancelled(cancel)?;
            let (a, mode, code) = capture(cli, snapshot, blocking, cancel).await?;
            if code == 3 {
                return Err(AppError::new(
                    3,
                    "No provider returned usable observations for an offboarding target",
                ));
            }
            let hours = match command {
                OffboardCommand::Plan {
                    expires_in_hours, ..
                } => Some(*expires_in_hours),
                _ => None,
            };
            let person = person.clone();
            let (report, plan) = blocking
                .run(move || prepared(a, mode, person, annotations, hours))
                .await??;
            cancelled(cancel)?;
            if let (Some(plan), OffboardCommand::Plan { output, .. }) = (plan, command) {
                let output = output.clone();
                let token = cancel.clone();
                blocking
                    .run(move || artifact::write_document(&output, &plan, &token))
                    .await??;
            }
            (report, html.clone(), code)
        }
        OffboardCommand::Verify {
            plan,
            snapshot,
            html,
        } => {
            let path = plan.clone();
            let plan = blocking
                .run(move || {
                    let plan: Plan = artifact::load_document(&path)?;
                    plan.validate()?;
                    Ok::<_, AppError>(plan)
                })
                .await??;
            cancelled(cancel)?;
            let (a, mode, code) = capture(cli, snapshot, blocking, cancel).await?;
            let report = blocking.run(move || verify(&plan, &a, mode)).await??;
            cancelled(cancel)?;
            (report, html.clone(), code)
        }
    };
    let html_requested = html.is_some();
    let (outcome, html_bytes) = blocking
        .run(move || {
            report.validate()?;
            let html_bytes = if html_requested {
                Some(view::html(&report)?)
            } else {
                None
            };
            let mut outcome = Outcome::new(
                match report.operation {
                    Operation::Assess => "offboard_assess",
                    Operation::Plan => "offboard_plan",
                    Operation::Verify => "offboard_verify",
                },
                serde_json::to_value(&report).map_err(|_| invalid())?,
            )?;
            outcome.report.complete = report.complete;
            outcome.code = if collection_code == 3 {
                3
            } else if report.complete {
                0
            } else {
                4
            };
            Ok::<_, AppError>((outcome, html_bytes))
        })
        .await??;
    cancelled(cancel)?;
    if let (Some(path), Some(bytes)) = (html, html_bytes) {
        let token = cancel.clone();
        blocking
            .run(move || artifact::write_bytes(&path, &bytes, &token))
            .await??;
    }
    Ok(outcome)
}
pub fn write(out: &mut dyn std::io::Write, result: &serde_json::Value) -> std::io::Result<()> {
    view::write(out, result)
}
