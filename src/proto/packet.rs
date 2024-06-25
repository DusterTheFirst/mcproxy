use serde::{Deserialize, Serialize};
use std::fmt::{self, Display, Formatter};

#[derive(Debug)]
pub struct Packet {
    pub length: i32,
    pub id: i32,
    pub data: Vec<u8>,
}

impl Display for Packet {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        writeln!(f, "Packet Info:")?;
        writeln!(f, "    Length: {:?} bytes", self.length)?;
        writeln!(f, "    ID: 0x{:02X}", self.id)?;
        write!(f, "    Data {:X?}", self.data)?;

        Ok(())
    }
}

#[derive(Debug)]
pub struct Handshake {
    pub protocol_version: i32,
    pub address: String,
    pub port: u16,
    pub next_state: NextState,
}

// TODO: Debug shows large view, DISPLAY hides data
impl Display for Handshake {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        writeln!(f, "Handshake Data:")?;
        writeln!(f, "    Protocol version: {}", self.protocol_version)?;
        writeln!(f, "    Address: {}", self.address)?;
        writeln!(f, "    Port: {}", self.port)?;
        write!(f, "    Next State: {:?}", self.next_state)?;

        Ok(())
    }
}

/// Response packet structs
pub mod response {
    use super::TextComponent;
    use serde::{Deserialize, Serialize};

    /// The JSON response to a ping
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct Response {
        pub version: Version,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub players: Option<Players>,
        pub description: TextComponent,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub favicon: Option<String>,
    }

    /// The version part of the JSON response to a ping
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct Version {
        pub name: String,
        pub protocol: i32,
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct Players {
        pub max: u16,
        pub online: u16,
        pub sample: Vec<Player>,
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct Player {
        pub name: String,
        pub id: String,
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
/// A minecraft chat object
pub enum TextComponent {
    ChatObject {
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        bold: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        italic: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        underlined: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        strikethrough: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        obfuscated: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        color: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        extra: Option<Vec<TextComponent>>,
    },
    String(String),
}

#[derive(Debug)]
pub enum NextState {
    Ping,
    Login,
    Transfer,
    Unknown(i32),
}

impl From<i32> for NextState {
    fn from(num: i32) -> NextState {
        match num {
            1 => NextState::Ping,
            2 => NextState::Login,
            3 => NextState::Transfer,
            _ => NextState::Unknown(num),
        }
    }
}

impl Display for NextState {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            NextState::Ping => f.write_str("ping"),
            NextState::Login => f.write_str("login"),
            NextState::Transfer => f.write_str("transfer"),
            NextState::Unknown(id) => f.write_fmt(format_args!("{id}")),
        }
    }
}