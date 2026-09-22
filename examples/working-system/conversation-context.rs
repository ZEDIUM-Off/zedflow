// @zedflow-context 2
use zf_context::context::*;
use zf_core::types::DataType;
use std::collections::BTreeMap;

pub fn strategy() -> ContextStrategy {
    ContextStrategy::new_v2("conversation-default", "Conversation")
        .define_type("ConversationMessage", DataType::Record { fields: BTreeMap::from([("parts".into(), DataType::List { item: Box::new(DataType::Record { fields: BTreeMap::from([]) }) }), ("role".into(), DataType::Text)]) })
        .define_type("SelectedFiles", DataType::Text)
        .define_type("SkillCatalog", DataType::Text)
        .define_type("UserInput", DataType::Text)
        .define_type("WorkspaceInstructions", DataType::Text)
        .require("files", DataType::Named { name: "SelectedFiles".into() })
        .require("history", DataType::List { item: Box::new(DataType::Named { name: "ConversationMessage".into() }) })
        .require("input", DataType::Named { name: "UserInput".into() })
        .require("instructions", DataType::Named { name: "WorkspaceInstructions".into() })
        .require("skills", DataType::Named { name: "SkillCatalog".into() })
        .with_program(vec![
            ContextBlock::emit("instructions", FragmentRole::Instruction, FragmentFormat::Text,
                ContextExpr::resource("instructions")
            ),
            ContextBlock::emit("skills", FragmentRole::Instruction, FragmentFormat::Text,
                ContextExpr::resource("skills")
            ),
            ContextBlock::emit("files", FragmentRole::Data, FragmentFormat::Text,
                ContextExpr::resource("files")
            ),
            ContextBlock::branch("conversation",
                ContextPredicate::present(ContextExpr::resource("history")),
                vec![
                    ContextBlock::for_each("history-messages", ContextExpr::resource("history"), "message", vec![
                        ContextBlock::emit("history", FragmentRole::Data, FragmentFormat::AdkMessages,
                            ContextExpr::list(DataType::Named { name: "ConversationMessage".into() }, vec![ContextExpr::variable("message")])
                        ),
                    ]),
                ],
                vec![
                    ContextBlock::emit("input", FragmentRole::Data, FragmentFormat::Text,
                        ContextExpr::resource("input")
                    ),
                ]
            ),
        ])
}
