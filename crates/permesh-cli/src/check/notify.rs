// SPDX-License-Identifier: MIT
//! Explicit check notifications (ADR 0032). One bounded attempt per channel.
//! Failures are reported, never retried, and never change the access result.
use super::report::{CheckReport, Delivery, summary_markdown};
use crate::{blocking::BlockingPool, cancellation::Cancellation};
use permesh_config::{Channel, Notification};
use permesh_secrets::{SecretRef, SecretResolver};
use serde_json::{Value, json};
use std::time::Duration;

pub const DELIVERY_TIMEOUT: Duration = Duration::from_secs(10);

/// Send only when a selected kind has a nonzero count (any kind when `on` is empty,
/// except report-only profile kinds, which must be selected explicitly).
pub fn selected(n: &Notification, report: &CheckReport) -> bool {
    if n.on.is_empty() {
        report.counts.iter().any(|(k, c)| {
            *c > 0 && (report.profiles.enforced() || !super::report::PROFILE_KINDS.contains(k))
        })
    } else {
        n.on.iter().any(|k| report.count(*k) > 0)
    }
}

fn detail_lines(report: &CheckReport) -> String {
    let mut out = String::from("\nDetails\n");
    for d in &report.departed {
        out.push_str(&format!(
            "- Departed with access: {} ({})\n",
            d["login"].as_str().unwrap_or_default(),
            d["instance"].as_str().unwrap_or_default()
        ));
    }
    if let super::report::Changes::Compared {
        new_admins,
        removed_admins,
        ..
    } = &report.changes
    {
        for (label, list) in [("New admin", new_admins), ("Removed admin", removed_admins)] {
            for a in list {
                out.push_str(&format!(
                    "- {label}: {} on {}\n",
                    a["login"].as_str().unwrap_or_default(),
                    a["resource_name"].as_str().unwrap_or_default()
                ));
            }
        }
    }
    for f in &report.findings {
        out.push_str(&format!(
            "- Policy {}: account {} on resource {}\n",
            f["rule"].as_str().unwrap_or_default(),
            f["account"].as_str().unwrap_or_default(),
            f["resource"].as_str().unwrap_or_default()
        ));
    }
    for f in report.profiles.findings() {
        let via: Vec<&str> = f["via"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        let target = match f["group"].as_str() {
            Some(group) => format!("member of {group}"),
            None => format!(
                "{} on {}",
                f["role"].as_str().unwrap_or_default(),
                f["resource"].as_str().unwrap_or_default()
            ),
        };
        out.push_str(&format!(
            "- {}: {} {target} ({}){}\n",
            if f["kind"] == "missing_access" {
                "Missing access"
            } else {
                "Unexplained access"
            },
            f["identity"].as_str().unwrap_or_default(),
            f["instance"].as_str().unwrap_or_default(),
            if via.is_empty() {
                String::new()
            } else {
                format!(" via {}", via.join(" > "))
            }
        ));
    }
    out
}

fn text(n: &Notification, report: &CheckReport) -> String {
    let mut text = summary_markdown(report);
    if n.detail {
        text.push_str(&detail_lines(report));
    }
    text
}

pub fn payload(n: &Notification, report: &CheckReport) -> Value {
    match &n.channel {
        Channel::SlackWebhook { .. } | Channel::TeamsWebhook { .. } => {
            json!({"text": text(n, report)})
        }
        Channel::Webhook { .. } | Channel::Email { .. } => {
            let mut body = json!({
                "check_report_version": report.check_report_version,
                "status": report.status(),
                "complete": report.complete,
                "counts": report.counts,
                "changes": report.changes.state(),
                "run_url": report.run_url,
            });
            if n.detail {
                body["departed"] = json!(report.departed);
                body["change_details"] = json!(report.changes);
                body["findings"] = json!(report.findings);
                body["profile_findings"] = json!(report.profiles.findings());
            }
            body
        }
    }
}

/// Webhooks require https; plain http only to a loopback host.
pub fn endpoint(value: &str) -> Result<reqwest::Url, &'static str> {
    let url = reqwest::Url::parse(value).map_err(|_| "endpoint is not a valid URL")?;
    if !url.username().is_empty() || url.password().is_some() {
        return Err("endpoint must not embed credentials");
    }
    match (url.scheme(), url.host_str()) {
        ("https", Some(_)) => Ok(url),
        ("http", Some("127.0.0.1" | "[::1]" | "localhost")) => Ok(url),
        _ => Err("endpoint must use https"),
    }
}

fn delivery(id: &str, delivered: bool, skipped: bool, reason: Option<&str>) -> Delivery {
    Delivery {
        id: id.to_owned(),
        delivered,
        skipped,
        reason: reason.map(str::to_owned),
    }
}

