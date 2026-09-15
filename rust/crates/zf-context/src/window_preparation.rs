//! Immutable window preparation settings and explicit selection commands.
//! Queueing, claiming and invoking preparation routes belong to execution adapters.
use crate::{
    context::{ContextItem, ContextValue},
    window::WindowItem,
};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use zf_core::identity::Revision;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreparationRoute {
    pub branch: String,
    pub route_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WindowPreparation {
    pub alias: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prepare: Option<PreparationRoute>,
}
impl WindowPreparation {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            !self.alias.trim().is_empty(),
            "A prepared window requires its own alias"
        );
        if let Some(route) = &self.prepare {
            ensure!(
                !route.branch.is_empty() && !route.route_id.is_empty(),
                "Window preparation requires an explicit branch and routeId"
            );
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WindowSelectionCommand {
    pub id: String,
    pub node_path: String,
    pub alias: String,
    pub revision: Revision,
    pub program_hash: String,
}

pub struct PreparedInvocationWindow {
    pub items: Option<Vec<ContextItem>>,
    pub manifest: Value,
    pub wait: Option<Value>,
}

pub fn items(values: Vec<WindowItem>) -> Vec<ContextItem> {
    values
        .into_iter()
        .map(|item| match item {
            WindowItem::Group {
                id,
                label,
                items: children,
            } => ContextItem::Group {
                id,
                label,
                items: items(children),
            },
            WindowItem::Fragment {
                id,
                role,
                format,
                value,
                sources,
            } => ContextItem::Fragment {
                id,
                role,
                format,
                value: ContextValue::Shared {
                    root: Arc::new(value),
                    pointer: String::new(),
                },
                sources,
            },
        })
        .collect()
}
