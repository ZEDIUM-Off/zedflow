use std::collections::BTreeMap;
use zedflow_coding_agent::{
    auth_storage::{AuthCredential, AuthSource, AuthStatus},
    login_dialog::{LoginAction, LoginDialog, LoginView},
    oauth_selector::{
        AuthSelectorMode, AuthSelectorProvider, AuthSelectorProviderType, OAuthSelector,
        status_indicator,
    },
};

#[test]
fn login_actions_are_injected_and_cancel_is_terminal() {
    let mut dialog = LoginDialog::new("github", Some("GitHub"), None);
    assert_eq!(
        dialog.show_auth("https://example.test/auth", None),
        LoginAction::OpenBrowser("https://example.test/auth".into())
    );
    dialog.show_prompt("Paste code", Some("abc".into()));
    assert_eq!(
        dialog.submit("secret"),
        LoginAction::Submit("secret".into())
    );
    dialog.show_prompt("Again", None);
    assert_eq!(
        dialog.cancel(),
        LoginAction::Cancel {
            success: false,
            message: "Login cancelled".into()
        }
    );
    assert_eq!(dialog.submit("ignored"), LoginAction::None);
    assert!(dialog.is_cancelled());
}

#[test]
fn device_code_and_logout_empty_state_match_pi() {
    let mut dialog = LoginDialog::new("github", None, None);
    dialog.show_device_code("https://example.test/device", "ABCD");
    assert!(matches!(dialog.view, LoginView::DeviceCode { .. }));

    let selector = OAuthSelector::with_mode(AuthSelectorMode::Logout, vec![]);
    assert_eq!(
        selector.empty_message(),
        "No providers logged in. Use /login first."
    );
}

#[test]
fn provider_search_and_status_match_pi() {
    let mut selector = OAuthSelector::new(vec![AuthSelectorProvider {
        id: "openai".into(),
        name: "OpenAI".into(),
        auth_type: AuthSelectorProviderType::ApiKey,
    }]);
    selector.filter("api_key");
    assert_eq!(selector.selected_provider().unwrap().id, "openai");

    let oauth = AuthCredential::OAuth {
        refresh: "r".into(),
        access: "a".into(),
        expires: 1,
        extra: BTreeMap::new(),
    };
    assert_eq!(
        status_indicator(AuthSelectorProviderType::ApiKey, Some(&oauth), None),
        "subscription configured"
    );
    let status = AuthStatus {
        configured: true,
        source: Some(AuthSource::Environment),
        label: Some("OPENAI_API_KEY".into()),
    };
    assert_eq!(
        status_indicator(AuthSelectorProviderType::ApiKey, None, Some(&status)),
        "✓ env: OPENAI_API_KEY"
    );
}
