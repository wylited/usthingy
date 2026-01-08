mod types;
mod utils;
mod cache;
mod autocomplete;
mod commands;
mod handler;

use poise::serenity_prelude as serenity;
use dotenv::dotenv;
use std::env;
use std::sync::Arc;
use tokio::sync::RwLock;
use octocrab::Octocrab;
use reqwest::Client as HttpClient;
use crate::types::{BotState, UserMapping};
use crate::cache::refresh_cache;
use crate::commands::{repo, proj, user, refresh};
use crate::handler::event_handler;

#[tokio::main]
async fn main() {
    dotenv().ok();
    
    let discord_token = env::var("DISCORD_TOKEN").expect("missing DISCORD_TOKEN");
    let github_token = env::var("GITHUB_TOKEN").expect("missing GITHUB_TOKEN");
    let github_org = env::var("GITHUB_ORG").expect("missing GITHUB_ORG");
    let github_client_id = env::var("GITHUB_CLIENT_ID").expect("missing GITHUB_CLIENT_ID (OAuth App)");

    let octocrab = Octocrab::builder()
        .personal_token(github_token)
        .build()
        .expect("Failed to create Octocrab instance");

    let http_client = HttpClient::new();

    // Initialize state
    let state = Arc::new(BotState {
        octocrab,
        http_client,
        github_org: github_org.clone(),
        github_client_id,
        repos: RwLock::new(Vec::new()),
        users: RwLock::new(Vec::new()),
        projects: RwLock::new(Vec::new()),
        user_mapping: RwLock::new(UserMapping::load()),
    });

    // Initial cache population (don't block main too long, spawn it)
    let state_clone = state.clone();
    tokio::spawn(async move {
        if let Err(e) = refresh_cache(&state_clone).await {
            eprintln!("Failed initial cache refresh: {}", e);
        }
    });

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![repo(), proj(), user(), refresh()],
            event_handler: |ctx, event, framework, data| {
                Box::pin(event_handler(ctx, event, framework, data))
            },
            ..Default::default()
        })
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                println!("Bot registered globally!");
                Ok(state)
            })
        })
        .build();

    let client = serenity::ClientBuilder::new(discord_token, serenity::GatewayIntents::non_privileged())
        .framework(framework)
        .await;

    client.unwrap().start().await.unwrap();
}
