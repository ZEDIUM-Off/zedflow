//! Package-level exports matching Pi's coding-agent index.

pub use crate::cli::{Args, Mode, parse_args};
pub use crate::cli::{InitialMessageResult, build_initial_message};
pub use crate::modes::{
    AssistantResult, PrintModeOptions, PrintOutputMode, RpcClient, RpcCommand, RpcResponse,
};
pub use crate::utils::frontmatter::{parse_frontmatter, strip_frontmatter};
