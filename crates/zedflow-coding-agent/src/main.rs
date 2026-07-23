//! Binary entry point for the coding-agent CLI.

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let parsed = zedflow_coding_agent::cli::parse_args(&args);
    if parsed.help {
        print!("{}", zedflow_coding_agent::cli::help_text());
    } else if parsed.version {
        println!("{}", zedflow_coding_agent::config::VERSION);
    }
}
