use std::{collections::HashMap, net::{IpAddr, SocketAddr}};

use serde::Deserialize;

use crate::proto::TextComponent;

use super::util::{Elaborated, Marker};

pub type Config = GenericConfig<Elaborated>;

#[derive(Deserialize, Debug)]
pub struct GenericConfig<T: Marker> {
    /// The config for the placeholder server
    pub placeholder_server: PlaceholderServerConfig<T>,
    /// The mapping of servers to their addresses
    pub servers: HashMap<String, SocketAddr>,
    /// Settings for the proxy server
    pub proxy: ProxyConfig,
}

#[derive(Deserialize, Debug)]
pub struct ProxyConfig {
    /// The port to bind to
    pub port: u16,
    /// The address to bind to
    pub address: IpAddr,
}

#[derive(Deserialize, Debug)]
pub struct PlaceholderServerConfig<T: Marker> {
    /// The message to use when kicking a user from the server
    pub kick_message: TextComponent, // TODO: remove?
    /// The responses config files
    pub responses: PlaceholderServerResponses<T>,
}

#[derive(Deserialize, Debug)]
pub struct PlaceholderServerResponses<T: Marker> {
    /// Response for server when mapping exists but connection failed
    pub offline: Option<T::PointerType>,
    /// Response for server when no mapping exists
    pub no_mapping: Option<T::PointerType>,
}