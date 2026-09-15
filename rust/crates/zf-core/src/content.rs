//! Portable content records. Encoding and persistence belong to storage.
use crate::identity::ContentRef;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContentBlob {
    pub reference: ContentRef,
    pub body: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentRecord {
    pub scope: String,
    pub kind: String,
    pub key: String,
    pub value_ref: ContentRef,
}
