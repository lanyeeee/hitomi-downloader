use std::path::{Path, PathBuf};

use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager};

use crate::types::{DownloadFormat, ProxyMode, ProxyProtocol};

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub download_dir: PathBuf,
    pub export_dir: PathBuf,
    pub enable_file_logger: bool,
    pub download_format: DownloadFormat,
    pub dir_fmt: String,
    pub proxy_host: String,
    pub proxy_mode: ProxyMode,
    pub proxy_port: u16,
    #[serde(default)]
    pub proxy_protocol: ProxyProtocol,
    #[serde(default)]
    pub proxy_username: String,
    #[serde(default)]
    pub proxy_password: String,
}

impl Config {
    pub fn new(app: &AppHandle) -> anyhow::Result<Config> {
        let app_data_dir = app.path().app_data_dir()?;
        let config_path = app_data_dir.join("config.json");

        let config = if config_path.exists() {
            let config_string = std::fs::read_to_string(config_path)?;
            match serde_json::from_str(&config_string) {
                // if deserialization succeeds, use the deserialized Config
                Ok(config) => config,
                // Otherwise, merge the default configuration with the configuration already in the file
                // to avoid resetting all configuration items when new configuration items are added in the new version
                // after the user upgrades to the new version
                Err(_) => Config::merge_config(&config_string, &app_data_dir),
            }
        } else {
            Config::default(&app_data_dir)
        };
        config.save(app)?;
        Ok(config)
    }

    /// Build the URL of the custom proxy without credentials, e.g. `http://127.0.0.1:7890` or `socks5h://127.0.0.1:7890`
    pub fn custom_proxy_endpoint(&self) -> String {
        format!(
            "{}://{}:{}",
            self.proxy_protocol.scheme(),
            self.proxy_host.trim(),
            self.proxy_port
        )
    }

    /// Build the URL of the custom proxy, including the percent-encoded credentials when a
    /// username is set, e.g. `socks5://user:pass@127.0.0.1:7890`.
    /// Never log this value, it may contain the proxy password.
    pub fn custom_proxy_url(&self) -> String {
        let endpoint = self.custom_proxy_endpoint();
        let username = self.proxy_username.trim();
        if username.is_empty() {
            return endpoint;
        }
        let username = utf8_percent_encode(username, NON_ALPHANUMERIC);
        let password = utf8_percent_encode(&self.proxy_password, NON_ALPHANUMERIC);
        // Replace `scheme://` with `scheme://username:password@`
        let (scheme, host_and_port) = endpoint.split_once("://").unwrap_or_default();
        format!("{scheme}://{username}:{password}@{host_and_port}")
    }

    pub fn save(&self, app: &AppHandle) -> anyhow::Result<()> {
        let app_data_dir = app.path().app_data_dir()?;
        let config_path = app_data_dir.join("config.json");
        let config_string = serde_json::to_string_pretty(self)?;
        std::fs::write(config_path, config_string)?;
        Ok(())
    }

    fn merge_config(config_string: &str, app_data_dir: &Path) -> Config {
        let Ok(mut json_value) = serde_json::from_str::<serde_json::Value>(config_string) else {
            return Config::default(app_data_dir);
        };
        let serde_json::Value::Object(ref mut map) = json_value else {
            return Config::default(app_data_dir);
        };
        let Ok(default_config_value) = serde_json::to_value(Config::default(app_data_dir)) else {
            return Config::default(app_data_dir);
        };
        let serde_json::Value::Object(default_map) = default_config_value else {
            return Config::default(app_data_dir);
        };
        for (key, value) in default_map {
            map.entry(key).or_insert(value);
        }
        let Ok(config) = serde_json::from_value(json_value) else {
            return Config::default(app_data_dir);
        };
        config
    }

