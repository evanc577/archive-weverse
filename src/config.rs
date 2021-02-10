use regex::Regex;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub cookies_file: String,
    #[serde(default = "default_keep_open")]
    pub keep_open: bool,
    #[serde(default = "default_num_processes")]
    pub max_connections: usize,
    pub artists: HashMap<String, ArtistConfig>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ArtistConfig {
    pub artist_download_path: Option<String>,
    pub moments_download_path: Option<String>,
    pub videos_download_path: Option<String>,
    pub recent_artist: Option<isize>,
    pub recent_moments: Option<isize>,
    pub recent_videos: Option<isize>,
}

fn default_keep_open() -> bool {
    false
}
fn default_num_processes() -> usize {
    20
}

pub fn read_config() -> Result<Config, String> {
    let conf_contents = fs::read_to_string("config.toml")
        .map_err(|e| format!("Error reading config.toml: {}", e))?;
    let conf: Config =
        toml::from_str(&conf_contents).map_err(|e| format!("Error parsing config.yml: {}", e))?;
    // println!("config: {:#?}", conf);
    Ok(conf)
}

pub fn read_token(cookies_file: &str) -> Result<String, String> {
    lazy_static! {
        static ref RE: Regex = Regex::new(
            r"(?m)^(?P<domain>\.weverse\.io)\t.+?\t.+?\t.+?\t.+?\t(?P<name>we_access_token)\t(?P<value>.+?)$"
        ).unwrap();
    }

    let cookies_contents = fs::read_to_string(&cookies_file)
        .map_err(|e| format!("Error reading {}: {}", &cookies_file, e))?;

    let token = RE
        .captures(&cookies_contents)
        .ok_or(format!("Error parsing {}", &cookies_file))?
        .name("value")
        .ok_or(format!("Error applying regex for {}", &cookies_file))?
        .as_str()
        .to_owned();

    // println!("token: {:?}", token);
    Ok(token)
}
