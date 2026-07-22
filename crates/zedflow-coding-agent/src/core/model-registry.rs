use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use serde::{Deserialize, Serialize};
use zedflow_ai::{
    auth::types::{AuthFuture, AuthLoginCallbacks, AuthResult, OAuthCredential},
    compat,
    types::{
        Api, Model, ModelCompat, ModelCost, ModelInput, SimpleStreamOptions, ThinkingLevelMap,
    },
    utils::oauth::index::{
        OAuthProviderInterface, register_oauth_provider, unregister_oauth_provider,
    },
};

use crate::{
    auth_storage::{AuthCredential, AuthSource, AuthStatus, AuthStorage},
    provider_display_names::provider_display_name,
    resolve_config_value::{
        get_config_value_env_var_names, is_command_config_value, is_config_value_configured,
        resolve_config_value_or_throw, resolve_config_value_uncached, resolve_headers_or_throw,
    },
    utils::json::strip_json_comments,
};

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ModelsConfig {
    providers: HashMap<String, ProviderConfig>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProviderConfig {
    name: Option<String>,
    base_url: Option<String>,
    api_key: Option<String>,
    api: Option<Api>,
    headers: Option<HashMap<String, String>>,
    compat: Option<ModelCompat>,
    auth_header: Option<bool>,
    models: Option<Vec<ModelDefinition>>,
    model_overrides: Option<HashMap<String, ModelOverride>>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ModelDefinition {
    id: String,
    name: Option<String>,
    api: Option<Api>,
    base_url: Option<String>,
    reasoning: Option<bool>,
    thinking_level_map: Option<ThinkingLevelMap>,
    input: Option<Vec<ModelInput>>,
    cost: Option<ModelCost>,
    context_window: Option<u64>,
    max_tokens: Option<u64>,
    headers: Option<HashMap<String, String>>,
    compat: Option<ModelCompat>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ModelOverride {
    name: Option<String>,
    reasoning: Option<bool>,
    thinking_level_map: Option<ThinkingLevelMap>,
    input: Option<Vec<ModelInput>>,
    cost: Option<PartialCost>,
    context_window: Option<u64>,
    max_tokens: Option<u64>,
    headers: Option<HashMap<String, String>>,
    compat: Option<ModelCompat>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PartialCost {
    input: Option<f64>,
    output: Option<f64>,
    cache_read: Option<f64>,
    cache_write: Option<f64>,
}

#[derive(Debug, Clone, Default)]
struct RequestConfig {
    api_key: Option<String>,
    headers: Option<HashMap<String, String>>,
    auth_header: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedRequestAuth {
    pub api_key: Option<String>,
    pub headers: Option<HashMap<String, String>>,
    pub env: Option<HashMap<String, String>>,
}

#[derive(Clone)]
pub struct ProviderConfigInput {
    pub name: Option<String>,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub api: Option<Api>,
    pub headers: Option<HashMap<String, String>>,
    pub auth_header: bool,
    pub models: Option<Vec<Model>>,
    pub stream_simple: Option<compat::ApiStreamSimpleFunction>,
    pub oauth: Option<Arc<dyn OAuthProviderInterface>>,
}

impl Default for ProviderConfigInput {
    fn default() -> Self {
        Self {
            name: None,
            base_url: None,
            api_key: None,
            api: None,
            headers: None,
            auth_header: false,
            models: None,
            stream_simple: None,
            oauth: None,
        }
    }
}

pub struct ModelRegistry {
    models: Vec<Model>,
    provider_request_configs: HashMap<String, RequestConfig>,
    model_request_headers: HashMap<String, HashMap<String, String>>,
    provider_display_names: HashMap<String, String>,
    registered_providers: HashMap<String, ProviderConfigInput>,
    load_error: Option<String>,
    pub auth_storage: AuthStorage,
    models_json_path: Option<PathBuf>,
}

impl ModelRegistry {
    pub fn create(auth_storage: AuthStorage, models_json_path: impl Into<PathBuf>) -> Self {
        Self::new(auth_storage, Some(models_json_path.into()))
    }
    pub fn in_memory(auth_storage: AuthStorage) -> Self {
        Self::new(auth_storage, None)
    }
    fn new(auth_storage: AuthStorage, models_json_path: Option<PathBuf>) -> Self {
        let mut value = Self {
            models: vec![],
            provider_request_configs: HashMap::new(),
            model_request_headers: HashMap::new(),
            provider_display_names: HashMap::new(),
            registered_providers: HashMap::new(),
            load_error: None,
            auth_storage,
            models_json_path,
        };
        value.load_models();
        value
    }
    pub fn refresh(&mut self) {
        for name in self.registered_providers.keys() {
            compat::unregister_api_providers(&format!("provider:{name}"));
            unregister_oauth_provider(name);
        }
        self.provider_request_configs.clear();
        self.model_request_headers.clear();
        self.provider_display_names.clear();
        self.load_error = None;
        self.load_models();
        for (name, config) in self.registered_providers.clone() {
            self.apply_provider_config(&name, &config);
        }
    }
    pub fn get_error(&self) -> Option<&str> {
        self.load_error.as_deref()
    }
    pub fn get_all(&self) -> &[Model] {
        &self.models
    }
    pub fn get_available(&self) -> Vec<&Model> {
        self.models
            .iter()
            .filter(|m| self.has_configured_auth(m))
            .collect()
    }
    pub fn find(&self, provider: &str, model_id: &str) -> Option<&Model> {
        self.models
            .iter()
            .find(|m| m.provider == provider && m.id == model_id)
    }

    fn load_models(&mut self) {
        let mut builtins = compat::get_models().unwrap_or_default();
        let Some(path) = self.models_json_path.clone() else {
            self.models = builtins;
            self.apply_oauth_model_modifiers();
            return;
        };
        if !path.exists() {
            self.models = builtins;
            self.apply_oauth_model_modifiers();
            return;
        }
        match self.load_custom_models(&path, &mut builtins) {
            Ok(custom) => {
                for model in custom {
                    if let Some(i) = builtins
                        .iter()
                        .position(|m| m.provider == model.provider && m.id == model.id)
                    {
                        builtins[i] = model
                    } else {
                        builtins.push(model)
                    }
                }
            }
            Err(error) => self.load_error = Some(error),
        }
        self.models = builtins;
        self.apply_oauth_model_modifiers();
    }

    fn apply_oauth_model_modifiers(&mut self) {
        for oauth in self.auth_storage.oauth_providers() {
            if let Some(credentials) = oauth_credentials(self.auth_storage.get(oauth.id())) {
                self.models = oauth.modify_models(&self.models, &credentials);
            }
        }
    }

    fn load_custom_models(
        &mut self,
        path: &Path,
        builtins: &mut [Model],
    ) -> Result<Vec<Model>, String> {
        let content = fs::read_to_string(path).map_err(|e| {
            format!(
                "Failed to load models.json: {e}\n\nFile: {}",
                path.display()
            )
        })?;
        let config: ModelsConfig =
            serde_json::from_str(&strip_json_comments(&content)).map_err(|e| {
                format!(
                    "Invalid models.json schema:\n  - {e}\n\nFile: {}",
                    path.display()
                )
            })?;
        let builtin_providers: HashSet<_> = builtins.iter().map(|m| m.provider.clone()).collect();
        let mut custom = vec![];
        for (provider, cfg) in config.providers {
            if let Some(name) = &cfg.name {
                self.provider_display_names
                    .insert(provider.clone(), name.clone());
            }
            let defs = cfg.models.as_deref().unwrap_or_default();
            let has_overrides = cfg.model_overrides.as_ref().is_some_and(|v| !v.is_empty());
            if defs.is_empty()
                && cfg.base_url.is_none()
                && cfg.headers.is_none()
                && cfg.compat.is_none()
                && !has_overrides
            {
                return Err(format!(
                    "Failed to load models.json: Provider {provider}: must specify \"baseUrl\", \"headers\", \"compat\", \"modelOverrides\", or \"models\".\n\nFile: {}",
                    path.display()
                ));
            }
            if !defs.is_empty() && !builtin_providers.contains(&provider) && cfg.base_url.is_none()
            {
                return Err(format!(
                    "Failed to load models.json: Provider {provider}: \"baseUrl\" is required when defining custom models.\n\nFile: {}",
                    path.display()
                ));
            }
            self.store_request_config(
                &provider,
                cfg.api_key.clone(),
                cfg.headers.clone(),
                cfg.auth_header.unwrap_or(false),
            );
            for model in builtins.iter_mut().filter(|m| m.provider == provider) {
                if let Some(url) = &cfg.base_url {
                    model.base_url.clone_from(url);
                }
                model.compat = merge_compat(model.compat.clone(), cfg.compat.clone());
                if let Some(overrides) = &cfg.model_overrides {
                    if let Some(value) = overrides.get(&model.id) {
                        apply_override(model, value);
                        self.store_model_headers(&provider, &model.id, value.headers.clone());
                    }
                }
            }
            let defaults = builtins
                .iter()
                .find(|m| m.provider == provider)
                .map(|m| (m.api.clone(), m.base_url.clone()));
            for def in defs {
                let api = def.api.clone().or_else(|| cfg.api.clone()).or_else(|| defaults.as_ref().map(|v| v.0.clone())).ok_or_else(|| format!("Failed to load models.json: Provider {provider}, model {}: no \"api\" specified. Set at provider or model level.\n\nFile: {}", def.id, path.display()))?;
                let base_url = def
                    .base_url
                    .clone()
                    .or_else(|| cfg.base_url.clone())
                    .or_else(|| defaults.as_ref().map(|v| v.1.clone()))
                    .unwrap();
                self.store_model_headers(&provider, &def.id, def.headers.clone());
                custom.push(Model {
                    id: def.id.clone(),
                    name: def.name.clone().unwrap_or_else(|| def.id.clone()),
                    api,
                    provider: provider.clone(),
                    base_url,
                    reasoning: def.reasoning.unwrap_or(false),
                    thinking_level_map: def.thinking_level_map.clone(),
                    input: def.input.clone().unwrap_or_else(|| vec![ModelInput::Text]),
                    cost: def.cost.clone().unwrap_or_default(),
                    context_window: def.context_window.unwrap_or(128000),
                    max_tokens: def.max_tokens.unwrap_or(16384),
                    headers: None,
                    compat: merge_compat(cfg.compat.clone(), def.compat.clone()),
                });
            }
        }
        Ok(custom)
    }

    fn store_request_config(
        &mut self,
        provider: &str,
        api_key: Option<String>,
        headers: Option<HashMap<String, String>>,
        auth_header: bool,
    ) {
        if api_key.is_some() || headers.is_some() || auth_header {
            self.provider_request_configs.insert(
                provider.into(),
                RequestConfig {
                    api_key,
                    headers,
                    auth_header,
                },
            );
        }
    }
    fn store_model_headers(
        &mut self,
        provider: &str,
        model: &str,
        headers: Option<HashMap<String, String>>,
    ) {
        let key = format!("{provider}:{model}");
        if let Some(headers) = headers.filter(|v| !v.is_empty()) {
            self.model_request_headers.insert(key, headers);
        } else {
            self.model_request_headers.remove(&key);
        }
    }
    pub fn has_configured_auth(&self, model: &Model) -> bool {
        self.auth_storage.has_auth(&model.provider)
            || self
                .provider_request_configs
                .get(&model.provider)
                .and_then(|v| v.api_key.as_deref())
                .is_some_and(|v| {
                    is_config_value_configured(
                        v,
                        self.auth_storage.get_provider_env(&model.provider).as_ref(),
                    )
                })
    }
    pub async fn get_api_key_and_headers(
        &mut self,
        model: &Model,
    ) -> Result<ResolvedRequestAuth, String> {
        let config = self.provider_request_configs.get(&model.provider).cloned();
        let env = self.auth_storage.get_provider_env(&model.provider);
        let api_key = match self.auth_storage.get_api_key(&model.provider, false).await {
            Some(v) => Some(v),
            None => config
                .as_ref()
                .and_then(|v| v.api_key.as_ref())
                .map(|v| {
                    resolve_config_value_or_throw(
                        v,
                        &format!("API key for provider \"{}\"", model.provider),
                        env.as_ref(),
                    )
                })
                .transpose()
                .map_err(|e| e.to_string())?,
        };
        let provider_headers = resolve_headers_or_throw(
            config.as_ref().and_then(|v| v.headers.as_ref()),
            &format!("provider \"{}\"", model.provider),
            env.as_ref(),
        )
        .map_err(|e| e.to_string())?;
        let key = format!("{}:{}", model.provider, model.id);
        let model_headers = resolve_headers_or_throw(
            self.model_request_headers.get(&key),
            &format!("model \"{}/{}\"", model.provider, model.id),
            env.as_ref(),
        )
        .map_err(|e| e.to_string())?;
        let mut headers = model.headers.clone().unwrap_or_default();
        headers.extend(provider_headers.unwrap_or_default());
        headers.extend(model_headers.unwrap_or_default());
        if config.as_ref().is_some_and(|v| v.auth_header) {
            let key = api_key
                .as_ref()
                .ok_or_else(|| format!("No API key found for \"{}\"", model.provider))?;
            headers.insert("Authorization".into(), format!("Bearer {key}"));
        }
        Ok(ResolvedRequestAuth {
            api_key,
            headers: (!headers.is_empty()).then_some(headers),
            env: env.filter(|v| !v.is_empty()),
        })
    }
    pub fn get_provider_auth_status(&self, provider: &str) -> AuthStatus {
        let status = self.auth_storage.get_auth_status(provider);
        if status.source.is_some() {
            return status;
        }
        let Some(key) = self
            .provider_request_configs
            .get(provider)
            .and_then(|v| v.api_key.as_deref())
        else {
            return status;
        };
        if is_command_config_value(key) {
            return AuthStatus {
                configured: true,
                source: Some(AuthSource::ModelsJsonCommand),
                label: None,
            };
        }
        let names = get_config_value_env_var_names(key);
        if !names.is_empty() {
            let configured = is_config_value_configured(
                key,
                self.auth_storage.get_provider_env(provider).as_ref(),
            );
            return AuthStatus {
                configured,
                source: configured.then_some(AuthSource::Environment),
                label: configured.then(|| names.join(", ")),
            };
        }
        AuthStatus {
            configured: true,
            source: Some(AuthSource::ModelsJsonKey),
            label: None,
        }
    }
    pub async fn get_api_key_for_provider(&mut self, provider: &str) -> Option<String> {
        if let Some(key) = self.auth_storage.get_api_key(provider, true).await {
            return Some(key);
        }
        self.provider_request_configs
            .get(provider)
            .and_then(|v| v.api_key.as_deref())
            .and_then(|v| {
                resolve_config_value_uncached(
                    v,
                    self.auth_storage.get_provider_env(provider).as_ref(),
                )
            })
    }
    pub fn is_using_oauth(&self, model: &Model) -> bool {
        matches!(
            self.auth_storage.get(&model.provider),
            Some(AuthCredential::OAuth { .. })
        )
    }
    pub fn get_provider_display_name<'a>(&'a self, provider: &'a str) -> &'a str {
        self.registered_providers
            .get(provider)
            .and_then(|v| v.name.as_deref())
            .or_else(|| {
                self.provider_display_names
                    .get(provider)
                    .map(String::as_str)
            })
            .unwrap_or_else(|| provider_display_name(provider))
    }
    pub fn register_provider(
        &mut self,
        name: &str,
        config: ProviderConfigInput,
    ) -> Result<(), String> {
        if config.stream_simple.is_some() && config.api.is_none() {
            return Err(format!(
                "Provider {name}: \"api\" is required when registering streamSimple."
            ));
        }
        if config.models.as_ref().is_some_and(|v| !v.is_empty()) && config.base_url.is_none() {
            return Err(format!(
                "Provider {name}: \"baseUrl\" is required when defining models."
            ));
        }
        if config.models.as_ref().is_some_and(|v| !v.is_empty())
            && config.api_key.is_none()
            && config.oauth.is_none()
        {
            return Err(format!(
                "Provider {name}: \"apiKey\" or \"oauth\" is required when defining models."
            ));
        }
        if let Some(models) = &config.models {
            for model in models {
                if model.api.is_empty() && config.api.is_none() {
                    return Err(format!(
                        "Provider {name}, model {}: no \"api\" specified.",
                        model.id
                    ));
                }
            }
        }
        self.apply_provider_config(name, &config);
        self.registered_providers
            .entry(name.into())
            .and_modify(|old| merge_provider_input(old, &config))
            .or_insert(config);
        Ok(())
    }
    pub fn unregister_provider(&mut self, name: &str) {
        if self.registered_providers.remove(name).is_some() {
            compat::unregister_api_providers(&format!("provider:{name}"));
            unregister_oauth_provider(name);
            self.refresh()
        }
    }
    fn apply_provider_config(&mut self, name: &str, config: &ProviderConfigInput) {
        if let Some(oauth) = &config.oauth {
            register_oauth_provider(Arc::new(NamedOAuthProvider {
                id: name.into(),
                inner: Arc::clone(oauth),
            }));
        }
        if let (Some(api), Some(stream_simple)) = (&config.api, &config.stream_simple) {
            let simple_for_stream = Arc::clone(stream_simple);
            compat::register_api_provider(
                compat::ApiProvider {
                    api: api.clone(),
                    stream: Arc::new(move |model, context, options| {
                        simple_for_stream(
                            model,
                            context,
                            options.map(|stream| SimpleStreamOptions {
                                stream,
                                reasoning: None,
                                thinking_budgets: None,
                            }),
                        )
                    }),
                    stream_simple: Arc::clone(stream_simple),
                },
                Some(format!("provider:{name}")),
            );
        }
        self.store_request_config(
            name,
            config.api_key.clone(),
            config.headers.clone(),
            config.auth_header,
        );
        if let Some(models) = config.models.as_ref().filter(|v| !v.is_empty()) {
            self.models.retain(|m| m.provider != name);
            for mut model in models.clone() {
                model.provider = name.into();
                if let Some(url) = &config.base_url {
                    if model.base_url.is_empty() {
                        model.base_url.clone_from(url)
                    }
                }
                if model.api.is_empty() {
                    if let Some(api) = &config.api {
                        model.api.clone_from(api)
                    }
                }
                let headers = model.headers.take();
                self.store_model_headers(name, &model.id, headers);
                self.models.push(model);
            }
            if let (Some(oauth), Some(credentials)) = (
                &config.oauth,
                oauth_credentials(self.auth_storage.get(name)),
            ) {
                self.models = oauth.modify_models(&self.models, &credentials);
            }
        } else if let Some(url) = &config.base_url {
            for model in self.models.iter_mut().filter(|m| m.provider == name) {
                model.base_url.clone_from(url);
            }
        }
    }
}

struct NamedOAuthProvider {
    id: String,
    inner: Arc<dyn OAuthProviderInterface>,
}

impl OAuthProviderInterface for NamedOAuthProvider {
    fn id(&self) -> &str {
        &self.id
    }
    fn name(&self) -> &str {
        self.inner.name()
    }
    fn uses_callback_server(&self) -> bool {
        self.inner.uses_callback_server()
    }
    fn login<'a>(
        &'a self,
        callbacks: &'a dyn AuthLoginCallbacks,
    ) -> AuthFuture<'a, AuthResult<OAuthCredential>> {
        self.inner.login(callbacks)
    }
    fn refresh_token<'a>(
        &'a self,
        credentials: &'a OAuthCredential,
    ) -> AuthFuture<'a, AuthResult<OAuthCredential>> {
        self.inner.refresh_token(credentials)
    }
    fn get_api_key(&self, credentials: &OAuthCredential) -> String {
        self.inner.get_api_key(credentials)
    }
    fn modify_models(&self, models: &[Model], credentials: &OAuthCredential) -> Vec<Model> {
        self.inner.modify_models(models, credentials)
    }
}

fn oauth_credentials(credential: Option<&AuthCredential>) -> Option<OAuthCredential> {
    let AuthCredential::OAuth {
        refresh,
        access,
        expires,
        extra,
    } = credential?
    else {
        return None;
    };
    Some(OAuthCredential {
        refresh: refresh.clone(),
        access: access.clone(),
        expires: *expires,
        extra: extra.clone(),
    })
}

fn merge_compat(base: Option<ModelCompat>, over: Option<ModelCompat>) -> Option<ModelCompat> {
    match (base, over) {
        (value, None) => value,
        (None, value) => value,
        (Some(base), Some(over)) => {
            let mut a = serde_json::to_value(base).ok()?;
            let b = serde_json::to_value(over).ok()?;
            merge_json(&mut a, b);
            serde_json::from_value(a).ok()
        }
    }
}
fn merge_json(base: &mut serde_json::Value, over: serde_json::Value) {
    match (base.as_object_mut(), over) {
        (Some(a), serde_json::Value::Object(b)) => {
            for (k, v) in b {
                if let Some(old) = a.get_mut(&k) {
                    merge_json(old, v)
                } else {
                    a.insert(k, v);
                }
            }
        }
        (_, value) => *base = value,
    }
}
fn apply_override(model: &mut Model, value: &ModelOverride) {
    if let Some(v) = &value.name {
        model.name.clone_from(v)
    }
    if let Some(v) = value.reasoning {
        model.reasoning = v
    }
    if let Some(v) = &value.thinking_level_map {
        model.thinking_level_map = Some(v.clone())
    }
    if let Some(v) = &value.input {
        model.input.clone_from(v)
    }
    if let Some(v) = value.context_window {
        model.context_window = v
    }
    if let Some(v) = value.max_tokens {
        model.max_tokens = v
    }
    if let Some(v) = &value.cost {
        if let Some(x) = v.input {
            model.cost.input = x
        }
        if let Some(x) = v.output {
            model.cost.output = x
        }
        if let Some(x) = v.cache_read {
            model.cost.cache_read = x
        }
        if let Some(x) = v.cache_write {
            model.cost.cache_write = x
        }
    }
    model.compat = merge_compat(model.compat.clone(), value.compat.clone());
}
fn merge_provider_input(old: &mut ProviderConfigInput, new: &ProviderConfigInput) {
    if new.name.is_some() {
        old.name = new.name.clone()
    }
    if new.base_url.is_some() {
        old.base_url = new.base_url.clone()
    }
    if new.api_key.is_some() {
        old.api_key = new.api_key.clone()
    }
    if new.api.is_some() {
        old.api = new.api.clone()
    }
    if new.headers.is_some() {
        old.headers = new.headers.clone()
    }
    if new.models.is_some() {
        old.models = new.models.clone()
    }
    if new.auth_header {
        old.auth_header = true
    }
    if new.stream_simple.is_some() {
        old.stream_simple.clone_from(&new.stream_simple)
    }
    if new.oauth.is_some() {
        old.oauth.clone_from(&new.oauth)
    }
}
