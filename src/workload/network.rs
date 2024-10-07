use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Network {
    /// Whether the instance functions as a server or client
    pub server: bool,

    /// Which ip address to use for the server to listen on,
    /// or for the client to connect to
    pub address: (u8, u8, u8, u8),

    /// Port for the server to listen on, or for the client
    /// to connect to.
    pub target_port: u16,

    /// Starting number of connections
    pub nconnections: u32,

    /// How often send data via new connections, in milliseconds.
    /// The interval is applied for all connections, e.g. an interval
    /// of 100 ms for 100 connections means that every 100 ms one out
    /// of 100 connections will be allowed to send some data.
    /// This parameter allows to control the overhead of sending data,
    /// so that it will not impact connections monitoring.
    #[serde(default = "default_network_send_interval")]
    pub send_interval: u128,
}

fn default_network_send_interval() -> u128 {
    10
}
