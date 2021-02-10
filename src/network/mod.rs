// #[macro_use]
// extern crate lazy_static;

use chrono::DateTime;
use futures::stream::{self, StreamExt};
use futures::task::Poll;
use reqwest::header;
use serde::Deserialize;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::fs::{self, File};
use std::io::prelude::*;
use std::sync::{Arc, Mutex};

use crate::config::Config;
use crate::network::network_structs::*;
use crate::network::urls::*;

mod network_structs;
mod urls;

fn get_prefix(post: &Post) -> String {
    let date = DateTime::parse_from_rfc3339(&post.created_at)
        .unwrap()
        .format("%Y%m%d");
    let id = post.id.to_string();
    let user = &post.community_user.nickname;
    format!("{}-{}-{}", date, id, user)
}

fn get_url(post: &Post) -> String {
    POST_URL
        .replace("{artist}", post.community.name.to_string().to_lowercase().as_str())
        .replace("{post_id}", post.id.to_string().as_str())
}

pub async fn download(conf: &Config, token: &str) -> Result<(), String> {
    let n = Network::new(&conf, &token).await?;

    // get a list of all posts to download
    println!("Getting all post info...");
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
        .flatten();

    let (tx, mut rx) = futures::channel::mpsc::unbounded::<Post>();

    // download the posts
    println!("Downloading all posts...");
    {
        let mut downloads = stream::poll_fn(|_| {
            let post = match rx.try_next() {
                Ok(o) => o,
                Err(_) => return Poll::Pending,
            }
            .unwrap();
            Poll::Ready(Some(n.download_post(post.clone())))
        })
        .buffer_unordered(conf.max_connections);

        // add all posts to downloads buffer
        let mut remaining: usize = 0;
        posts.for_each(|p| {
            tx.unbounded_send(p).unwrap();
            remaining += 1;
        });

        // spawn a new thread to manage printing to stdout
        let (print_tx, print_rx) = std::sync::mpsc::channel::<String>();
        let mtx = Arc::new(Mutex::new(0usize));
        {
            let mtx = mtx.clone();
            tokio::spawn(async move {
                print_worker(print_rx, mtx).await;
            });
        }

        while remaining > 0 {
            futures::future::poll_fn(|ctx| match downloads.poll_next_unpin(ctx) {
                Poll::Ready(v) => {
                    if v.is_none() {
                        return Poll::Ready(None);
                    }

                    let result = v.unwrap();
                    match &result {
                        Ok(DownloadResult::Downloaded(p)) => {
                            print_tx.send(format!("Downloaded {}", get_url(&p))).unwrap();
                            remaining -= 1;
                        }
                        Ok(DownloadResult::Skipped(_)) => {
                            remaining -= 1;
                        }
                        Ok(DownloadResult::RequiresPassword(p)) => {
                            let n = n.clone();
                            let p = p.clone();
                            let tx = tx.clone();
                            let waker = ctx.waker().clone();
                            let mtx = mtx.clone();
                            tokio::spawn(async move {
                                let mut post = n.download_post_info(&p, Some(&mtx)).await;
                                while post.is_err() {
                                    post = n.download_post_info(&p, Some(&mtx)).await;
                                }
                                if post.is_ok() {
                                    tx.unbounded_send(post.unwrap()).unwrap();
                                    waker.wake();
                                }
                            });
                            return Poll::Pending;
                        }
                        Err((s, _)) => eprintln!("{}", s),
                    }
                    return Poll::Ready(Some(result));
                }
                Poll::Pending => Poll::Pending,
            })
            .await;
        }
    }

    Ok(())
}

