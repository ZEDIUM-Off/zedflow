use zedflow_coding_agent::{
    export_extension,
    sdk::{Extension, ExtensionApi, JsonValue},
};

#[derive(Default)]
struct Fixture;

impl Extension for Fixture {
    fn initialize(&mut self, api: &mut ExtensionApi, _: JsonValue) -> Result<(), String> {
        api.register_tool("fixture-tool");
        api.register_command("fixture-command");
        api.register_provider("fixture-provider", serde_json::json!({"model":"fixture"}));
        api.on_event("session_start");
        api.show_ui(serde_json::json!({"text":"fixture UI"}));
        Ok(())
    }

    fn invoke(&mut self, api: &mut ExtensionApi, request: JsonValue) -> Result<JsonValue, String> {
        api.on_event("tool_call");
        Ok(serde_json::json!({"echo":request}))
    }

    fn shutdown(&mut self, api: &mut ExtensionApi) -> Result<(), String> {
        api.on_event("shutdown");
        Ok(())
    }
}

export_extension!(Fixture);
