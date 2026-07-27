//! Interactive-mode contracts that do not require the TUI package.

use std::{
    collections::{HashSet, VecDeque},
    io::{self, BufRead, Write},
};

use zedflow_ai::AssistantContentBlock;

/// Parse the first argument of an interactive path command (`/import` or
/// `/export`). The command must be a complete token; quoted arguments have
/// their matching outer quotes removed.
#[must_use]
pub fn get_path_command_argument(text: &str, command: &str) -> Option<String> {
    let prefix = format!("{command} ");
    if text == command || !text.starts_with(&prefix) {
        return None;
    }

    let argument = text[prefix.len()..].trim_start();
    if argument.is_empty() {
        return None;
    }

    match argument.as_bytes()[0] {
        b'\'' | b'"' => {
            let quote = argument.as_bytes()[0] as char;
            argument[1..]
                .find(quote)
                .map(|end| argument[1..end + 1].to_owned())
        }
        _ => argument.split_whitespace().next().map(str::to_owned),
    }
}

/// Quote a path using the same conservative shell-safe form as Pi.
#[must_use]
pub fn quote_if_needed(value: &str) -> String {
    if !value.is_empty()
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "_-./~:@".contains(character))
    {
        value.to_owned()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

/// Whether selecting a provider should use an API-key login rather than an
/// OAuth flow. Built-in display-name entries are API-key providers in Pi;
/// other built-ins and registered OAuth providers are not.
#[must_use]
pub fn is_api_key_login_provider(
    provider_id: &str,
    oauth_provider_ids: &HashSet<String>,
    built_in_provider_ids: &HashSet<String>,
    built_in_display_name_ids: &HashSet<String>,
) -> bool {
    if built_in_display_name_ids.contains(provider_id) {
        return true;
    }
    if built_in_provider_ids.contains(provider_id) {
        return false;
    }
    !oauth_provider_ids.contains(provider_id)
}

/// Pi's Anthropic subscription credentials use this key prefix.
#[must_use]
pub fn is_anthropic_subscription_auth_key(api_key: Option<&str>) -> bool {
    api_key.is_some_and(|key| key.starts_with("sk-ant-oat"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractiveState {
    Created,
    Running,
    Stopped,
}

#[derive(Debug, Default)]
pub struct InteractiveMode {
    state: InteractiveState,
    pending_user_inputs: VecDeque<String>,
    last_status: Option<String>,
}

impl Default for InteractiveState {
    fn default() -> Self {
        Self::Created
    }
}
impl InteractiveMode {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    pub fn run(&mut self) {
        self.state = InteractiveState::Running;
    }
    pub fn stop(&mut self) {
        self.state = InteractiveState::Stopped;
    }
    #[must_use]
    pub fn state(&self) -> InteractiveState {
        self.state
    }

    /// Queue startup input until the interactive consumer is ready.
    pub fn queue_user_input(&mut self, text: impl Into<String>) {
        let text = text.into().trim().to_owned();
        if !text.is_empty() {
            self.pending_user_inputs.push_back(text);
        }
    }

    /// Return queued input in submission order, matching `getUserInput()`.
    pub fn get_user_input(&mut self) -> Option<String> {
        self.pending_user_inputs.pop_front()
    }

    #[must_use]
    pub fn pending_user_input_count(&self) -> usize {
        self.pending_user_inputs.len()
    }

    /// Coalesce adjacent status updates and return the text currently shown.
    pub fn show_status(&mut self, message: impl Into<String>) -> &str {
        self.last_status = Some(message.into());
        self.last_status.as_deref().unwrap_or_default()
    }

    #[must_use]
    pub fn last_status(&self) -> Option<&str> {
        self.last_status.as_deref()
    }
}

/// Run the currently usable terminal chat mode on the real Rust agent runtime.
///
/// This deliberately uses canonical terminal input: it is immediately testable
/// without introducing another terminal dependency while the full-screen Pi UI
/// remains a separate fidelity gap.
pub fn run(args: &[String]) -> io::Result<()> {
    let parsed = crate::cli::parse_args(args.iter().cloned());
    let setup = crate::rpc_entry::create_runtime(args)?;
    let session = setup.runtime.session();
    let model = session.get_model();
    let executor = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| io::Error::other(error.to_string()))?;
    let initial = (!parsed.messages.is_empty()).then(|| parsed.messages.join(" "));
    let stdin = io::stdin();
    let stdout = io::stdout();
    run_loop(
        stdin.lock(),
        stdout.lock(),
        initial,
        &format!("{}/{}", model.provider, model.id),
        |prompt| {
            executor
                .block_on(session.prompt(prompt.to_owned(), None))
                .map(|message| {
                    let text = message
                        .content
                        .into_iter()
                        .filter_map(|block| match block {
                            AssistantContentBlock::Text(text) => Some(text.text),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    if text.is_empty() {
                        message
                            .error_message
                            .unwrap_or_else(|| "(no text response)".into())
                    } else {
                        text
                    }
                })
                .map_err(|error| error.to_string())
        },
    )
}

fn run_loop<R, W, F>(
    mut reader: R,
    mut writer: W,
    initial: Option<String>,
    model: &str,
    mut prompt: F,
) -> io::Result<()>
where
    R: BufRead,
    W: Write,
    F: FnMut(&str) -> Result<String, String>,
{
    writeln!(writer, "\x1b[1;36mPi Rust\x1b[0m  \x1b[2m{model}\x1b[0m")?;
    writeln!(writer, "\x1b[2m/help for commands · /quit to exit\x1b[0m\n")?;

    let mut pending = initial;
    loop {
        let input = if let Some(input) = pending.take() {
            writeln!(writer, "\x1b[1;32m❯\x1b[0m {input}")?;
            input
        } else {
            write!(writer, "\x1b[1;32m❯\x1b[0m ")?;
            writer.flush()?;
            let mut input = String::new();
            if reader.read_line(&mut input)? == 0 {
                writeln!(writer)?;
                break;
            }
            input.trim().to_owned()
        };

        match input.as_str() {
            "" => continue,
            "/quit" | "/exit" => break,
            "/clear" => {
                write!(writer, "\x1b[2J\x1b[H")?;
                continue;
            }
            "/help" => {
                writeln!(writer, "\n/clear  clear screen\n/quit   exit\n")?;
                continue;
            }
            _ => {}
        }

        write!(writer, "\x1b[2mThinking…\x1b[0m\r")?;
        writer.flush()?;
        match prompt(&input) {
            Ok(response) => writeln!(writer, "\x1b[2K\r\x1b[1;35mAssistant\x1b[0m\n{response}\n")?,
            Err(error) => writeln!(writer, "\x1b[2K\r\x1b[1;31mError:\x1b[0m {error}\n")?,
        }
    }
    Ok(())
}

#[cfg(test)]
mod terminal_tests {
    use super::*;

    #[test]
    fn terminal_loop_prompts_and_exits() {
        let input = io::Cursor::new(b"hello\n/quit\n");
        let mut output = Vec::new();
        run_loop(input, &mut output, None, "test/model", |prompt| {
            Ok(format!("reply:{prompt}"))
        })
        .unwrap();
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("Pi Rust"));
        assert!(output.contains("reply:hello"));
    }
}