async fn resolve(reference: &str, pool: &BlockingPool) -> Result<String, &'static str> {
    let reference = SecretRef::parse(reference).map_err(|_| "invalid credential reference")?;
    pool.run(move || SecretResolver.resolve(&reference))
        .await
        .map_err(|_| "credential reference could not be resolved")?
        .map(|secret| secret.expose().to_owned())
        .map_err(|_| "credential reference could not be resolved")
}

async fn post(url: reqwest::Url, body: &Value) -> Result<(), &'static str> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .connect_timeout(Duration::from_secs(5))
        .timeout(DELIVERY_TIMEOUT)
        .build()
        .map_err(|_| "cannot build HTTP client")?;
    let response = client.post(url).json(body).send().await.map_err(|e| {
        if e.is_timeout() {
            "endpoint timed out"
        } else {
            "endpoint could not be reached"
        }
    })?;
    if response.status().is_success() {
        Ok(())
    } else {
        Err("endpoint rejected the notification")
    }
}

async fn send(
    n: &Notification,
    report: &CheckReport,
    pool: &BlockingPool,
) -> Result<(), &'static str> {
    match &n.channel {
        Channel::SlackWebhook { url }
        | Channel::TeamsWebhook { url }
        | Channel::Webhook { url } => {
            let url = endpoint(&resolve(url, pool).await?)?;
            post(url, &payload(n, report)).await
        }
        Channel::Email { .. } => send_email(n, report, pool).await,
    }
}

/// SMTP always uses TLS: implicit on 465, required STARTTLS everywhere else.
#[derive(Debug, PartialEq, Eq)]
pub enum TlsMode {
    Implicit,
    StartTlsRequired,
}
pub fn tls_mode(port: u16) -> TlsMode {
    if port == 465 {
        TlsMode::Implicit
    } else {
        TlsMode::StartTlsRequired
    }
}

fn subject(report: &CheckReport) -> String {
    match report.status() {
        "incomplete" => "Permesh check: incomplete".into(),
        "clean" => "Permesh check: clean".into(),
        _ => format!("Permesh check: {} findings", report.review_items()),
    }
}

pub fn email_message(
    n: &Notification,
    report: &CheckReport,
) -> Result<lettre::Message, &'static str> {
    let Channel::Email { from, to, .. } = &n.channel else {
        return Err("not an email channel");
    };
    let mut builder = lettre::Message::builder()
        .from(from.parse().map_err(|_| "invalid email sender address")?)
        .subject(subject(report))
        .header(lettre::message::header::ContentType::TEXT_PLAIN);
    for recipient in to {
        builder = builder.to(recipient
            .parse()
            .map_err(|_| "invalid email recipient address")?);
    }
    builder
        .body(text(n, report))
        .map_err(|_| "cannot build email message")
}

async fn send_email(
    n: &Notification,
    report: &CheckReport,
    pool: &BlockingPool,
) -> Result<(), &'static str> {
    use lettre::{AsyncSmtpTransport, AsyncTransport, Tokio1Executor};
    let Channel::Email {
        smtp_host,
        smtp_port,
        username,
        password,
        ..
    } = &n.channel
    else {
        return Err("not an email channel");
    };
    let message = email_message(n, report)?;
    let password = resolve(password, pool).await?;
    let builder = match tls_mode(*smtp_port) {
        TlsMode::Implicit => AsyncSmtpTransport::<Tokio1Executor>::relay(smtp_host),
        TlsMode::StartTlsRequired => {
            AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(smtp_host)
        }
    }
    .map_err(|_| "invalid SMTP host")?;
    let transport = builder
        .port(*smtp_port)
        .credentials(lettre::transport::smtp::authentication::Credentials::new(
            username.clone(),
            password,
        ))
        .timeout(Some(DELIVERY_TIMEOUT))
        .build();
    transport.send(message).await.map(|_| ()).map_err(|e| {
        if e.is_timeout() {
            "endpoint timed out"
        } else {
            "SMTP server could not be reached or rejected the message"
        }
    })
}

