//! `pi` command entry point.

use std::{fs, io, path::Path, time::Duration};
use zedflow_coding_agent::cli::{Mode, parse_args};
use zedflow_coding_agent::{
    config::get_agent_dir,
    extensions::{ABI_V1, JsonEnvelope, load_native_extensions},
    modes::InteractiveMode,
    package_manager::{DefaultPackageManager, PackageScope},
    package_manager_cli::{PackageCommand, parse_package_command},
    resource_loader::DefaultResourceLoader,
    rpc_entry,
};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Err(error) = dispatch(&args) {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}

fn dispatch(args: &[String]) -> Result<(), String> {
    if let Some(command) = parse_package_command(args) {
        return dispatch_package(command, args);
    }
    if args.first().is_some_and(|arg| arg == "config") {
        return Err("config requires the interactive configuration UI".into());
    }

    let parsed = parse_args(args.iter().cloned());
    for diagnostic in &parsed.diagnostics {
        eprintln!(
            "{}: {}",
            match diagnostic.kind {
                zedflow_coding_agent::cli::DiagnosticType::Error => "Error",
                zedflow_coding_agent::cli::DiagnosticType::Warning => "Warning",
            },
            diagnostic.message
        );
    }
    if parsed
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.kind == zedflow_coding_agent::cli::DiagnosticType::Error)
    {
        return Err("invalid arguments".into());
    }
    if parsed.version {
        println!("{}", zedflow_coding_agent::config::VERSION);
        return Ok(());
    }
    if parsed.help {
        println!(
            "pi - AI coding assistant\n\nUse --print for non-interactive mode or --mode rpc for JSONL RPC."
        );
        return Ok(());
    }
    if let Some(input) = parsed.export.as_deref() {
        let output = parsed.messages.first().map(String::as_str);
        let path = export_file(input, output)?;
        println!("Exported to: {}", path.display());
        return Ok(());
    }
    if let Some(pattern) = parsed.list_models.as_ref() {
        list_models(pattern.as_deref());
        return Ok(());
    }

    match parsed.mode.or(parsed.print.then_some(Mode::Text)) {
        Some(Mode::Rpc) => {
            rpc_entry::run(io::stdin().lock(), io::stdout()).map_err(|e| e.to_string())
        }
        Some(Mode::Text) | Some(Mode::Json) => run_print(args, &parsed),
        None => run_interactive().map_err(|e| e.to_string()),
    }
}

fn dispatch_package(
    command: zedflow_coding_agent::package_manager_cli::PackageCommandOptions,
    _args: &[String],
) -> Result<(), String> {
    if command.help {
        println!("Package commands: install, remove, update, list");
        return Ok(());
    }
    if let Some(error) = command
        .invalid_option
        .or(command.invalid_argument)
        .or(command.missing_option_value)
        .or(command.conflicting_options)
    {
        return Err(error);
    }
    let manager = DefaultPackageManager::new(
        std::env::current_dir().map_err(|e| e.to_string())?,
        get_agent_dir(),
    );
    let scope = if command.local {
        PackageScope::Project
    } else {
        PackageScope::User
    };
    match command.command {
        PackageCommand::List => {
            for package in manager.list_configured_packages() {
                println!("{}", package.source);
            }
            Ok(())
        }
        PackageCommand::Remove => {
            let source = command.source.ok_or("remove requires a source")?;
            if !manager.remove(&source, scope)? {
                return Err(format!("Package not found: {source}"));
            }
            println!("Removed: {source}");
            Ok(())
        }
        PackageCommand::Install => {
            let source = command.source.ok_or("install requires a source")?;
            manager.install_source(&source, scope)?;
            println!("Installed: {source}");
            Ok(())
        }
        PackageCommand::Update => {
            let target = command.update_target.ok_or("update target missing")?;
            if !zedflow_coding_agent::package_manager_cli::update_target_includes_extensions(
                &target,
            ) {
                return Err("self update is not available in the source-only build".into());
            }
            let source = match target {
                zedflow_coding_agent::package_manager_cli::UpdateTarget::Extensions(source) => {
                    source
                }
                zedflow_coding_agent::package_manager_cli::UpdateTarget::All => command.source,
                zedflow_coding_agent::package_manager_cli::UpdateTarget::SelfOnly => unreachable!(),
            };
            let count = manager.update(source.as_deref(), scope)?;
            println!("Updated {count} package(s)");
            Ok(())
        }
    }
}

