use mcproxy_model::Hostname;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use std::{
    collections::VecDeque,
    fmt::{self, Display, Formatter},
};
use tracing::warn;

/// Response packet structs
pub mod response;

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
    pub address_forge_version: Option<SmolStr>,
    pub port: u16,
    pub next_state: NextState,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(untagged)]
/// A minecraft chat object
pub enum RawTextComponent {
    Object(RawTextComponentObject),
    Array(Vec<RawTextComponent>),
    String(String),
}

impl From<RawTextComponent> for RawTextComponentObject {
    fn from(value: RawTextComponent) -> Self {
        match value {
            // A component specified as an object consists of one or more content fields, and any number of styling fields.
            RawTextComponent::Object(object) => object,
            // A component specified as an array is interpreted as the first element of the array, with
            // the rest of the elements then appended to the component's extra field. This can produce
            // unusual behavior with style inheritance, but it is still useful shorthand.
            RawTextComponent::Array(array) => {
                // Turn the array into a circular buffer in order to take ownership of its first element.
                let mut array = VecDeque::from(array);
                let Some(first) = array.pop_front() else {
                    return Default::default();
                };

                let mut first = RawTextComponentObject::from(first);

                if !array.is_empty() {
                    let extra = first.extra.get_or_insert_with(Vec::new);

                    let (start, end) = array.as_slices();

                    extra.extend_from_slice(start);
                    extra.extend_from_slice(end);
                }

                first
            }
            // A component specified as a string is interpreted equivalently to {"text":"string"}.
            RawTextComponent::String(text) => RawTextComponentObject {
                text,
                ..Default::default()
            },
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub struct RawTextComponentObject {
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
    pub extra: Option<Vec<RawTextComponent>>,
}

#[derive(Debug, Clone, Default)]
pub struct ElaboratedTextComponent {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub underlined: bool,
    pub strikethrough: bool,
    pub obfuscated: bool,
    pub color: Option<Color>,
}

impl ElaboratedTextComponent {
    pub fn from_text_component(component: RawTextComponent) -> Vec<ElaboratedTextComponent> {
        Self::from_text_component_object(RawTextComponentObject::from(component))
    }

    pub fn from_text_component_object(
        object: RawTextComponentObject,
    ) -> Vec<ElaboratedTextComponent> {
        #[derive(Default)]
        struct ParentSettings {
            bold: Option<bool>,
            italic: Option<bool>,
            underlined: Option<bool>,
            strikethrough: Option<bool>,
            obfuscated: Option<bool>,
            color: Option<Color>,
        }

        fn merge<T>(parent: Option<T>, current: Option<T>) -> Option<T> {
            match (parent, current) {
                (None, None) => None,
                (_, Some(current)) => Some(current),
                (Some(parent), None) => Some(parent),
            }
        }

        fn recurse(
            vec: &mut Vec<ElaboratedTextComponent>,
            parent: &ParentSettings,
            object: RawTextComponentObject,
        ) {
            let current_settings = ParentSettings {
                bold: merge(parent.bold, object.bold),
                italic: merge(parent.italic, object.italic),
                underlined: merge(parent.underlined, object.underlined),
                strikethrough: merge(parent.strikethrough, object.strikethrough),
                obfuscated: merge(parent.obfuscated, object.obfuscated),
                color: merge(parent.color.as_ref().cloned(), object.color),
            };

            vec.push(ElaboratedTextComponent {
                text: object.text,
                bold: current_settings.bold.unwrap_or(false),
                italic: current_settings.italic.unwrap_or(false),
                underlined: current_settings.underlined.unwrap_or(false),
                strikethrough: current_settings.strikethrough.unwrap_or(false),
                obfuscated: current_settings.obfuscated.unwrap_or(false),
                color: current_settings.color.clone(),
            });

            for extra in object.extra.unwrap_or_default() {
                recurse(vec, &current_settings, RawTextComponentObject::from(extra));
            }
        }

        let mut vec = Vec::new();

        recurse(&mut vec, &ParentSettings::default(), object);

        vec
    }

    pub fn decode_formatting_codes(string: &str) -> Vec<ElaboratedTextComponent> {
        let mut components = Vec::new();

        #[derive(Debug)]
        enum State {
            Text { start: usize },
            FormattingCode,
        }

        let mut state = State::Text { start: 0 };
        let mut current_component = ElaboratedTextComponent::default();

        for (i, char) in string.char_indices() {
            match state {
                State::Text { start } => {
                    if char == '§' {
                        if i != 0 {
                            components.push(ElaboratedTextComponent {
                                text: String::from(&string[start..i]),
                                ..current_component.clone()
                            });
                        }

                        state = State::FormattingCode;
                    }
                }
                State::FormattingCode => {
                    if let Some(color) = ColorName::from_code(char) {
                        current_component.color = Some(Color::Named(color));
                    } else {
                        match char {
                            'k' => current_component.obfuscated = true,
                            'l' => current_component.bold = true,
                            'm' => current_component.strikethrough = true,
                            'n' => current_component.underlined = true,
                            'o' => current_component.italic = true,
                            'r' => current_component = Default::default(),
                            _ => {
                                warn!(code = %char, "attempted to decode invalid formatting code")
                            }
                        }
                    }

                    state = State::Text {
                        start: i + char.len_utf8(),
                    }
                }
            }
        }

        if let State::Text { start } = state {
            components.push(ElaboratedTextComponent {
                text: String::from(&string[start..]),
                ..current_component.clone()
            });
        }

        components
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(untagged)]
pub enum Color {
    Named(ColorName),
    Hex(smol_str::SmolStr),
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

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
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
    pub fn to_code(self) -> char {
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
