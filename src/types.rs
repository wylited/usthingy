use std::collections::HashMap;
use tokio::sync::RwLock;
use octocrab::Octocrab;
use reqwest::Client as HttpClient;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct CachedRepo {
    pub name: String,
    #[allow(dead_code)]
    pub full_name: String,
}

#[derive(Clone, Debug)]
pub struct CachedUser {
    pub login: String,
    #[allow(dead_code)]
    pub avatar_url: String,
}

#[derive(Clone, Debug)]
pub struct CachedItem {
    pub title: String,
    pub number: i64,
    pub repo_name: String,
    pub state: String,
}

#[derive(Clone, Debug)]
pub struct CachedField {
    pub id: String,
    pub name: String,
    pub data_type: String, // TEXT, NUMBER, DATE, SINGLE_SELECT, ITERATION
    pub options: HashMap<String, String>, // Option Name -> Option ID
}

#[derive(Clone, Debug)]
pub struct CachedProject {
    pub id: String,
    pub title: String,
    pub url: String,
    #[allow(dead_code)]
    pub number: i64,
    pub items: Vec<CachedItem>,
    pub fields: Vec<CachedField>,
}

#[derive(serde::Serialize, serde::Deserialize, Default, Clone)]
pub struct UserMapping {
    // Discord ID -> GitHub Username
    pub map: HashMap<u64, String>,
}

impl UserMapping {
    pub fn load() -> Self {
        if let Ok(content) = std::fs::read_to_string("user_mapping.json") {
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            Self::default()
        }
    }

    pub fn save(&self) {
        if let Ok(content) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write("user_mapping.json", content);
        }
    }
}

pub struct BotState {
    pub octocrab: Octocrab,
    pub http_client: HttpClient,
    pub github_org: String,
    pub github_client_id: String,
    // Caches protected by RwLock for concurrent access
    pub repos: RwLock<Vec<CachedRepo>>,
    pub users: RwLock<Vec<CachedUser>>,
    pub projects: RwLock<Vec<CachedProject>>,
    // User mapping (Discord -> GitHub)
    pub user_mapping: RwLock<UserMapping>,
}

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Context<'a> = poise::Context<'a, Arc<BotState>, Error>;
