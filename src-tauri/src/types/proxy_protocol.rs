use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Default, Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Type)]
pub enum ProxyProtocol {
    #[default]
    Http,
    Socks5,
    Socks5h,
}

impl ProxyProtocol {
    /// URL scheme used to build the proxy address.
    /// `socks5` resolves DNS locally, `socks5h` lets the proxy resolve it.
    pub fn scheme(&self) -> &str {
        match self {
            ProxyProtocol::Http => "http",
            ProxyProtocol::Socks5 => "socks5",
            ProxyProtocol::Socks5h => "socks5h",
        }
    }
}
