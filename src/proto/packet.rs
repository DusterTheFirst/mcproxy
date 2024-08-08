use mcproxy_model::Hostname;
use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    fmt::{self, Display, Formatter},
};

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
    pub address: Hostname,
    pub address_forge_version: Option<String>,
    pub port: u16,
    pub next_state: NextState,
}

/// Response packet structs
pub mod response {
    use super::TextComponent;
    use serde::{Deserialize, Serialize};

    /// The JSON response to a ping
    #[derive(Serialize, Deserialize, Debug, Clone)]
    #[cfg_attr(test, derive(schemars::JsonSchema))]
    pub struct StatusResponse {
        pub version: Version,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub players: Option<Players>,
        pub description: TextComponent,
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
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(untagged)]
/// A minecraft chat object
pub enum TextComponent {
    Object(TextComponentObject),
    Array(Vec<TextComponent>),
    String(String),
}

impl From<TextComponent> for TextComponentObject {
    fn from(value: TextComponent) -> Self {
        match value {
            // A component specified as an object consists of one or more content fields, and any number of styling fields.
            TextComponent::Object(object) => object,
            // A component specified as an array is interpreted as the first element of the array, with
            // the rest of the elements then appended to the component's extra field. This can produce
            // unusual behavior with style inheritance, but it is still useful shorthand.
            TextComponent::Array(array) => {
                // Turn the array into a circular buffer in order to take ownership of its first element.
                let mut array = VecDeque::from(array);
                let Some(first) = array.pop_front() else {
                    return Default::default();
                };

                let mut first = TextComponentObject::from(first);

                if !array.is_empty() {
                    let extra = first.extra.get_or_insert_with(Vec::new);

                    let (start, end) = array.as_slices();

                    extra.extend_from_slice(start);
                    extra.extend_from_slice(end);
                }

                first
            }
            // A component specified as a string is interpreted equivalently to {"text":"string"}.
            TextComponent::String(text) => TextComponentObject {
                text,
                ..Default::default()
            },
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub struct TextComponentObject {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bold: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub italic: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub underlined: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strikethrough: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub obfuscated: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<Vec<TextComponent>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(untagged)]
pub enum Color {
    Named(ColorName),
    Hex(String),
}

impl Color {
    pub fn foreground_color(&self) -> &str {
        match self {
            Color::Named(color_name) => color_name.foreground_color(),
            Color::Hex(color) => color,
        }
    }

    pub fn background_color(&self) -> &str {
        match self {
            Color::Named(color_name) => color_name.background_color(),
            Color::Hex(color) => color,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ColorName {
    Black,
    DarkBlue,
    DarkGreen,
    #[doc(alias = "DarkCyan")]
    DarkAqua,
    DarkRed,
    #[doc(alias = "Purple")]
    DarkPurple,
    Gold,
    Gray,
    DarkGray,
    Blue,
    Green,
    #[doc(alias = "Cyan")]
    Aqua,
    Red,
    #[doc(alias = "Pink")]
    LightPurple,
    Yellow,
    White,
}

impl ColorName {
    pub fn to_code(&self) -> char {
        match self {
            ColorName::Black => '0',
            ColorName::DarkBlue => '1',
            ColorName::DarkGreen => '2',
            ColorName::DarkAqua => '3',
            ColorName::DarkRed => '4',
            ColorName::DarkPurple => '5',
            ColorName::Gold => '6',
            ColorName::Gray => '7',
            ColorName::DarkGray => '8',
            ColorName::Blue => '9',
            ColorName::Green => 'a',
            ColorName::Aqua => 'b',
            ColorName::Red => 'c',
            ColorName::LightPurple => 'd',
            ColorName::Yellow => 'e',
            ColorName::White => 'f',
        }
    }

    pub fn from_code(code: char) -> Option<Self> {
        match code {
            '0' => Some(ColorName::Black),
            '1' => Some(ColorName::DarkBlue),
            '2' => Some(ColorName::DarkGreen),
            '3' => Some(ColorName::DarkAqua),
            '4' => Some(ColorName::DarkRed),
            '5' => Some(ColorName::DarkPurple),
            '6' => Some(ColorName::Gold),
            '7' => Some(ColorName::Gray),
            '8' => Some(ColorName::DarkGray),
            '9' => Some(ColorName::Blue),
            'a' => Some(ColorName::Green),
            'b' => Some(ColorName::Aqua),
            'c' => Some(ColorName::Red),
            'd' => Some(ColorName::LightPurple),
            'e' => Some(ColorName::Yellow),
            'f' => Some(ColorName::White),
            _ => None,
        }
    }

    pub fn foreground_color(&self) -> &'static str {
        match self {
            ColorName::Black => "#000000",
            ColorName::DarkBlue => "#0000aa",
            ColorName::DarkGreen => "#00aa00",
            ColorName::DarkAqua => "#00aaaa",
            ColorName::DarkRed => "#aa0000",
            ColorName::DarkPurple => "#aa00aa",
            ColorName::Gold => "#ffaa00",
            ColorName::Gray => "#aaaaaa",
            ColorName::DarkGray => "#555555",
            ColorName::Blue => "#5555ff",
            ColorName::Green => "#55ff55",
            ColorName::Aqua => "#55ffff",
            ColorName::Red => "#ff5555",
            ColorName::LightPurple => "#ff55ff",
            ColorName::Yellow => "#ffff55",
            ColorName::White => "#ffffff",
        }
    }

    pub fn background_color(&self) -> &'static str {
        match self {
            ColorName::Black => "#000000",
            ColorName::DarkBlue => "#00002a",
            ColorName::DarkGreen => "#002a00",
            ColorName::DarkAqua => "#002a2a",
            ColorName::DarkRed => "#2a0000",
            ColorName::DarkPurple => "#2a002a",
            ColorName::Gold => "#2a2a00",
            ColorName::Gray => "##2a2a2a",
            ColorName::DarkGray => "#151515",
            ColorName::Blue => "#15153f",
            ColorName::Green => "#153f15",
            ColorName::Aqua => "#153f3f",
            ColorName::Red => "#3f1515",
            ColorName::LightPurple => "#3f153f",
            ColorName::Yellow => "#3f3f15",
            ColorName::White => "#3f3f3f",
        }
    }
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

impl From<NextState> for i32 {
    fn from(val: NextState) -> Self {
        match val {
            NextState::Ping => 1,
            NextState::Login => 2,
            NextState::Transfer => 3,
            NextState::Unknown(x) => x,
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