fn list_models(pattern: Option<&str>) {
    let models = zedflow_ai::providers::all::builtin_models().get_models(None);
    let mut models = zedflow_coding_agent::cli::list_models::filter_models(&models, pattern);
    models.sort_by(|left, right| {
        (left.provider.as_str(), &left.id).cmp(&(right.provider.as_str(), &right.id))
    });
    if models.is_empty() {
        println!(
            "No models{}",
            pattern.map_or_else(String::new, |value| format!(" matching \\\"{value}\\\""))
        );
        return;
    }
    for model in models {
        println!("{}  {}", model.provider, model.id);
    }
}

fn export_file(input: &str, output: Option<&str>) -> Result<std::path::PathBuf, String> {
    let input = Path::new(input);
    let content =
        fs::read_to_string(input).map_err(|error| format!("{}: {error}", input.display()))?;
    let output = output.map_or_else(
        || {
            std::path::PathBuf::from(format!(
                "pi-session-{}.html",
                input.file_stem().unwrap_or_default().to_string_lossy()
            ))
        },
        std::path::PathBuf::from,
    );
    fs::write(
        &output,
        zedflow_coding_agent::export_html::export_session_to_html(&content),
    )
    .map_err(|error| format!("{}: {error}", output.display()))?;
    Ok(output)
}

fn run_print(args: &[String], parsed: &zedflow_coding_agent::cli::Args) -> Result<(), String> {
    let runtime = rpc_entry::create_runtime_for_args(args).map_err(|error| error.to_string())?;
    let session = runtime.session();
    let result = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| error.to_string())?
        .block_on(async {
            let mut result = None;
            for prompt in &parsed.messages {
                result = Some(session.prompt(prompt, None).await);
            }
            result
                .ok_or_else(|| "print mode requires a prompt".to_owned())?
                .map_err(|error| error.to_string())
        })?;
    if parsed.mode == Some(Mode::Json) {
        println!(
            "{}",
            serde_json::to_string(&result).map_err(|error| error.to_string())?
        );
        return Ok(());
    }
    for content in result.content {
        if let zedflow_ai::AssistantContentBlock::Text(text) = content {
            println!("{}", text.text);
        }
    }
    Ok(())
}

fn run_interactive() -> io::Result<()> {
    let cwd = std::env::current_dir()?;
    let mut resources = DefaultResourceLoader::new(&cwd, get_agent_dir());
    resources.reload();
    let runner = load_native_extensions(
        resources.native_extension_artifacts(),
        &JsonEnvelope {
            version: ABI_V1,
            payload: serde_json::json!({"kind": "initialize"}),
        },
    )
    .map_err(io::Error::other)?;
    let mut mode =
        InteractiveMode::with_extension_runner(zedflow_tui::ProcessTerminal::new(), runner);
    mode.run()?;
    loop {
        mode.pump_events(Duration::from_millis(10))?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_route_writes_default_html_file() {
        let directory = std::env::temp_dir().join(format!("zedflow-export-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let input = directory.join("session.jsonl");
        fs::write(&input, "hello").unwrap();
        let output = directory.join("result.html");
        assert_eq!(
            export_file(input.to_str().unwrap(), Some(output.to_str().unwrap())).unwrap(),
            output
        );
        assert!(fs::read_to_string(output).unwrap().contains("hello"));
    }
}