    fn default(app_data_dir: &Path) -> Config {
        Config {
            download_dir: app_data_dir.join("download"),
            export_dir: app_data_dir.join("export"),
            enable_file_logger: true,
            download_format: DownloadFormat::Webp,
            dir_fmt: "{title} - {id}".to_string(),
            proxy_mode: ProxyMode::System,
            proxy_host: "127.0.0.1".to_string(),
            proxy_port: 7890,
            proxy_protocol: ProxyProtocol::Http,
            proxy_username: String::new(),
            proxy_password: String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn custom_proxy_config(username: &str, password: &str) -> Config {
        Config {
            download_dir: PathBuf::from("/tmp/download"),
            export_dir: PathBuf::from("/tmp/export"),
            enable_file_logger: false,
            download_format: DownloadFormat::Webp,
            dir_fmt: String::new(),
            proxy_host: " 127.0.0.1 ".to_string(),
            proxy_mode: ProxyMode::Custom,
            proxy_port: 7890,
            proxy_protocol: ProxyProtocol::Socks5h,
            proxy_username: username.to_string(),
            proxy_password: password.to_string(),
        }
    }

    #[test]
    fn custom_proxy_url_without_credentials() {
        let config = custom_proxy_config("", "");
        assert_eq!(config.custom_proxy_url(), "socks5h://127.0.0.1:7890");
        assert_eq!(config.custom_proxy_endpoint(), "socks5h://127.0.0.1:7890");
    }

    #[test]
    fn custom_proxy_url_with_credentials() {
        let config = custom_proxy_config("user", "p@ss:w/rd%");
        assert_eq!(
            config.custom_proxy_url(),
            "socks5h://user:p%40ss%3Aw%2Frd%25@127.0.0.1:7890"
        );
        assert!(reqwest::Proxy::all(config.custom_proxy_url()).is_ok());
        assert_eq!(config.custom_proxy_endpoint(), "socks5h://127.0.0.1:7890");
    }

    #[test]
    fn custom_proxy_url_encodes_username() {
        let config = custom_proxy_config("us er@dom", "secret");
        assert_eq!(
            config.custom_proxy_url(),
            "socks5h://us%20er%40dom:secret@127.0.0.1:7890"
        );
        assert!(reqwest::Proxy::all(config.custom_proxy_url()).is_ok());
    }

    #[test]
    fn custom_proxy_url_with_empty_password() {
        let config = custom_proxy_config("user", "");
        assert_eq!(config.custom_proxy_url(), "socks5h://user:@127.0.0.1:7890");
        assert!(reqwest::Proxy::all(config.custom_proxy_url()).is_ok());
    }

    #[test]
    fn proxy_protocol_schemes() {
        assert_eq!(ProxyProtocol::Http.scheme(), "http");
        assert_eq!(ProxyProtocol::Socks5.scheme(), "socks5");
        assert_eq!(ProxyProtocol::Socks5h.scheme(), "socks5h");
    }

    #[test]
    fn custom_proxy_url_for_each_protocol() {
        for (protocol, expected) in [
            (ProxyProtocol::Http, "http://127.0.0.1:7890"),
            (ProxyProtocol::Socks5, "socks5://127.0.0.1:7890"),
            (ProxyProtocol::Socks5h, "socks5h://127.0.0.1:7890"),
        ] {
            let mut config = custom_proxy_config("", "");
            config.proxy_protocol = protocol;
            assert_eq!(config.custom_proxy_url(), expected);
            assert!(reqwest::Proxy::all(config.custom_proxy_url()).is_ok());
        }
    }

    #[test]
    fn legacy_config_without_proxy_fields_uses_defaults() {
        let json = serde_json::json!({
            "downloadDir": "/tmp/download",
            "exportDir": "/tmp/export",
            "enableFileLogger": true,
            "downloadFormat": "Webp",
            "dirFmt": "{title} - {id}",
            "proxyMode": "System",
            "proxyHost": "127.0.0.1",
            "proxyPort": 7890,
        });
        let config: Config = serde_json::from_value(json).unwrap();
        assert_eq!(config.proxy_protocol, ProxyProtocol::Http);
        assert!(config.proxy_username.is_empty());
        assert!(config.proxy_password.is_empty());
    }
}
