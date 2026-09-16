// @zedflow-context 2
use zf_context::context::*;
use zf_core::types::DataType;
use std::collections::BTreeMap;

pub fn strategy() -> ContextStrategy {
    ContextStrategy::new_v2("{{id}}", "{{id}}")
        .require("input", DataType::Text)
        .with_program(vec![ContextBlock::emit(
            "input",
            FragmentRole::Data,
            FragmentFormat::Text,
            ContextExpr::resource("input"),
        )])
}
