use chrono::DateTime;
use futures::stream::{self, StreamExt};
use reqwest::header;
use serde::Deserialize;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::fs::{self, File};
use std::io::prelude::*;

use crate::config::Config;
use crate::network::network_structs::*;
use crate::network::urls::*;

mod network_structs;
mod urls;

pub async fn download(conf: &Config, token: &String) -> Result<(), String> {
    let n = Network::new(&conf, &token).await?;

    // get a list of all posts to download
    let artist_iter = conf
        .artists
        .iter()
        .map(|(k, v)| n.download_posts_info(&v.recent_artist, k.to_owned(), PostType::Artist));
    let moments_iter = conf
        .artists
        .iter()
        .map(|(k, v)| n.download_posts_info(&v.recent_moments, k.to_owned(), PostType::Moment));
    let posts = stream::iter(artist_iter.chain(moments_iter))
        .buffer_unordered(conf.max_connections)
        .collect::<Vec<Result<Vec<_>, _>>>()
        .await
        .into_iter()
        .collect::<Result<Vec<Vec<_>>, _>>()
        .map_err(|e| format!("Error collecting posts info: {}", e))?
        .into_iter()
        .flatten()
        .collect::<Vec<Post>>();
    println!("posts: {:#?}", &posts);

    // download the posts
    let posts_iter = posts.iter().map(|p| n.download_post(&p));
    let test = stream::iter(posts_iter)
        .buffer_unordered(conf.max_connections)
        .collect::<Vec<Result<_, _>>>()
        .await;

    Ok(())
}

async fn get_artist_id(client: &reqwest::Client) -> Result<HashMap<String, i64>, String> {
    #[derive(Deserialize)]
    struct InfoResp {
        communities: Vec<Community>,
    }
    #[derive(Deserialize)]
    struct Community {
        name: String,
        id: i64,
    }

    let url = INFO_URL;
    println!("Sending request to: {}", &url);
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Error sending info request: {}", e))?;

    let info: InfoResp = serde_json::from_str(
        &resp
            .text()
            .await
            .map_err(|e| format!("Error reading response text for {}: {}", url, e))?,
    )
    .map_err(|e| format!("Error parsing json for {}: {}", url, e))?;

    let artist_id_map: HashMap<String, i64> = info
        .communities
        .iter()
        .map(|c| (c.name.to_lowercase(), c.id))
        .collect();
    Ok(artist_id_map)
}

impl Network<'_> {
    async fn new<'a>(config: &'a Config, token: &str) -> Result<Network<'a>, String> {
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
        let anon_client = reqwest::Client::builder()
            .build()
            .map_err(|e| format!("Error building request client: {}", e))?;
        let artist_id_map = get_artist_id(&client).await?;

        let n = Network {
            config: config,
            client: client,
            anon_client: anon_client,
            artist_id_map: artist_id_map,
        };
        Ok(n)
    }

    async fn download_posts_info(
        &self,
        recent: &Option<isize>,
        artist: String,
        post_type: PostType,
    ) -> Result<Vec<Post>, String> {
        let artist_id = self
            .artist_id_map
            .get(&artist[..])
            .ok_or(format!("Error could not find {} in artist_id_map", &artist))?;

        // set up loop variables
        let mut from = String::from("");
        let mut count: isize = 0;
        let url = match post_type {
            PostType::Artist => ARTIST_TAB,
            PostType::Moment => TO_FANS,
        }
        .replace("{artist_id}", &artist_id.to_string()[..]);

        // return value
        let mut ret: Vec<Post> = Vec::new();

        loop {
            // build request
            let page_size: String = match recent {
                Some(v) => std::cmp::max(1, v - count).to_string(),
                None => String::from("100"),
            };
            let params = [("pageSize", &page_size), ("from", &from)];

            // send request
            println!("Sending request to: {}", &url);
            let resp = self
                .client
                .get(&url)
                .query(&params)
                .send()
                .await
                .map_err(|e| format!("Error sending request for {}: {}", &url, e))?;

            // parse response
            let text = &resp
                .text()
                .await
                .map_err(|e| format!("Error reading response text for {}: {}", &url, e))?;
            let posts_resp: Posts = serde_json::from_str(text)
                .map_err(|e| format!("Error parsing json for {}: {}", &url, e))?;
            let posts = &posts_resp.posts;
            let num_posts = isize::try_from(posts.len())
                .map_err(|e| format!("Error could not convert num posts to isize: {}", e))?;

            // add to return vector
            ret.extend(posts_resp.posts);

            // determine if we need to keep looping
            if posts_resp.is_ended {
                break;
            }
            match recent {
                Some(v) => {
                    if v - num_posts <= 0 {
                        break;
                    }
                    count += num_posts;
                }
                None => (),
            }
            from = posts_resp
                .last_id
                .ok_or("Error lastId not found")?
                .to_string();
        }
        Ok(ret)
    }

    async fn download_post(&self, post: &Post) -> Result<String, String> {
        let date = DateTime::parse_from_rfc3339(&post.created_at)
            .unwrap()
            .format("%Y%m%d");
        let id = post.id.to_string();
        let user = &post.community_user.nickname;
        let prefix = format!("{}-{}-{}", date, id, user);

        // create download directory
        let artist = post.community.name.to_lowercase();
        let artist_config = &self.config.artists.get(&artist).unwrap();
        let dir = format!(
            "{}/{}",
            match post.post_type.to_lowercase().as_str() {
                "normal" => artist_config.artist_download_path.clone(),
                "to_fans" => artist_config.moments_download_path.clone(),
                _ => return Err(String::from("Could not parse post_type")),
            }
            .unwrap_or(String::from("posts")),
            prefix
        );
        let temp_dir = format!("{}.temp", dir);

        // don't download if directory exists
        if fs::metadata(dir.as_str()).is_ok() {
            return Ok(String::from(format!("{} exists, skipping", dir.as_str())));
        }

        // recreate temp directory
        match fs::remove_dir_all(&temp_dir) {
            Ok(_) => println!("Removed {}", temp_dir),
            Err(_) => (),
        };
        match fs::remove_file(&temp_dir) {
            Ok(_) => println!("Removed {}", temp_dir),
            Err(_) => (),
        };
        let _ = fs::create_dir_all(temp_dir.as_str()).map_err(|e| {
            format!(
                "Error could not create directory {}: {}",
                temp_dir.as_str(),
                e
            )
        });

        // download photos
        if let Some(photos) = &post.photos {
            for (i, photo) in photos.iter().enumerate() {
                let ext = match photo.url.rfind('.') {
                    Some(ext_idx) => &photo.url[ext_idx..],
                    None => "",
                };
                let save_path = format!("{}/{}-img{:02}{}", temp_dir, prefix, i, ext);
                self.download_direct(&photo.url, &save_path).await?;
            }
        }

        // download videos

        // write contents

        // rename temp directory
        let _ = fs::rename(&temp_dir, &dir)
            .map_err(|e| format!("Error renaming {}: {}", temp_dir, e))?;

        Ok(String::from("OK"))
    }

    async fn download_direct(&self, url: &str, save_path: &str) -> Result<(), String> {
        let data = self
            .anon_client
            .get(url)
            .send()
            .await
            .map_err(|e| format!("Error sending request to {}: {}", url, e))?
            .bytes()
            .await
            .map_err(|e| format!("Error parsing into bytes for {}: {}", url, e))?;
        let mut buffer = File::create(save_path)
            .map_err(|e| format!("Error creating file {}: {}", save_path, e))?;
        buffer
            .write_all(&data)
            .map_err(|e| format!("Error writing to file {}: {}", save_path, e))
    }
}
