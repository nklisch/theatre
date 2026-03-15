pub mod assertions;
pub mod backend;
pub mod cli_backend;
pub mod dispatch;
pub mod godot_process;
pub mod macros;
pub mod mcp_backend;

pub use backend::{LiveBackend, ToolResult};
pub use cli_backend::CliBackend;
pub use godot_process::LiveGodotProcess;
pub use mcp_backend::McpBackend;
