#![forbid(unsafe_code)]

//! Zedflow coding-agent crate.

/// Deterministic utilities ported from Pi's coding-agent package.
#[path = "utils/mod.rs"]
pub mod utils;

pub mod config;

#[path = "bun/cli.rs"]
pub mod bun_cli;
#[path = "bun/register-bedrock.rs"]
pub mod bun_register_bedrock;
#[path = "bun/restore-sandbox-env.rs"]
pub mod bun_restore_sandbox_env;
#[path = "migrations.rs"]
pub mod migrations;

#[path = "core/bash-executor.rs"]
pub mod bash_executor;
#[path = "core/tools/bash.rs"]
pub mod bash_tool;
#[path = "core/exec.rs"]
pub mod exec;
#[path = "core/footer-data-provider.rs"]
pub mod footer_data_provider;
#[path = "core/keybindings.rs"]
pub mod keybindings;
#[path = "core/package-manager.rs"]
pub mod package_manager;

#[path = "modes/interactive/components/custom-entry.rs"]
pub mod custom_entry;
#[path = "modes/interactive/components/custom-message.rs"]
pub mod custom_message;
#[path = "modes/interactive/components/daxnuts.rs"]
pub mod daxnuts;
#[path = "modes/interactive/components/diff.rs"]
pub mod diff;
#[path = "modes/interactive/components/dynamic-border.rs"]
pub mod dynamic_border;
#[path = "modes/interactive/components/earendil-announcement.rs"]
pub mod earendil_announcement;
#[path = "modes/interactive/components/extension-editor.rs"]
pub mod extension_editor;
#[path = "modes/interactive/components/extension-input.rs"]
pub mod extension_input;
#[path = "modes/interactive/components/extension-selector.rs"]
pub mod extension_selector;
#[path = "modes/interactive/components/first-time-setup.rs"]
pub mod first_time_setup;
#[path = "modes/interactive/components/footer.rs"]
pub mod footer;
#[path = "modes/interactive/components/keybinding-hints.rs"]
pub mod keybinding_hints;
#[path = "modes/interactive/components/login-dialog.rs"]
pub mod login_dialog;
#[path = "modes/interactive/model-search.rs"]
pub mod model_search;
#[path = "modes/interactive/components/model-selector.rs"]
pub mod model_selector;
#[path = "modes/interactive/components/index.rs"]
pub mod modes_interactive_components_index;
#[path = "modes/interactive/theme/theme.rs"]
pub mod modes_interactive_theme_theme;
#[path = "modes/interactive/components/oauth-selector.rs"]
pub mod oauth_selector;
#[path = "package-manager-cli.rs"]
pub mod package_manager_cli;
#[path = "core/project-trust.rs"]
pub mod project_trust;
#[path = "core/prompt-templates.rs"]
pub mod prompt_templates;
#[path = "core/provider-attribution.rs"]
pub mod provider_attribution;
#[path = "core/tools/render-utils.rs"]
pub mod render_utils;
#[path = "modes/interactive/components/scoped-models-selector.rs"]
pub mod scoped_models_selector;
#[path = "core/sdk.rs"]
pub mod sdk;
#[path = "modes/interactive/components/session-selector.rs"]
pub mod session_selector;
#[path = "modes/interactive/components/session-selector-search.rs"]
pub mod session_selector_search;
#[path = "modes/interactive/components/settings-selector.rs"]
pub mod settings_selector;
#[path = "modes/interactive/components/show-images-selector.rs"]
pub mod show_images_selector;
#[path = "modes/interactive/components/skill-invocation-message.rs"]
pub mod skill_invocation_message;
#[path = "modes/interactive/components/status-indicator.rs"]
pub mod status_indicator;
#[path = "core/telemetry.rs"]
pub mod telemetry;
#[path = "modes/interactive/theme/theme-controller.rs"]
pub mod theme_controller;
#[path = "modes/interactive/components/theme-selector.rs"]
pub mod theme_selector;
#[path = "modes/interactive/components/thinking-selector.rs"]
pub mod thinking_selector;
#[path = "core/tools/tool-definition-wrapper.rs"]
pub mod tool_definition_wrapper;
#[path = "modes/interactive/components/tool-execution.rs"]
pub mod tool_execution;
#[path = "core/tools/index.rs"]
pub mod tools_index;
#[path = "modes/interactive/components/tree-selector.rs"]
pub mod tree_selector;
#[path = "core/trust-manager.rs"]
pub mod trust_manager;
#[path = "modes/interactive/components/trust-selector.rs"]
pub mod trust_selector;
#[path = "modes/interactive/components/user-message.rs"]
pub mod user_message;
#[path = "modes/interactive/components/user-message-selector.rs"]
pub mod user_message_selector;
#[path = "modes/interactive/components/visual-truncate.rs"]
pub mod visual_truncate;

