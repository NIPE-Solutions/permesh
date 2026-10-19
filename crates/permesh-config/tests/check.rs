// SPDX-License-Identifier: MIT
#![allow(clippy::unwrap_used)]
use permesh_config::{Channel, Config, FindingKind};

const BASE: &str = "version: 1\norganization: {name: Test}\nproviders: [{id: demo, type: demo}]\n";

fn parse(extra: &str) -> Result<Config, permesh_config::Error> {
    Config::from_bytes(format!("{BASE}{extra}").as_bytes())
}

#[test]
fn check_section_is_optional() {
    let config = Config::from_bytes(BASE.as_bytes()).unwrap();
    assert!(config.check.is_none());
    let yaml = permesh_config::to_yaml(&config).unwrap();
    assert!(!yaml.contains("check"));
}

#[test]
fn parses_all_channel_types() {
    let config = parse(
        "check:\n  notifications:\n    - {id: slack, type: slack_webhook, url: 'env://SLACK'}\n    - {id: teams, type: teams_webhook, url: 'env://TEAMS', on: [new_admin], detail: true}\n    - {id: hook, type: webhook, url: 'keychain://demo/hook'}\n    - {id: mail, type: email, smtp_host: smtp.example.com, smtp_port: 587, username: permesh, password: 'env://SMTP', from: 'permesh@example.com', to: ['it@example.com']}\n",
    )
    .unwrap();
    let n = &config.check.as_ref().unwrap().notifications;
    assert_eq!(n.len(), 4);
    assert!(matches!(n[0].channel, Channel::SlackWebhook { .. }));
    assert!(n[0].on.is_empty() && !n[0].detail);
    assert!(matches!(n[1].channel, Channel::TeamsWebhook { .. }));
    assert_eq!(n[1].on, vec![FindingKind::NewAdmin]);
    assert!(n[1].detail);
    assert!(matches!(n[2].channel, Channel::Webhook { .. }));
    assert!(matches!(
        n[3].channel,
        Channel::Email { smtp_port: 587, .. }
    ));
    let round = Config::from_bytes(permesh_config::to_yaml(&config).unwrap().as_bytes()).unwrap();
    assert_eq!(round.check.unwrap().notifications.len(), 4);
}

#[test]
fn rejects_inline_secret_urls() {
    let error = parse(
        "check:\n  notifications:\n    - {id: slack, type: slack_webhook, url: 'https://hooks.slack.com/services/X'}\n",
    )
    .err()
    .unwrap();
    assert!(!error.to_string().contains("hooks.slack.com"));
    let error = parse(
        "check:\n  notifications:\n    - {id: mail, type: email, smtp_host: h, smtp_port: 587, username: u, password: 'hunter2', from: f, to: [t]}\n",
    )
    .err()
    .unwrap();
    assert!(!error.to_string().contains("hunter2"));
}

#[test]
fn rejects_duplicate_ids_unknown_fields_and_bad_kinds() {
    for extra in [
        "check:\n  notifications:\n    - {id: a, type: webhook, url: 'env://A'}\n    - {id: a, type: webhook, url: 'env://B'}\n",
        "check:\n  notifications:\n    - {id: a, type: webhook, url: 'env://A', retries: 3}\n",
        "check:\n  notifications:\n    - {id: a, type: webhook, url: 'env://A', on: [everything]}\n",
        "check:\n  notifications:\n    - {id: 'a b', type: webhook, url: 'env://A'}\n",
        "check:\n  extra: 1\n  notifications: []\n",
    ] {
        assert!(parse(extra).is_err(), "{extra}");
    }
}

#[test]
fn rejects_more_than_16_notifications_and_empty_recipient_list() {
    let many: String = (0..17)
        .map(|i| format!("    - {{id: n{i}, type: webhook, url: 'env://N{i}'}}\n"))
        .collect();
    assert!(parse(&format!("check:\n  notifications:\n{many}")).is_err());
    assert!(
        parse("check:\n  notifications:\n    - {id: mail, type: email, smtp_host: h, smtp_port: 587, username: u, password: 'env://P', from: f, to: []}\n")
            .is_err()
    );
}
