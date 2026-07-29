use std::sync::Arc;

use zedflow_coding_agent::extensions::{create_extension_runtime, define_tool};

#[test]
fn extension_runtime_registers_a_callable_tool() {
    let mut runtime = create_extension_runtime();
    runtime.register_tool(
        define_tool("summary", "summarize a session"),
        Arc::new(|_, _| Ok(serde_json::Value::Null)),
    );
    assert_eq!(runtime.tools.len(), 1);
    assert_eq!(runtime.registered_tools.len(), 1);
}
