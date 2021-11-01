use crate::config::Config;
use serde::Deserialize;
use std::collections::HashMap;
use std::error::Error;
use std::fmt;

#[derive(Clone)]
pub struct Network {
    pub config: Config,
    pub client: reqwest::Client,
    pub anon_client: reqwest::Client,
    pub artist_id_map: HashMap<String, i64>,
}

#[derive(Debug, Deserialize)]
pub struct Posts {
    pub posts: Vec<Post>,
    #[serde(rename = "isEnded")]
    pub is_ended: bool,
    #[serde(rename = "lastId")]
    pub last_id: Option<i64>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Post {
    pub id: i64,
    #[serde(rename = "communityUser")]
    pub community_user: CommunityUser,
    pub community: Community,
    #[serde(rename = "communityTabId")]
    pub community_tab_id: i64,
    #[serde(rename = "type")]
    pub post_type: String,
    pub body: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    pub photos: Option<Vec<Photo>>,
    #[serde(rename = "attachedVideos")]
    pub attached_videos: Option<Vec<Video>>,
    #[serde(rename = "isLocked")]
    pub locked: bool,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CommunityUser {
    pub id: i64,
    #[serde(rename = "profileNickname")]
    pub nickname: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Community {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Photo {
    pub id: i64,
    #[serde(rename = "orgImgUrl")]
    pub url: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Video {
    #[serde(rename = "videoUrl")]
    pub video_url: Option<String>,
}

pub enum PostType {
    Artist,
    Moment,
}

#[derive(Debug)]
pub enum DownloadOk {
    Downloaded(Post),
    Skipped(Post),
}

#[derive(Debug)]
pub enum DownloadErr {
    ArtistMapErr(String),
    LastIdErr,
    ParsePostTypeErr(String),
    RequestErr(String, reqwest::Error),
    ResponseErr(String, Post, reqwest::StatusCode),
    ResponseBytesErr(String, reqwest::Error),
    ResponseJsonErr(String, serde_json::Error),
    ResponseTextErr(String, reqwest::Error),
    StdinErr,
    StdinErrStr(std::io::Error),
    FileCreateErr(String, std::io::Error),
    FileWriteErr(String, std::io::Error),
    RenameErr(String, std::io::Error),
}

impl Error for DownloadErr {}

impl fmt::Display for DownloadErr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            DownloadErr::ArtistMapErr(s) => {
                format!("Error finding {} in artist_id_map", s).fmt(f)
            },
            DownloadErr::RequestErr(s, e) => {
                format!("Error sending request for {}: {}", s, e).fmt(f)
            },
            DownloadErr::ResponseTextErr(s, e) => {
                format!("Error reading response text for {}: {}", s, e).fmt(f)
            },
            DownloadErr::ResponseJsonErr(s, e) => {
                format!("Error parsing json for {}: {}", s, e).fmt(f)
            },
            DownloadErr::LastIdErr => write!(f, "Error lastId not found"),
            DownloadErr::ParsePostTypeErr(s) => {
                format!("Error parsing post type: {}", s).fmt(f)
            },
            DownloadErr::StdinErr => write!(f, "Error reading stdin"),
            DownloadErr::StdinErrStr(s) => {
                format!("Error reading stdin: {}", s).fmt(f)
            },
            DownloadErr::ResponseErr(s, _, c) => {
                format!("Error response for {}: {}", s, c).fmt(f)
            },
            DownloadErr::ResponseBytesErr(s, e) => {
                format!("Error parsing bytes for {}: {}", s, e).fmt(f)
            },
            DownloadErr::FileCreateErr(s, e) => {
                format!("Error creating file {}: {}", s, e).fmt(f)
            },
            DownloadErr::FileWriteErr(s, e) => {
                format!("Error writing to file {}: {}", s, e).fmt(f)
            },
            DownloadErr::RenameErr(s, e) => {
                format!("Error renaming {}: {}", s, e).fmt(f)
            },
        }
    }
}
