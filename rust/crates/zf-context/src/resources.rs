//! Captured context program and declarative resource bindings.
//! Acquisition, activation and production are responsibilities of host adapters.
use crate::{context::ContextStrategy, context_library::ContextLibrary};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use zf_core::{
    identity::{Revision, Scope},
    types::TypeRegistry,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContextProgram {
    pub strategy: ContextStrategy,
    #[serde(default)]
    pub types: TypeRegistry,
    pub source: String,
    pub hash: String,
    #[serde(default)]
    pub bindings: BTreeMap<String, ResourceBinding>,
    #[serde(default)]
    pub library: ContextLibrary,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub library_sources: Vec<Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub type_sources: Vec<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<super::window_preparation::WindowPreparation>,
}
impl ContextProgram {
    /// Strategy text alone does not identify linked types, libraries or bindings.
    pub fn revision(&self) -> Result<String> {
        use sha2::Digest;
        Ok(format!(
            "{:x}",
            sha2::Sha256::digest(serde_json::to_vec(self)?)
        ))
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InputEncoding {
    AdkMessages,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ResourceBinding {
    State {
        field: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pointer: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        encoding: Option<InputEncoding>,
    },
    Attachments {
        slot: String,
    },
    /// Explicitly compile the conversation plus the current unconsumed input.
    Conversation {
        history_field: String,
        input_field: String,
    },
    Attachment {
        item_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        skill_name: Option<String>,
    },
    Entity {
        scope: Scope,
        alias: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        revision: Option<Revision>,
    },
    Produced {
        producer: super::context_resources::ResourceProducer,
    },
    Reader {
        reader: String,
        input: super::resource_readers::ReaderInput,
    },
}