#[path = "cli/mod.rs"]
pub mod cli;
#[path = "index.rs"]
pub mod index;
#[path = "modes/mod.rs"]
pub mod modes;
#[path = "rpc-entry.rs"]
pub mod rpc_entry;

pub use cli::{Args, InitialMessageResult, Mode, build_initial_message, parse_args};
pub use modes::{
    AssistantResult, PrintModeOptions, PrintOutputMode, RpcClient, RpcCommand, RpcResponse,
    handle_command_line, prompts, render_print_result, run_rpc_loop,
};

#[path = "core/agent-session.rs"]
pub mod agent_session;
#[path = "core/agent-session-runtime.rs"]
pub mod agent_session_runtime;
#[path = "core/agent-session-services.rs"]
pub mod agent_session_services;
#[path = "core/auth-guidance.rs"]
pub mod auth_guidance;
#[path = "core/auth-storage.rs"]
pub mod auth_storage;
#[path = "core/compaction/mod.rs"]
pub mod compaction;
#[path = "core/defaults.rs"]
pub mod defaults;
#[path = "core/diagnostics.rs"]
pub mod diagnostics;
#[path = "core/event-bus.rs"]
pub mod event_bus;
#[path = "core/experimental.rs"]
pub mod experimental;
#[path = "core/export-html/mod.rs"]
pub mod export_html;
#[path = "core/extensions/mod.rs"]
pub mod extensions;
#[path = "core/http-dispatcher.rs"]
pub mod http_dispatcher;
#[path = "core/messages.rs"]
pub mod messages;
#[path = "core/model-registry.rs"]
pub mod model_registry;
#[path = "core/model-resolver.rs"]
pub mod model_resolver;
#[path = "core/output-guard.rs"]
pub mod output_guard;
#[path = "core/provider-display-names.rs"]
pub mod provider_display_names;
#[path = "core/resolve-config-value.rs"]
pub mod resolve_config_value;
#[path = "core/resource-loader.rs"]
pub mod resource_loader;
#[path = "core/session-cwd.rs"]
pub mod session_cwd;
#[path = "core/session-manager.rs"]
pub mod session_manager;
#[path = "core/settings-manager.rs"]
pub mod settings_manager;
#[path = "core/skills.rs"]
pub mod skills;
#[path = "core/slash-commands.rs"]
pub mod slash_commands;
#[path = "core/source-info.rs"]
pub mod source_info;
#[path = "core/system-prompt.rs"]
pub mod system_prompt;
#[path = "core/timings.rs"]
pub mod timings;

#[path = "core/tools/edit.rs"]
pub mod edit;
#[path = "core/tools/edit-diff.rs"]
pub mod edit_diff;
#[path = "core/tools/file-mutation-queue.rs"]
pub mod file_mutation_queue;
#[path = "core/tools/find.rs"]
pub mod find;
#[path = "core/tools/grep.rs"]
pub mod grep;
#[path = "core/tools/ls.rs"]
pub mod ls;
#[path = "core/tools/output-accumulator.rs"]
pub mod output_accumulator;
#[path = "core/tools/path-utils.rs"]
pub mod path_utils;
#[path = "core/tools/read.rs"]
pub mod read;
#[path = "core/tools/truncate.rs"]
pub mod truncate;
#[path = "core/tools/write.rs"]
pub mod write;

/// Pi-compatible tool namespaces.
pub mod core {
    pub use crate::{
        agent_session, agent_session_runtime, agent_session_services, auth_guidance, auth_storage,
        bash_executor, compaction, defaults, diagnostics, event_bus, exec, experimental,
        export_html, extensions, http_dispatcher, messages, model_registry, model_resolver,
        output_guard, provider_display_names, resource_loader, session_cwd, session_manager,
        settings_manager, skills, slash_commands, source_info, system_prompt, timings,
    };

    pub mod tools {
        pub use crate::{
            bash_tool as bash, edit, edit_diff, file_mutation_queue, find, grep, ls,
            output_accumulator, path_utils, read, render_utils, tool_definition_wrapper,
            tools_index, truncate, write,
        };
    }
}

/// Crate identity, useful while the clean workspace skeleton is being filled.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");
