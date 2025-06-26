use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Default, Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Type)]
pub enum ProxyMode {
    #[default]
    System,
    NoProxy,
    Custom,
}
