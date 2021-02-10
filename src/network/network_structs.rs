use crate::config::Config;
use serde::Deserialize;
use std::collections::HashMap;

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
pub enum DownloadResult {
    Downloaded(Post),
    Skipped(Post),
    RequiresPassword(Post),
}
