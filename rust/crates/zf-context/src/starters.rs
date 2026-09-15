//! Pure starter strategies and declarative bindings. Installation belongs to CLI/storage.
use crate::context::{
    ContextBlock, ContextCapability, ContextExpr, ContextPredicate, ContextStrategy,
    FragmentFormat, FragmentRole,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use zf_core::types::DataType;

pub fn strategy(id: &str, name: &str, tools: &[&str]) -> ContextStrategy {
    let named = |name: &str| DataType::Named { name: name.into() };
    let mut strategy = ContextStrategy::new_v2(id, name)
        // These bindings produce aggregated text, not the gallery's individual
        // Instruction/Skill/Document records. Preserve that explicit contract.
        .define_type("WorkspaceInstructions", DataType::Text)
        .define_type("SkillCatalog", DataType::Text)
        .define_type("SelectedFiles", DataType::Text)
        .define_type("UserInput", DataType::Text)
        .define_type(
            "ConversationMessage",
            DataType::Record {
                fields: BTreeMap::from([
                    ("role".into(), DataType::Text),
                    (
                        "parts".into(),
                        DataType::List {
                            item: Box::new(DataType::Record {
                                fields: BTreeMap::new(),
                            }),
                        },
                    ),
                ]),
            },
        )
        .require("instructions", named("WorkspaceInstructions"))
        .require("skills", named("SkillCatalog"))
        .require("files", named("SelectedFiles"))
        .require("input", named("UserInput"))
        .require(
            "history",
            DataType::List {
                item: Box::new(named("ConversationMessage")),
            },
        );
    for tool in tools {
        strategy = strategy.capability(ContextCapability::new(
            tool,
            DataType::Record {
                fields: BTreeMap::new(),
            },
            DataType::Record {
                fields: BTreeMap::new(),
            },
        ));
    }
    let emit =
        |id: &str, role, format| ContextBlock::emit(id, role, format, ContextExpr::resource(id));
    strategy.with_program(vec![
        emit(
            "instructions",
            FragmentRole::Instruction,
            FragmentFormat::Text,
        ),
        emit("skills", FragmentRole::Instruction, FragmentFormat::Text),
        emit("files", FragmentRole::Data, FragmentFormat::Text),
        ContextBlock::branch(
            "conversation",
            ContextPredicate::present(ContextExpr::resource("history")),
            vec![ContextBlock::for_each(
                "history-messages",
                ContextExpr::resource("history"),
                "message",
                // Forward the whole message. Rebuilding only its text would
                // discard thinking signatures and tool-call/result identities.
                vec![ContextBlock::emit(
                    "history",
                    FragmentRole::Data,
                    FragmentFormat::AdkMessages,
                    ContextExpr::list(
                        named("ConversationMessage"),
                        vec![ContextExpr::variable("message")],
                    ),
                )],
            )],
            vec![emit("input", FragmentRole::Data, FragmentFormat::Text)],
        ),
    ])
}

/// The Harness explicitly projects the current branch result, independently of
/// files and conversation. Other default strategies retain their input contract.
pub fn harness_strategy() -> ContextStrategy {
    let mut result = strategy(
        "harness-default",
        "Harness de workspace",
        &["read", "write", "edit", "exec"],
    )
    .define_type("RouteResult", DataType::Text)
    .require(
        "routeResult",
        DataType::Named {
            name: "RouteResult".into(),
        },
    );
    result.program.insert(
        3,
        ContextBlock::branch(
            "route-result-available",
            ContextPredicate::all(vec![
                ContextPredicate::present(ContextExpr::resource("routeResult")),
                ContextPredicate::negate(ContextPredicate::equal(
                    ContextExpr::resource("routeResult"),
                    ContextExpr::literal(
                        DataType::Named {
                            name: "RouteResult".into(),
                        },
                        json!(""),
                    ),
                )),
            ]),
            vec![ContextBlock::emit(
                "route-result",
                FragmentRole::Data,
                FragmentFormat::Text,
                ContextExpr::resource("routeResult"),
            )],
            vec![],
        ),
    );
    result
}

pub fn bindings() -> Value {
    json!({
        "instructions":{"kind":"attachments","slot":"instructions"},
        "skills":{"kind":"attachments","slot":"skills"},
        "files":{"kind":"attachments","slot":"files"},
        "history":{"kind":"conversation","historyField":"messages","inputField":"input"},
        "input":{"kind":"state","field":"input"}
    })
}
