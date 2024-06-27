use std::{fmt::Debug, path::PathBuf};

use serde::{de::DeserializeOwned, Deserialize};

use crate::proto::response::StatusResponse;

mod private {
    pub trait Sealed {}
}

pub trait Marker: private::Sealed {
    type PointerType: DeserializeOwned + Debug;
}

pub struct Raw {}
impl private::Sealed for Raw {}
impl Marker for Raw {
    type PointerType = PathBuf;
}
impl<'d> Deserialize<'d> for Raw {
    fn deserialize<D>(_: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'d>,
    {
        Ok(Self {})
    }
}

pub struct Elaborated {}
impl private::Sealed for Elaborated {}
impl Marker for Elaborated {
    type PointerType = StatusResponse;
}
impl<'d> Deserialize<'d> for Elaborated {
    fn deserialize<D>(_: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'d>,
    {
        Ok(Self {})
    }
}