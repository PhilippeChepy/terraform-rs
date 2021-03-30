use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};

#[derive(Debug, Deserialize, Serialize)]
pub struct TerraformEvent {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default)]
    pub change: Vec<TerraformResourceChange>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub status: Option<TerraformResourceStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub resource_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub id_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub id_value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub create_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub update_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub delete_count: Option<u32>,
    pub command: String,
    pub source: String,
    pub source_stream: TerraformSourceStream,
}

impl Default for TerraformEvent {
    fn default() -> Self {
        Self {
            change: Vec::new(),
            status: None,
            resource_path: None,
            id_key: None,
            id_value: None,
            create_count: None,
            update_count: None,
            delete_count: None,
            command: String::new(),
            source: String::new(),
            source_stream: TerraformSourceStream::Stdout,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub enum TerraformResourceChange {
    Create,
    Read,
    Update,
    Destroy,
    Replace,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub enum TerraformResourceStatus {
    Planned,
    Started,
    InProgress,
    Done,
    Completed,
}

#[derive(Debug, Deserialize_repr, Serialize_repr)]
#[repr(u8)]
pub enum TerraformSourceStream {
    Stdout = 1,
    Stderr = 2,
}
