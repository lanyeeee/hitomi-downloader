use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Default, Debug, Copy, Clone, PartialEq, Serialize, Deserialize, Type)]
pub enum DownloadFormat {
    #[default]
    Webp,
    Avif,
}
impl DownloadFormat {
    pub fn to_extension(&self) -> &str {
        match self {
            DownloadFormat::Webp => "webp",
            DownloadFormat::Avif => "avif",
        }
    }
}
