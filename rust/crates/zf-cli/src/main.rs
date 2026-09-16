mod commands;
mod templates;

use clap::Parser;

#[tokio::main]
async fn main() -> std::process::ExitCode {
    match commands::execute(commands::Cli::parse()).await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Erreur : {error:#}");
            std::process::ExitCode::FAILURE
        }
    }
}
