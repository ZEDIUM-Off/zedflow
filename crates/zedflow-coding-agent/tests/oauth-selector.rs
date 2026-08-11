use zedflow_coding_agent::oauth_selector::{
    AuthSelectorProvider, AuthSelectorProviderType, OAuthSelector,
};
#[test]
fn oauth_selector_fuzzy_filters_provider_names() {
    let mut selector = OAuthSelector::new(vec![AuthSelectorProvider {
        id: "github".into(),
        name: "GitHub".into(),
        auth_type: AuthSelectorProviderType::OAuth,
    }]);
    selector.filter("gh");
    assert_eq!(selector.selected_provider().unwrap().id, "github");
}
