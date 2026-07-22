use std::process::Command;

use zedflow_agent::ThinkingLevel;
use zedflow_coding_agent::{
    defaults::DEFAULT_THINKING_LEVEL,
    diagnostics::{ResourceCollision, ResourceDiagnostic, ResourceDiagnosticType, ResourceType},
    experimental::are_experimental_features_enabled,
};

#[test]
fn static_contracts_match_pi() {
    assert_eq!(DEFAULT_THINKING_LEVEL, ThinkingLevel::Medium);

    let collision = ResourceCollision {
        resource_type: ResourceType::Skill,
        name: "review".into(),
        winner_path: "/local/review".into(),
        loser_path: "/npm/review".into(),
        winner_source: Some("local".into()),
        loser_source: None,
    };
    let diagnostic = ResourceDiagnostic {
        r#type: ResourceDiagnosticType::Collision,
        message: "duplicate skill".into(),
        path: None,
        collision: Some(collision.clone()),
    };

    assert_eq!(diagnostic.collision, Some(collision));
}

#[test]
fn experimental_gate_matches_only_one() {
    for (value, expected) in [
        (None, false),
        (Some(""), false),
        (Some("1"), true),
        (Some("0"), false),
        (Some("true"), false),
    ] {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .arg("--exact")
            .arg("experimental_gate_child")
            .arg("--nocapture")
            .env("ZEDFLOW_EXPECT_EXPERIMENTAL", expected.to_string());
        match value {
            Some(value) => command.env("PI_EXPERIMENTAL", value),
            None => command.env_remove("PI_EXPERIMENTAL"),
        };

        assert!(command.status().unwrap().success());
    }
}

#[test]
fn experimental_gate_child() {
    let Ok(expected) = std::env::var("ZEDFLOW_EXPECT_EXPERIMENTAL") else {
        return;
    };
    assert_eq!(are_experimental_features_enabled(), expected == "true");
}