async fn print_worker(rx: std::sync::mpsc::Receiver<String>, mtx: Arc<Mutex<usize>>) {
    loop {
        match rx.recv() {
            Ok(s) => {
                let guard = mtx.lock().unwrap();
                println!("{}", s);
                std::mem::drop(guard);
            }
            Err(_) => return,
        }
    }
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

    let url = API_INFO_URL;
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

impl<'a> Network {
    async fn new(config: &'a Config, token: &str) -> Result<Network, String> {
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
        println!("Getting artist ids...");
        let artist_id_map = get_artist_id(&anon_client).await?;

        let n = Network {
            config: config.clone(),
            client,
            anon_client,
            artist_id_map,
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
            PostType::Artist => API_ARTIST_TAB,
            PostType::Moment => API_TO_FANS,
        }
        .replace("{artist_id}", &artist_id.to_string()[..]);

        // return value
        let mut ret: Vec<Post> = Vec::new();

        loop {
            // build request
            let page_size: String = match recent {
                Some(v) => {
                    if v <= &0 {
                        break;
                    }
                    std::cmp::max(1, v - count).to_string()
                }
                None => String::from("100"),
            };
            let params = [("pageSize", &page_size), ("from", &from)];

            // send request
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
                    count += num_posts;
                    if v - count <= 0 {
                        break;
                    }
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

    async fn download_post(&self, mut post: Post) -> Result<DownloadResult, (String, Post)> {
        let prefix = get_prefix(&post);

        // create download directory
        let artist = post.community.name.to_lowercase();
        let artist_config = &self.config.artists.get(&artist).unwrap();
        let dir = format!(
            "{}/{}",
            match post.post_type.to_lowercase().as_str() {
                "normal" => artist_config.artist_download_path.clone(),
                "to_fans" => artist_config.moments_download_path.clone(),
                _ => return Err((String::from("Could not parse post_type"), post)),
            }
            .unwrap_or_else(|| String::from("posts")),
            prefix
        );
        let temp_dir = format!("{}.temp", dir);

        // don't download if directory exists
        if fs::metadata(dir.as_str()).is_ok() {
            return Ok(DownloadResult::Skipped(post));
        }

        if post.locked {
            return Ok(DownloadResult::RequiresPassword(post));
        }

        if post.attached_videos.is_some() {
            post = self
                .download_post_info(&post, None)
                .await
                .map_err(|(s, _)| (s, post.clone()))?;
        }

        // recreate temp directory
        let _ = fs::remove_dir_all(&temp_dir);
        let _ = fs::remove_file(&temp_dir);
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
                self.download_direct(&photo.url, &save_path)
                    .await
                    .map_err(|e| (e, post.clone()))?;
            }
        }

        // download videos
        if let Some(videos) = &post.attached_videos {
            for (i, video) in videos.iter().enumerate() {
                if let Some(video_url) = &video.video_url {
                    let ext = match video_url.rfind('.') {
                        Some(ext_idx) => &video_url[ext_idx..],
                        None => "",
                    };
                    let save_path = format!("{}/{}-vid{:02}{}", temp_dir, prefix, i, ext);
                    self.download_direct(&video_url, &save_path)
                        .await
                        .map_err(|e| (e, post.clone()))?;
                }
            }
        }

        // write contents
        {
            let save_path = format!("{}/{}-content.txt", temp_dir, prefix);
            let body = match &post.body {
                Some(v) => v.as_str(),
                None => "",
            };
            let content = format!(
                "https://weverse.io/{}/artist/{}\n{} ({}):\n{}",
                post.community.name.to_lowercase(),
                post.id.to_string(),
                &post.community_user.nickname,
                &post.created_at,
                body
            );
            let mut buffer = File::create(save_path.as_str()).map_err(|e| {
                (
                    format!("Error creating file {}: {}", save_path.as_str(), e),
                    post.clone(),
                )
            })?;
            buffer.write_all(&content.as_bytes()).map_err(|e| {
                (
                    format!("Error writing to file {}: {}", save_path.as_str(), e),
                    post.clone(),
                )
            })?;
        }

        // rename temp directory
        let _ = fs::rename(&temp_dir, &dir)
            .map_err(|e| (format!("Error renaming {}: {}", temp_dir, e), post.clone()))?;

        Ok(DownloadResult::Downloaded(post))
    }

    async fn download_post_info(
        &self,
        post: &'a Post,
        mtx: Option<&Arc<Mutex<usize>>>,
    ) -> Result<Post, (String, &'a Post)> {
        let url = API_POST_URL
            .replace("{artist_id}", post.community.id.to_string().as_str())
            .replace("{post_id}", post.id.to_string().as_str());

        let resp = if post.locked {
            let password = {
                // lock mutex to prevent printing other messages when we query
                // for the password
                let mtx = mtx.unwrap();
                let guard = mtx.lock().unwrap();

                // query user for password
                println!("Password required for {}:", get_url(&post));
                let mut buf = String::new();
                std::io::stdin()
                    .read_line(&mut buf)
                    .expect("Failed to read input.");
                let ret = buf
                    .lines()
                    .next()
                    .expect("Could not read entry.")
                    .to_string();

                // drop the mutex lock and return
                std::mem::drop(guard);
                ret
            };

            let json: HashMap<&str, &str> = [("lockPassword", password.as_str())]
                .iter()
                .cloned()
                .collect();

            self.client
                .post(&url)
                .json(&json)
                .send()
                .await
                .map_err(|e| (format!("{}", e), post))?
        } else {
            self.client
                .get(&url)
                .send()
                .await
                .map_err(|e| (format!("{}", e), post))?
        };

        // parse response
        if !resp.status().is_success() {
            return Err((format!("{}", resp.status()), post));
        }

        let text = &resp.text().await.map_err(|e| {
            (
                format!("Error reading response text for {}: {}", &url, e),
                post,
            )
        })?;
        let post_resp: Post = serde_json::from_str(text)
            .map_err(|e| (format!("Error parsing json for {}: {}", &url, e), post))?;

        Ok(post_resp)
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