pub async fn deliver_all(
    config: &[Notification],
    report: &CheckReport,
    enabled: bool,
    pool: &BlockingPool,
    cancel: &Cancellation,
) -> Vec<Delivery> {
    let mut results = Vec::new();
    for n in config {
        if !enabled || !selected(n, report) {
            results.push(delivery(&n.id, false, true, None));
            continue;
        }
        if cancel.is_cancelled() {
            results.push(delivery(&n.id, false, false, Some("cancelled")));
            continue;
        }
        let outcome = tokio::select! {
            biased;
            () = cancel.cancelled() => Err("cancelled"),
            result = tokio::time::timeout(DELIVERY_TIMEOUT + Duration::from_secs(2), send(n, report, pool)) => {
                result.unwrap_or(Err("endpoint timed out"))
            }
        };
        results.push(match outcome {
            Ok(()) => delivery(&n.id, true, false, None),
            Err(reason) => delivery(&n.id, false, false, Some(reason)),
        });
    }
    results
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::check::report::{Changes, Files};
    use permesh_config::FindingKind;

    fn report() -> CheckReport {
        let mut counts: std::collections::BTreeMap<_, _> = crate::check::report::ALL_KINDS
            .iter()
            .filter(|k| !crate::check::report::DIFF_KINDS.contains(k))
            .map(|k| (*k, 0))
            .collect();
        counts.insert(FindingKind::DepartedWithAccess, 1);
        counts.insert(FindingKind::PolicyFinding, 1);
        CheckReport {
            check_report_version: 1,
            complete: true,
            departed_assessed: true,
            counts,
            providers: vec![],
            changes: Changes::NoBaseline,
            departed: vec![
                json!({"instance":"demo","account":"102","login":"former-dev","reason":"inactive_identity","access_paths":1}),
            ],
            findings: vec![
                json!({"rule":"external_privileged","account":"104","resource":"acme/payments-api","state":"finding"}),
            ],
            policy: json!({}),
            profiles: crate::check::report::Profiles::NotConfigured,
            notifications: vec![],
            run_url: None,
            files: Files {
                snapshot: String::new(),
                report: String::new(),
                summary: String::new(),
            },
        }
    }
    fn notification(channel: Channel, detail: bool, on: Vec<FindingKind>) -> Notification {
        Notification {
            id: "n".into(),
            channel,
            on,
            detail,
        }
    }
    fn channels() -> Vec<Channel> {
        let url = "env://HOOK".to_owned();
        vec![
            Channel::SlackWebhook { url: url.clone() },
            Channel::TeamsWebhook { url: url.clone() },
            Channel::Webhook { url },
        ]
    }

    fn profiled(enforce: bool) -> CheckReport {
        let mut report = report();
        report.counts.insert(FindingKind::UnexplainedAccess, 1);
        report.counts.insert(FindingKind::MissingAccess, 1);
        report.profiles = crate::check::report::Profiles::Compared {
            enforce,
            complete: true,
            value: json!({"identities":[{"identity":"bob@example.com","missing":[{"state":"finding","entry":{"kind":"group","instance":"demo","group":"team-10"},"reason":"not_observed"},{"state":"cannot_tell","entry":{"kind":"group","instance":"demo","group":"team-11"},"reason":"instance_not_observable"}],"unexplained":[{"state":"finding","instance":"demo","account":"101","resource":"repo-21","resource_name":"acme/infrastructure","role":"Admin","grant_id":"grant-3","via":["team-10"]},{"state":"excepted","instance":"demo","account":"101","resource":"repo-20","role":"Read","grant_id":"grant-9","via":[]}]}]}),
        };
        report
    }

    #[test]
    fn report_only_profile_counts_are_not_review_items() {
        assert_eq!(profiled(false).review_items(), report().review_items());
        assert_eq!(profiled(true).review_items(), report().review_items() + 2);
    }

    #[test]
    fn report_only_profile_counts_do_not_trigger_unfiltered_notifications() {
        let mut quiet = profiled(false);
        for kind in [FindingKind::DepartedWithAccess, FindingKind::PolicyFinding] {
            quiet.counts.insert(kind, 0);
        }
        let all = notification(channels().remove(0), false, vec![]);
        assert!(!selected(&all, &quiet));
        let explicit = notification(
            channels().remove(0),
            false,
            vec![FindingKind::UnexplainedAccess],
        );
        assert!(selected(&explicit, &quiet));
    }

    /// The generic webhook and email JSON body is stable (ADR 0036).
    #[test]
    fn webhook_payload_fixture() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/stable/webhook-payload.json"
        );
        let hook = channels().remove(2);
        let value = json!({
            "default": payload(&notification(hook.clone(), false, vec![]), &profiled(false)),
            "detail": payload(&notification(hook, true, vec![]), &profiled(false)),
        });
        if std::env::var_os("PERMESH_BLESS").is_some() {
            std::fs::write(path, serde_json::to_string_pretty(&value).unwrap() + "\n").unwrap();
        }
        let expected: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(value, expected, "webhook payload is a stable format");
    }

    #[test]
    fn detail_payload_lists_profile_findings_without_names_in_summary() {
        let hook = channels().remove(2);
        let body = payload(&notification(hook.clone(), true, vec![]), &profiled(false));
        assert_eq!(
            body["profile_findings"],
            json!([
                {"identity":"bob@example.com","kind":"missing_access","instance":"demo","group":"team-10","via":[]},
                {"identity":"bob@example.com","kind":"unexplained_access","instance":"demo","resource":"repo-21","role":"Admin","via":["team-10"]}
            ])
        );
        let plain = payload(&notification(hook, false, vec![]), &profiled(false)).to_string();
        assert!(!plain.contains("bob@example.com"), "{plain}");
        let slack = payload(
            &notification(channels().remove(0), true, vec![]),
            &profiled(false),
        )
        .to_string();
        assert!(slack.contains("bob@example.com"), "{slack}");
        assert!(slack.contains("via team-10"), "{slack}");
    }

    #[test]
    fn default_payloads_contain_no_names() {
        for channel in channels() {
            let body = payload(&notification(channel, false, vec![]), &report()).to_string();
            assert!(!body.contains("former-dev"), "{body}");
            assert!(!body.contains("acme/payments-api"), "{body}");
            assert!(
                body.contains("Departed people with access: 1")
                    || body.contains("\"departed_with_access\":1"),
                "{body}"
            );
        }
    }

    #[test]
    fn detail_payload_includes_names() {
        for channel in channels() {
            let body = payload(&notification(channel, true, vec![]), &report()).to_string();
            assert!(body.contains("former-dev"), "{body}");
        }
    }

    #[test]
    fn detail_payload_keeps_change_state() {
        let body = payload(&notification(channels().remove(2), true, vec![]), &report());
        assert_eq!(body["changes"]["state"], "no_baseline");
        assert_eq!(body["change_details"], "no_baseline");
    }

    #[test]
    fn on_filter_selects_only_matching_kinds() {
        let r = report();
        assert!(!selected(
            &notification(channels().remove(0), false, vec![FindingKind::NewAdmin]),
            &r
        ));
        assert!(selected(
            &notification(
                channels().remove(0),
                false,
                vec![FindingKind::DepartedWithAccess]
            ),
            &r
        ));
        assert!(selected(
            &notification(channels().remove(0), false, vec![]),
            &r
        ));
    }

    #[test]
    fn rejects_non_https_non_loopback_url() {
        assert_eq!(
            endpoint("http://example.com/hook").err(),
            Some("endpoint must use https")
        );
        assert_eq!(
            endpoint("https://user:pw@example.com/hook").err(),
            Some("endpoint must not embed credentials")
        );
        assert!(endpoint("https://hooks.example.com/x").is_ok());
        assert!(endpoint("http://127.0.0.1:9/x").is_ok());
        assert!(endpoint("ftp://example.com").is_err());
    }

    fn email(detail: bool) -> Notification {
        notification(
            Channel::Email {
                smtp_host: "smtp.example.com".into(),
                smtp_port: 587,
                username: "permesh".into(),
                password: "env://SMTP".into(),
                from: "Permesh <permesh@example.com>".into(),
                to: vec!["it@example.com".into()],
            },
            detail,
            vec![],
        )
    }

    #[test]
    fn email_message_has_counts_subject_and_no_names() {
        let message = email_message(&email(false), &report()).unwrap();
        let raw = String::from_utf8(message.formatted()).unwrap();
        assert!(raw.contains("Subject: Permesh check: 2 findings"), "{raw}");
        assert!(raw.contains("Departed people with access: 1"), "{raw}");
        assert!(!raw.contains("former-dev"), "{raw}");
        let detailed =
            String::from_utf8(email_message(&email(true), &report()).unwrap().formatted()).unwrap();
        assert!(detailed.contains("former-dev"));
    }

    #[test]
    fn email_never_uses_plaintext_transport() {
        assert_eq!(tls_mode(465), TlsMode::Implicit);
        for port in [25, 587, 2525] {
            assert_eq!(tls_mode(port), TlsMode::StartTlsRequired);
        }
    }

    #[test]
    fn invalid_email_addresses_fail_without_echoing_them() {
        let mut n = email(false);
        if let Channel::Email { to, .. } = &mut n.channel {
            *to = vec!["not an address".into()];
        }
        let error = email_message(&n, &report()).err().unwrap();
        assert!(!error.contains("not an address"));
    }

    #[test]
    fn default_generic_payload_says_whether_changes_were_compared() {
        let body = payload(
            &notification(channels().remove(2), false, vec![]),
            &report(),
        );
        assert_eq!(
            body["changes"],
            json!({"state": "no_baseline", "reasons": []})
        );
        assert!(body["counts"].get("new_admin").is_none(), "{body}");
    }
}
