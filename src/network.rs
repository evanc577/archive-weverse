use crate::config::Config;
use reqwest::header;
use serde::Deserialize;
use std::collections::HashMap;

#[allow(unused_macros)]
macro_rules! info_url {
    () => {
        "https://weversewebapi.weverse.io/wapi/v1/communities/info"
    };
}
#[allow(unused_macros)]
macro_rules! artist_tab {
    () => {
        "https://weversewebapi.weverse.io/wapi/v1/communities/\
         {artist_id}/posts/artistTab?pageSize={page_size}&from={from}"
    };
}
#[allow(unused_macros)]
macro_rules! media_tab {
    () => {
        "https://weversewebapi.weverse.io/wapi/v1/stream/\
         community/{artist_id}/mediaTab?pageSize={page_size}&from={from}"
    };
}
#[allow(unused_macros)]
macro_rules! to_fans {
    () => {
        "https://weversewebapi.weverse.io/wapi/v1/stream/community/\
                               {artist_id}/toFans?pageSize={page_size}&from={from}"
    };
}
#[allow(unused_macros)]
macro_rules! post_url {
    () => {
        "https://weversewebapi.weverse.io/wapi/v1/communities/\
         {artist_id}/posts/{post_id}";
    };
}
#[allow(unused_macros)]
macro_rules! video_dash_url {
    () => {
        "https://cdn-media.weverse.io/video{video_id}/DASH.mpd"
    };
}

pub struct Network<'a> {
    config: &'a Config,
    token: &'a str,
    client: reqwest::Client,
    artist_id_map: HashMap<String, i64>,
}

impl Network<'_> {
    pub fn new<'a>(config: &'a Config, token: &'a str) -> Result<Network<'a>, String> {
        // create client with appropriate authorization header
        let mut headers = header::HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            header::HeaderValue::from_str(&format!("Bearer {}", &token)[..])
                .map_err(|e| format!("Error constructing request header: {}", e))?,
        );
        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .map_err(|e| format!("Error building request client: {}", e))?;

        let n = Network {
            config: config,
            token: token,
            client: client,
            artist_id_map: HashMap::new(),
        };
        Ok(n)
    }

    async fn get_artist_id(&mut self, artist: &str) -> Result<i64, String> {
        #[derive(Deserialize)] struct InfoResp {
            communities: Vec<Community>,
        }
        #[derive(Deserialize)]
        struct Community {
            name: String,
            id: i64,
        }

        // get artist id mapping from server if we don't already have it
        if !self.artist_id_map.contains_key(&artist.to_lowercase()) {
            let url = info_url!();
            let resp = self
                .client
                .get(url)
                .send()
                .await
                .map_err(|e| format!("Error sending info request: {}", e))?;

            let info: InfoResp =
                serde_json::from_str(&resp.text().await.map_err(|e| {
                    format!("Error reading response text for {}: {}", url, e)
                })?)
                .map_err(|e| format!("Error parsing json for {}: {}", url, e))?;

            for c in &info.communities {
                self.artist_id_map.insert(c.name.to_lowercase(), c.id);
            }
        }

        let id = self
            .artist_id_map
            .get(&artist.to_lowercase())
            .ok_or(format!("Error reading artist_id_map"))?;
        Ok(*id)
    }
}
