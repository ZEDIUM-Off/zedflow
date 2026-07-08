//! Cloudflare auth helpers ported from Pi's `packages/ai/src/providers/cloudflare-auth.ts`.

use crate::auth::types::{
    ApiKeyAuth as ApiKeyAuthTrait, ApiKeyCredential, ApiKeyResolveInput, AuthLoginCallbacks,
    AuthModel, AuthPrompt, AuthResult as AuthCallbackResult, ModelAuth, ProviderEnv,
    ProviderHeaders, ResolvedAuth,
};

/// Cloudflare API key environment variable used by Pi.
pub const CLOUDFLARE_API_KEY: &str = "CLOUDFLARE_API_KEY";

/// Cloudflare account id environment variable used by Pi.
pub const CLOUDFLARE_ACCOUNT_ID: &str = "CLOUDFLARE_ACCOUNT_ID";

/// Cloudflare AI Gateway id environment variable used by Pi.
pub const CLOUDFLARE_GATEWAY_ID: &str = "CLOUDFLARE_GATEWAY_ID";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CloudflareAuthKind {
    WorkersAi,
    AiGateway,
}

/// API-key auth implementation for Cloudflare Workers AI.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CloudflareWorkersAiAuth;

/// API-key auth implementation for Cloudflare AI Gateway.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CloudflareAiGatewayAuth;

/// Creates Pi's Cloudflare Workers AI auth handler.
#[must_use]
pub const fn cloudflare_workers_ai_auth() -> CloudflareWorkersAiAuth {
    CloudflareWorkersAiAuth
}

/// Creates Pi's Cloudflare AI Gateway auth handler.
#[must_use]
pub const fn cloudflare_ai_gateway_auth() -> CloudflareAiGatewayAuth {
    CloudflareAiGatewayAuth
}

impl ApiKeyAuthTrait for CloudflareWorkersAiAuth {
    fn name(&self) -> &str {
        "Cloudflare API key"
    }

    fn login<'a>(
        &'a self,
        callbacks: &'a dyn AuthLoginCallbacks,
    ) -> crate::auth::types::AuthFuture<'a, AuthCallbackResult<Option<ApiKeyCredential>>> {
        Box::pin(async move {
            let key = callbacks
                .prompt(AuthPrompt::Secret {
                    message: "Enter Cloudflare API key".to_owned(),
                    placeholder: None,
                    signal: None,
                })
                .await?;
            let account_id = callbacks
                .prompt(AuthPrompt::Text {
                    message: "Enter Cloudflare account ID".to_owned(),
                    placeholder: None,
                    signal: None,
                })
                .await?;

            Ok(Some(ApiKeyCredential {
                key: Some(key),
                env: Some(ProviderEnv::from([(
                    CLOUDFLARE_ACCOUNT_ID.to_owned(),
                    account_id,
                )])),
            }))
        })
    }

    fn resolve<'a>(
        &'a self,
        input: ApiKeyResolveInput<'a>,
    ) -> crate::auth::types::AuthFuture<'a, AuthCallbackResult<Option<ResolvedAuth>>> {
        Box::pin(async move {
            Ok(resolve_cloudflare_env(
                CloudflareAuthKind::WorkersAi,
                input.model,
                input.ctx,
                input.credential,
            )
            .await
            .map(|resolved| ResolvedAuth {
                auth: ModelAuth {
                    api_key: Some(resolved.api_key),
                    headers: None,
                    base_url: Some(resolved.base_url),
                },
                env: Some(resolved.env),
                source: Some(resolved.source),
            }))
        })
    }
}

impl ApiKeyAuthTrait for CloudflareAiGatewayAuth {
    fn name(&self) -> &str {
        "Cloudflare API key"
    }

