use std::{fmt::Debug, path::PathBuf};

use serde::{de::DeserializeOwned, Deserialize};

use crate::proto::response::StatusResponse;

mod private {
    pub trait Sealed {}
}

pub trait Marker: private::Sealed {
    #[cfg(not(test))]
    type PointerType: DeserializeOwned + Debug;

    #[cfg(test)]
    type PointerType: DeserializeOwned + schemars::JsonSchema + Debug;
}

#[derive(Deserialize, Debug)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub struct Raw {}
impl private::Sealed for Raw {}
impl Marker for Raw {
    type PointerType = PathBuf;
}

#[derive(Deserialize, Debug)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub struct Elaborated {}
impl private::Sealed for Elaborated {}
impl Marker for Elaborated {
    type PointerType = StatusResponse;
}
