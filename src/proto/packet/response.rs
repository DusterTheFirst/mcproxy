use super::RawTextComponent;
use serde::{Deserialize, Serialize};

/// The JSON response to a ping
#[derive(Serialize, Deserialize, Debug, Clone)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub struct StatusResponse {
    pub version: Version,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub players: Option<Players>,
    pub description: RawTextComponent,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub favicon: Option<String>,
}

/// The version part of the JSON response to a ping
#[derive(Serialize, Deserialize, Debug, Clone)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub struct Version {
    pub name: String,
    pub protocol: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub struct Players {
    pub max: u32, // Max supported by vanilla server is 2^31 - 1
    pub online: u32,
    #[serde(default)]
    pub sample: Vec<Player>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub struct Player {
    pub name: String,
    pub id: String,
}
