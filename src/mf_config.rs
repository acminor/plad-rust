use serde_derive::Deserialize;
use std::fs;

#[derive(Debug, Deserialize)]
pub struct MFConfig {
    pub python_plugin_directories: Vec<String>;
    pub data_directories: Vec<String>;
}

pub load_mfconfig() -> MFConfig {
    let config_path = "./mf_config.toml";

    let contents =
        fs::read_to_string(&config_path).expect("Failed to read mf_config.toml");

    toml::from_str(&contents).expect("Failed to parse mf_config TOML")
}
