//! Deterministic state for Pi's OAuth login dialog.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoginView {
    Empty,
    Auth {
        url: String,
        instructions: Option<String>,
    },
    DeviceCode {
        verification_uri: String,
        user_code: String,
    },
    Prompt {
        message: String,
        placeholder: Option<String>,
    },
    Info(Vec<String>),
    Waiting(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoginAction {
    OpenBrowser(String),
    Submit(String),
    Cancel { success: bool, message: String },
    None,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginDialog {
    pub title: String,
    pub view: LoginView,
    pub progress: Vec<String>,
    pending_input: bool,
    cancelled: bool,
}

impl LoginDialog {
    #[must_use]
    pub fn new(provider_id: &str, provider_name: Option<&str>, title: Option<&str>) -> Self {
        let provider = provider_name.unwrap_or(provider_id);
        Self {
            title: title.map_or_else(|| format!("Login to {provider}"), str::to_owned),
            view: LoginView::Empty,
            progress: Vec::new(),
            pending_input: false,
            cancelled: false,
        }
    }

    pub fn show_auth(
        &mut self,
        url: impl Into<String>,
        instructions: Option<String>,
    ) -> LoginAction {
        let url = url.into();
        self.view = LoginView::Auth {
            url: url.clone(),
            instructions,
        };
        LoginAction::OpenBrowser(url)
    }

    pub fn show_device_code(&mut self, uri: impl Into<String>, code: impl Into<String>) {
        self.view = LoginView::DeviceCode {
            verification_uri: uri.into(),
            user_code: code.into(),
        };
    }

    pub fn show_prompt(&mut self, message: impl Into<String>, placeholder: Option<String>) {
        self.pending_input = true;
        self.view = LoginView::Prompt {
            message: message.into(),
            placeholder,
        };
    }

    pub fn show_info(&mut self, lines: Vec<String>) {
        self.view = LoginView::Info(lines);
    }
    pub fn show_waiting(&mut self, message: impl Into<String>) {
        self.view = LoginView::Waiting(message.into());
    }
    pub fn show_progress(&mut self, message: impl Into<String>) {
        self.progress.push(message.into());
    }

    pub fn submit(&mut self, value: impl Into<String>) -> LoginAction {
        if !self.pending_input || self.cancelled {
            return LoginAction::None;
        }
        self.pending_input = false;
        LoginAction::Submit(value.into())
    }

    pub fn cancel(&mut self) -> LoginAction {
        self.pending_input = false;
        self.cancelled = true;
        LoginAction::Cancel {
            success: false,
            message: "Login cancelled".into(),
        }
    }

    #[must_use]
    pub const fn is_cancelled(&self) -> bool {
        self.cancelled
    }
}