    fn login<'a>(
        &'a self,
        callbacks: &'a dyn AuthLoginCallbacks,
    ) -> crate::auth::types::AuthFuture<'a, AuthCallbackResult<Option<ApiKeyCredential>>> {
        Box::pin(async move {
            let key = callbacks
                .prompt(AuthPrompt::Secret {
                    message: "Enter Cloudflare API key".to_owned(),
                    placeholder: None,
                    signal: None,
                })
                .await?;
            let account_id = callbacks
                .prompt(AuthPrompt::Text {
                    message: "Enter Cloudflare account ID".to_owned(),
                    placeholder: None,
                    signal: None,
                })
                .await?;
            let gateway_id = callbacks
                .prompt(AuthPrompt::Text {
                    message: "Enter Cloudflare AI Gateway ID".to_owned(),
                    placeholder: None,
                    signal: None,
                })
                .await?;

            Ok(Some(ApiKeyCredential {
                key: Some(key),
                env: Some(ProviderEnv::from([
                    (CLOUDFLARE_ACCOUNT_ID.to_owned(), account_id),
                    (CLOUDFLARE_GATEWAY_ID.to_owned(), gateway_id),
                ])),
            }))
        })
    }

    fn resolve<'a>(
        &'a self,
        input: ApiKeyResolveInput<'a>,
    ) -> crate::auth::types::AuthFuture<'a, AuthCallbackResult<Option<ResolvedAuth>>> {
        Box::pin(async move {
            Ok(resolve_cloudflare_env(
                CloudflareAuthKind::AiGateway,
                input.model,
                input.ctx,
                input.credential,
            )
            .await
            .map(|resolved| ResolvedAuth {
                auth: ModelAuth {
                    api_key: None,
                    headers: Some(ProviderHeaders::from([
                        (
                            "cf-aig-authorization".to_owned(),
                            Some(format!("Bearer {}", resolved.api_key)),
                        ),
                        ("Authorization".to_owned(), None),
                        ("x-api-key".to_owned(), None),
                    ])),
                    base_url: Some(resolved.base_url),
                },
                env: Some(resolved.env),
                source: Some(resolved.source),
            }))
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CloudflareResolvedEnv {
    api_key: String,
    env: ProviderEnv,
    base_url: String,
    source: String,
}

async fn resolve_value(
    name: &str,
    ctx: &dyn crate::auth::types::AuthContext,
    credential: Option<&ApiKeyCredential>,
) -> Option<String> {
    if let Some(credential) = credential {
        if name == CLOUDFLARE_API_KEY {
            return credential.key.clone();
        }
        return credential
            .env
            .as_ref()
            .and_then(|env| env.get(name).cloned());
    }
    ctx.env(name).await
}

fn resolve_cloudflare_base_url(
    model: &AuthModel,
    account_id: &str,
    gateway_id: Option<&str>,
) -> String {
    model
        .base_url
        .as_deref()
        .unwrap_or_default()
        .replace(&format!("{{{CLOUDFLARE_ACCOUNT_ID}}}"), account_id)
        .replace(
            &format!("{{{CLOUDFLARE_GATEWAY_ID}}}"),
            gateway_id.unwrap_or_default(),
        )
}

async fn resolve_cloudflare_env(
    kind: CloudflareAuthKind,
    model: &AuthModel,
    ctx: &dyn crate::auth::types::AuthContext,
    credential: Option<&ApiKeyCredential>,
) -> Option<CloudflareResolvedEnv> {
    let api_key = resolve_value(CLOUDFLARE_API_KEY, ctx, credential).await?;
    let account_id = resolve_value(CLOUDFLARE_ACCOUNT_ID, ctx, credential).await?;
    let gateway_id = match kind {
        CloudflareAuthKind::WorkersAi => None,
        CloudflareAuthKind::AiGateway => {
            Some(resolve_value(CLOUDFLARE_GATEWAY_ID, ctx, credential).await?)
        }
    };

    if api_key.is_empty()
        || account_id.is_empty()
        || gateway_id.as_deref().is_some_and(str::is_empty)
    {
        return None;
    }

    let mut env = ProviderEnv::from([(CLOUDFLARE_ACCOUNT_ID.to_owned(), account_id.clone())]);
    if let Some(gateway_id) = &gateway_id {
        env.insert(CLOUDFLARE_GATEWAY_ID.to_owned(), gateway_id.clone());
    }

    Some(CloudflareResolvedEnv {
        api_key,
        env,
        base_url: resolve_cloudflare_base_url(model, &account_id, gateway_id.as_deref()),
        source: if credential.is_some() {
            "stored credential".to_owned()
        } else {
            CLOUDFLARE_API_KEY.to_owned()
        },
    })
}
