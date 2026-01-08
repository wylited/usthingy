use poise::serenity_prelude as serenity;
use crate::types::{Context, Error};

// --- Helper: Check Permissions ---
// Returns GitHub username if authenticated, Error if not
pub async fn check_auth(ctx: Context<'_>) -> Result<String, Error> {
    let discord_id = ctx.author().id.get();
    let state = ctx.data();
    let mapping = state.user_mapping.read().await;
    
    if let Some(gh_user) = mapping.map.get(&discord_id) {
        Ok(gh_user.clone())
    } else {
        ctx.say("⛔ **Permission Denied**: You must connect your GitHub account to perform this action.\nUse `/user connect <github_username>` first.").await?;
        Err("User not authenticated".into())
    }
}

// --- Helper: Build Item Embed ---
pub fn build_item_embed(
    item_node: &serde_json::Value,
    target_num: i64, 
) -> Option<serenity::CreateEmbed> {
    let content = item_node.get("content")?;
    let num = content.get("number").and_then(|n| n.as_i64())?;
    
    if num != target_num { return None; }

    let title = content.get("title").and_then(|t| t.as_str()).unwrap_or("?");
    let body = content.get("body").and_then(|b| b.as_str()).unwrap_or("");
    let repo = content.get("repository").and_then(|r| r.get("name")).and_then(|n| n.as_str()).unwrap_or("?");
    let url = content.get("url").and_then(|u| u.as_str()).unwrap_or("");
    let state = content.get("state").and_then(|s| s.as_str()).unwrap_or("");
    
    let assignees: Vec<String> = content.get("assignees")
        .and_then(|a| a.get("nodes"))
        .and_then(|n| n.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.get("login").and_then(|l| l.as_str()).map(|s| s.to_string())).collect())
        .unwrap_or_default();

    let labels: Vec<String> = content.get("labels")
        .and_then(|l| l.get("nodes"))
        .and_then(|n| n.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.get("name").and_then(|l| l.as_str()).map(|s| s.to_string())).collect())
        .unwrap_or_default();

    let color = match state {
        "OPEN" => 0x57F287, // Green
        "CLOSED" | "MERGED" => 0x95A5A6, // Grey
        _ => 0x5865F2, // Blurple
    };

    let title_icon = match state {
        "OPEN" => "<:issue:1458877117176742065>",
        "CLOSED" => "<:issue_neutral:1458877524015579209>",
        "MERGED" => "<:pr_merged:1458877132414517360>",
        _ => "",
    };

    let mut embed = serenity::CreateEmbed::new()
        .title(format!("{} {} #{} {}", title_icon, repo, num, title))
        .url(url)
        .description(if body.len() > 1000 { format!("{}...", &body[..1000]) } else { body.to_string() })
        .field("State", state, true)
        .field("Assignees", if assignees.is_empty() { "None".to_string() } else { assignees.join(", ") }, true)
        .field("Labels", if labels.is_empty() { "None".to_string() } else { labels.join(", ") }, true)
        .color(color);

    // Parse custom fields from fieldValues
    if let Some(field_values) = item_node.get("fieldValues").and_then(|fv| fv.get("nodes")).and_then(|n| n.as_array()) {
        for fv in field_values {
            let field_name = fv.get("field").and_then(|f| f.get("name")).and_then(|n| n.as_str());
            
            if let Some(name) = field_name {
                 // Skip if it's just "Title" or "Repository" as we already show them
                if name == "Title" || name == "Repository" || name == "Assignees" || name == "Labels" || name == "Status" || name == "Linked pull requests" {
                     continue;
                }

                let value_str = if let Some(text) = fv.get("text").and_then(|t| t.as_str()) {
                    text.to_string()
                } else if let Some(name) = fv.get("name").and_then(|n| n.as_str()) {
                    name.to_string() // Single select option name
                } else if let Some(date) = fv.get("date").and_then(|d| d.as_str()) {
                    date.to_string()
                } else if let Some(num) = fv.get("number").and_then(|n| n.as_f64()) {
                    num.to_string()
                } else {
                    continue; // Skip unknown or empty types
                };
                
                if !value_str.is_empty() {
                    embed = embed.field(name, value_str, true);
                }
            }
        }
    }

    Some(embed)
}