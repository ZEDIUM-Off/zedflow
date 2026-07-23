#![forbid(unsafe_code)]

//! Zedflow coding-agent crate.

/// Deterministic utilities ported from Pi's coding-agent package.
#[path = "utils/mod.rs"]
pub mod utils;

pub mod config;

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
        compaction, defaults, diagnostics, event_bus, experimental, export_html, extensions,
        http_dispatcher, messages, model_registry, model_resolver, output_guard,
        provider_display_names, resource_loader, session_cwd, session_manager, settings_manager,
        skills, slash_commands, source_info, system_prompt, timings,
    };

    pub mod tools {
        pub use crate::{
            edit, edit_diff, file_mutation_queue, find, grep, ls, output_accumulator, path_utils,
            read, truncate, write,
        };
    }
}

/// Crate identity, useful while the clean workspace skeleton is being filled.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");
