// @zedflow-bridge 1
use zf_flows::composition::*;

pub fn bridge() -> BridgeDefinition {
    BridgeDefinition::new()
        .import("documentation", "documentation")
        .connect("documentation", Connection::new(Endpoint::new("root", "documentation"), Endpoint::new("documentation", "main"), RouteMode::CallAwait, InvocationKind::Node))
}
