use poise::serenity_prelude as serenity;
use crate::types::{Context, Error};
use crate::utils::{check_auth, build_item_embed};
use crate::autocomplete::*;
use std::time::Duration;
use serenity::all::{CreateSelectMenu, CreateSelectMenuKind, CreateSelectMenuOption};
use crate::cache::refresh_cache;

// --- Commands ---

/// Manage Repositories (Issues, PRs)
#[poise::command(slash_command, subcommands("assign", "target", "list_repos", "list_issues"))]
pub async fn repo(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

/// Assign an issue to a user
#[poise::command(slash_command)]
pub async fn assign(
    ctx: Context<'_>,
    #[description = "Repository name"] 
    #[autocomplete = "repo_autocomplete"]
    repo: String,
    #[description = "Issue number"] number: u64,
    #[description = "GitHub Username"] 
    #[autocomplete = "user_autocomplete"]
    user: String,
) -> Result<(), Error> {
    let _ = check_auth(ctx).await?; // Enforce auth
    let state = ctx.data();
    let org = &state.github_org;
    ctx.defer().await?;

    match state.octocrab.issues(org, &repo).add_assignees(number, &[&user]).await {
        Ok(issue) => {
             let embed = serenity::CreateEmbed::new()
                .title(format!("assigned issue #{}", number))
                .url(issue.html_url.to_string())
                .description(format!("successfully assigned **{}** to issue **#{}** in **{}**", user, number, repo))
                .color(0x57F287) // Green
                .timestamp(serenity::Timestamp::now());
            
             ctx.send(poise::CreateReply::default().embed(embed)).await?;
        }
        Err(e) => {
             let embed = serenity::CreateEmbed::new()
                .title("assignment failed")
                .description(format!("error: {}", e))
                .color(0xED4245); // Red
             ctx.send(poise::CreateReply::default().embed(embed)).await?;
        }
    }
    Ok(())
}

/// Target an issue with arguments
#[poise::command(slash_command)]
pub async fn target(
    ctx: Context<'_>,
    #[description = "Repository name"] 
    #[autocomplete = "repo_autocomplete"]
    repo: String,
    #[description = "Issue number"] number: u64,
    #[description = "Arguments (e.g. +1w, v2.0)"] args: String,
) -> Result<(), Error> {
    let _ = check_auth(ctx).await?; // Enforce auth
    let state = ctx.data();
    let org = &state.github_org;

    // Fetch issue details first for context
    let issue_res = state.octocrab.issues(org, &repo).get(number).await;
    
    let title = match issue_res {
        Ok(i) => i.title,
        Err(_) => "Unknown Issue".to_string(),
    };

    let embed = serenity::CreateEmbed::new()
        .title(format!("Target Issue #{}?", number))
        .description(format!("**Repo:** {}\n**Issue:** {}\n**Target Args:** `{}`", repo, title, args))
        .color(0xFEE75C); // Yellow

    let components = vec![serenity::CreateActionRow::Buttons(vec![
        serenity::CreateButton::new("confirm_target")
            .label("Confirm")
            .style(serenity::ButtonStyle::Success),
        serenity::CreateButton::new("cancel_target")
            .label("Cancel")
            .style(serenity::ButtonStyle::Secondary),
    ])];

    ctx.send(poise::CreateReply::default().embed(embed).components(components)).await?;
    Ok(())
}

/// List all repositories in the Organization
#[poise::command(slash_command, rename = "list")]
pub async fn list_repos(ctx: Context<'_>) -> Result<(), Error> {
    let state = ctx.data();
    let repos = state.repos.read().await;
    
    if repos.is_empty() {
        ctx.say("No repositories found in cache. Try refreshing?").await?;
        return Ok(());
    }
    
    // Chunk repos for embed fields if many
    let repo_names: Vec<String> = repos.iter().map(|r| format!("• {}", r.name)).collect();
    let description = repo_names.join("\n");

    // Truncate if too long for one embed (Discord limit 4096 chars)
    let description = if description.len() > 4000 {
        format!("{}\n...and more", &description[..4000])
    } else {
        description
    };

    let embed = serenity::CreateEmbed::new()
        .title(format!("Repositories in {}", state.github_org))
        .description(description)
        .color(0x5865F2); // Blurple

    ctx.send(poise::CreateReply::default().embed(embed)).await?;
    Ok(())
}

/// List open issues in a repository
#[poise::command(slash_command, rename = "issues")]
pub async fn list_issues(
    ctx: Context<'_>,
    #[description = "Repository name"] 
    #[autocomplete = "repo_autocomplete"]
    repo: String,
) -> Result<(), Error> {
    let state = ctx.data();
    let org = &state.github_org;
    ctx.defer().await?;

    match state.octocrab.issues(org, &repo).list().state(octocrab::params::State::Open).per_page(10).send().await {
        Ok(page) => {
             if page.items.is_empty() {
                 ctx.say(format!("No open issues in {}/{}", org, repo)).await?;
                 return Ok(());
             }

             let mut embed = serenity::CreateEmbed::new()
                .title(format!("Open Issues in {}/{}", org, repo))
                .color(0x5865F2);

             for issue in page.items {
                 embed = embed.field(
                     format!("#{} {}", issue.number, issue.title), 
                     format!("By: {} | [Link]({})", issue.user.login, issue.html_url), 
                     false
                 );
             }
             
             // Add a refresh button example
             let components = vec![serenity::CreateActionRow::Buttons(vec![
                serenity::CreateButton::new(format!("refresh_issues_{}", repo))
                    .label("Refresh")
                    .style(serenity::ButtonStyle::Secondary)
                    .emoji('🔄')
             ])];

             ctx.send(poise::CreateReply::default().embed(embed).components(components)).await?;
        }
        Err(e) => {
            ctx.say(format!("❌ Failed to fetch issues: {}", e)).await?;
        }
    }
    Ok(())
}

/// Manage Organization Projects
#[poise::command(slash_command, subcommands("list_projects", "view_project", "view_item", "edit_project_item"))]
pub async fn proj(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

/// List Projects in the Organization
#[poise::command(slash_command, rename = "list")]
pub async fn list_projects(ctx: Context<'_>) -> Result<(), Error> {
    let state = ctx.data();
    let projects = state.projects.read().await;

    if projects.is_empty() {
        ctx.say("No projects found in cache. Try /refresh?").await?;
        return Ok(());
    }

    let mut embed = serenity::CreateEmbed::new()
        .title(format!("Projects in {}", state.github_org))
        .color(0xEB459E); // Pinkish

    for proj in projects.iter() {
        embed = embed.field(
            &proj.title,
            format!("[View Board]({})", proj.url),
            false
        );
    }

    ctx.send(poise::CreateReply::default().embed(embed)).await?;
    Ok(())
}

/// View items in a specific Project
#[poise::command(slash_command, rename = "view")]
pub async fn view_project(
    ctx: Context<'_>,
    #[description = "Project Title"] 
    #[autocomplete = "project_autocomplete"]
    title: String,
    #[description = "Filter items (active [default], all)"]
    filter: Option<String>,
    #[description = "Page number (default 1)"]
    page: Option<usize>,
) -> Result<(), Error> {
    let state = ctx.data();
    let filter = filter.unwrap_or_else(|| "active".to_string()).to_lowercase();
    let page_num = page.unwrap_or(1);
    let page_size = 20;

    // Scope the read lock
    let project_opt = {
        let projects = state.projects.read().await;
        projects.iter().find(|p| p.title.eq_ignore_ascii_case(&title)).cloned()
    };
    
    match project_opt {
        Some(proj) => {
            ctx.defer().await?;
            
            // GraphQL query to fetch project items (fetch 100 to support local paging/filtering)
            // In a real robust app, we would use cursor-based pagination
            let query = serde_json::json!({
                "query": r#"
                    query($id: ID!) {
                        node(id: $id) {
                            ... on ProjectV2 {
                                items(first: 100) {
                                    nodes {
                                        id
                                        type
                                        content {
                                            ... on Issue {
                                                title
                                                number
                                                url
                                                repository { name }
                                                state
                                                body
                                                assignees(first: 3) { nodes { login } }
                                                labels(first: 3) { nodes { name } }
                                            }
                                            ... on PullRequest {
                                                title
                                                number
                                                url
                                                repository { name }
                                                state
                                                body
                                                assignees(first: 3) { nodes { login } }
                                            }
                                            ... on DraftIssue {
                                                title
                                                body
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                "#,
                "variables": {
                    "id": proj.id
                }
            });

            match state.octocrab.graphql(&query).await {
                Ok(resp) => {
                    let json_resp: serde_json::Value = resp;
                    
                    let mut all_items = Vec::new();
                    
                    if let Some(nodes) = json_resp.get("data")
                        .and_then(|d| d.get("node"))
                        .and_then(|d| d.get("items"))
                        .and_then(|d| d.get("nodes"))
                        .and_then(|d| d.as_array()) 
                    {
                        for item in nodes {
                            let content = item.get("content");
                            
                            if let Some(issue) = content.and_then(|c| c.get("number")) {
                                let state = content.and_then(|c| c.get("state")).and_then(|s| s.as_str()).unwrap_or("");
                                
                                // Filtering
                                let is_closed = state == "CLOSED" || state == "MERGED";
                                if filter == "active" && is_closed { continue; }

                                let title = content.and_then(|c| c.get("title")).and_then(|t| t.as_str()).unwrap_or("?");
                                let repo = content.and_then(|c| c.get("repository")).and_then(|r| r.get("name")).and_then(|n| n.as_str()).unwrap_or("?");
                                let number = issue.as_i64().unwrap_or(0);
                                let url = content.and_then(|c| c.get("url")).and_then(|u| u.as_str()).unwrap_or("");
                                
                                let icon = match state {
                                    "OPEN" => "🟢",
                                    "CLOSED" => "🟣",
                                    "MERGED" => "🟣",
                                    _ => "⚪",
                                };
                                
                                all_items.push(format!("{} **{}/[#{}]({})** {}", icon, repo, number, url, title));
                            } else if let Some(draft_title) = content.and_then(|c| c.get("title")).and_then(|t| t.as_str()) {
                                all_items.push(format!("📝 **Draft:** {}", draft_title));
                            }
                        }
                    }

                    let mut display_lines = Vec::new();
                    let mut menu_options = Vec::new();

                    if let Some(nodes) = json_resp.get("data")
                        .and_then(|d| d.get("node"))
                        .and_then(|d| d.get("items"))
                        .and_then(|d| d.get("nodes"))
                        .and_then(|d| d.as_array()) 
                    {
                        for item in nodes {
                            let content = item.get("content");
                            if let Some(issue) = content.and_then(|c| c.get("number")) {
                                let state = content.and_then(|c| c.get("state")).and_then(|s| s.as_str()).unwrap_or("");
                                
                                // Filtering
                                let is_closed = state == "CLOSED" || state == "MERGED";
                                if filter == "active" && is_closed { continue; }

                                let title = content.and_then(|c| c.get("title")).and_then(|t| t.as_str()).unwrap_or("?");
                                let repo = content.and_then(|c| c.get("repository")).and_then(|r| r.get("name")).and_then(|n| n.as_str()).unwrap_or("?");
                                let number = issue.as_i64().unwrap_or(0);
                                let url = content.and_then(|c| c.get("url")).and_then(|u| u.as_str()).unwrap_or("");
                                
                                let icon = match state {
                                    "OPEN" => "<:issue:1458877117176742065>",
                                    "CLOSED" => "<:issue_neutral:1458877524015579209>",
                                    "MERGED" => "<:pr_merged:1458877132414517360>",
                                    _ => "⚪",
                                };
                                
                                display_lines.push(format!("{} **{}/[#{}]({})** {}", icon, repo, number, url, title));
                                menu_options.push((number, title.to_string(), repo.to_string()));
                            } else if let Some(draft_title) = content.and_then(|c| c.get("title")).and_then(|t| t.as_str()) {
                                display_lines.push(format!("<:project_item_draft:1458877119789797580> **Draft:** {}", draft_title));
                                // Drafts skipped in menu for now as they have no number
                            }
                        }
                    }

                    // Recalculate pagination based on filtered list
                    let total_items = display_lines.len();
                    // Re-calculate start_idx in case filter changed things
                    let start_idx = (page_num - 1) * page_size;
                    
                    if start_idx >= total_items && total_items > 0 {
                         ctx.say(format!("Page {} is out of bounds.", page_num)).await?;
                         return Ok(());
                    }
                    if display_lines.is_empty() {
                         ctx.say(format!("No items found in project {} with filter '{}'.", proj.title, filter)).await?;
                         return Ok(());
                    }

                    let end_idx = std::cmp::min(start_idx + page_size, total_items);
                    let page_display = &display_lines[start_idx..end_idx];
                    let page_menu_opts = menu_options.iter().skip(start_idx).take(page_size);

                    let embed = serenity::CreateEmbed::new()
                        .title(format!("Project: {} ({})", proj.title, filter))
                        .url(&proj.url)
                        .description(page_display.join("\n"))
                        .footer(serenity::CreateEmbedFooter::new(format!("Page {}/{} • Total: {}", page_num, (total_items + page_size - 1) / page_size, total_items)))
                        .color(0xEB459E);

                    let mut components = Vec::new();
                    
                    // 1. Select Menu
                    let mut select_opts_vec = Vec::new();
                    for (num, title, repo) in page_menu_opts {
                        // Label max 100 chars
                        let label = format!("{} #{}: {}", repo, num, title);
                        let label = if label.len() > 95 { format!("{}...", &label[..95]) } else { label };
                        select_opts_vec.push(CreateSelectMenuOption::new(label, num.to_string()));
                    }
                    
                    if !select_opts_vec.is_empty() {
                        let menu_id = format!("proj_select_{}", proj.id); // Use Project ID to context
                        let menu = CreateSelectMenu::new(menu_id, CreateSelectMenuKind::String { options: select_opts_vec })
                            .placeholder("🔍 Select an item to view details...");
                        components.push(serenity::CreateActionRow::SelectMenu(menu));
                    }

                    // 2. Buttons
                    let mut buttons = Vec::new();
                    if page_num > 1 {
                        buttons.push(serenity::CreateButton::new(format!("proj_page_{}_{}", title, page_num - 1)).label("◀️ Prev").style(serenity::ButtonStyle::Secondary));
                    }
                    buttons.push(serenity::CreateButton::new_link(&proj.url).label("Open Board"));
                    if end_idx < total_items {
                         buttons.push(serenity::CreateButton::new(format!("proj_page_{}_{}", title, page_num + 1)).label("Next ▶️").style(serenity::ButtonStyle::Secondary));
                    }
                    components.push(serenity::CreateActionRow::Buttons(buttons));

                    ctx.send(poise::CreateReply::default().embed(embed).components(components)).await?;
                }
                Err(e) => {
                    ctx.say(format!("❌ Failed to fetch project items: {}", e)).await?;
                }
            }
        }
        None => {
            ctx.say(format!("Project '{}' not found in cache. Try /refresh?", title)).await?;
        }
    }
    Ok(())
}

/// View details of a specific item in a project
#[poise::command(slash_command, rename = "view-item")]
pub async fn view_item(
    ctx: Context<'_>,
    #[description = "Project Title"] 
    #[autocomplete = "project_autocomplete"]
    title: String,
    #[description = "Item ID (e.g. '123' or 'backend-service #123')"]
    #[autocomplete = "item_autocomplete"]
    item_query: String,
) -> Result<(), Error> {
    // Re-using the logic from view_project but finding specific item
    // In production, you'd want a more direct lookup
    let state = ctx.data();
    
     // Scope the read lock
    let project_opt = {
        let projects = state.projects.read().await;
        projects.iter().find(|p| p.title.eq_ignore_ascii_case(&title)).cloned()
    };
    
    match project_opt {
        Some(proj) => {
            ctx.defer().await?;
             // Fetch items (same query as view)
             // ... (Optimized query to just get specific item would be better but V2 ID mapping is complex)
             // We'll fetch items and filter in memory for this MVP
             let query = serde_json::json!({
                "query": r#"
                    query($id: ID!) {
                        node(id: $id) {
                            ... on ProjectV2 {
                                items(first: 100) {
                                    nodes {
                                        content {
                                            ... on Issue {
                                                title
                                                number
                                                body
                                                url
                                                repository { name }
                                                state
                                                assignees(first: 3) { nodes { login } }
                                                labels(first: 5) { nodes { name } }
                                            }
                                            ... on PullRequest {
                                                title
                                                number
                                                body
                                                url
                                                repository { name }
                                                state
                                                assignees(first: 3) { nodes { login } }
                                            }
                                        }
                                        fieldValues(first: 20) {
                                            nodes {
                                                ... on ProjectV2ItemFieldTextValue { text field { ... on ProjectV2FieldCommon { name } } }
                                                ... on ProjectV2ItemFieldDateValue { date field { ... on ProjectV2FieldCommon { name } } }
                                                ... on ProjectV2ItemFieldSingleSelectValue { name field { ... on ProjectV2FieldCommon { name } } }
                                                ... on ProjectV2ItemFieldNumberValue { number field { ... on ProjectV2FieldCommon { name } } }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                "#,
                "variables": { "id": proj.id }
            });
            
            match state.octocrab.graphql(&query).await {
                Ok(resp) => {
                    let json_resp: serde_json::Value = resp;
                    
                    // Improved parsing for "Repo #123" or "123"
                    let target_num = if let Some(idx) = item_query.find('#') {
                        item_query[idx+1..].chars().take_while(|c| c.is_ascii_digit()).collect::<String>()
                    } else {
                        item_query.chars().take_while(|c| c.is_ascii_digit()).collect::<String>()
                    };
                    let target_num: i64 = target_num.parse().unwrap_or(0);
                    
                    if let Some(nodes) = json_resp.get("data")
                        .and_then(|d| d.get("node"))
                        .and_then(|d| d.get("items"))
                        .and_then(|d| d.get("nodes"))
                        .and_then(|d| d.as_array()) 
                    {
                        for item in nodes {
                            if let Some(embed) = build_item_embed(item, target_num) {
                                let components = vec![serenity::CreateActionRow::Buttons(vec![
                                    serenity::CreateButton::new(format!("edit:item:{}:{}", proj.id, target_num))
                                        .label("✏️ Edit Item")
                                        .style(serenity::ButtonStyle::Secondary)
                                ])];
                                ctx.send(poise::CreateReply::default().embed(embed).components(components)).await?;
                                return Ok(());
                            }
                        }
                        ctx.say(format!("❌ Item #{} not found in project items (checked top 100).", target_num)).await?;
                    }
                },
                Err(e) => { ctx.say(format!("Error fetching items: {}", e)).await?; }
            }
        },
        None => { ctx.say("Project not found.").await?; }
    }

    Ok(())
}
/// Edit a project item field
#[poise::command(slash_command, rename = "edit")]
pub async fn edit_project_item(
    ctx: Context<'_>,
    #[description = "Project Title"] 
    #[autocomplete = "project_autocomplete"]
    title: String,
    #[description = "Item ID (e.g. '123' or 'Repo #123')"]
    #[autocomplete = "item_autocomplete"]
    item_query: String,
    #[description = "Field Name (e.g. 'Status', 'Priority', 'Date')"]
    #[autocomplete = "field_autocomplete"]
    field: String,
    #[description = "New Value (Select Option or Text)"]
    #[autocomplete = "value_autocomplete"]
    value: String,
) -> Result<(), Error> {
    let _ = check_auth(ctx).await?;
    let state = ctx.data();
    
    // 1. Identify Project & Field from Cache
    let (proj, target_field, option_id) = {
        let projects = state.projects.read().await;
        if let Some(p) = projects.iter().find(|p| p.title.eq_ignore_ascii_case(&title)) {
            // Clone the project to extend lifetime
            let p_clone = p.clone();
            
            if let Some(f) = p.fields.iter().find(|f| f.name.eq_ignore_ascii_case(&field)) {
                // Check if value is an Option ID mapping
                let opt_id = f.options.iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case(&value))
                    .map(|(_, v)| v.clone());
                (p_clone, f.clone(), opt_id)
            } else {
                ctx.say(format!("Field '{}' not found in project '{}'.", field, title)).await?;
                return Ok(());
            }
        } else {
             ctx.say(format!("Project '{}' not found.", title)).await?;
             return Ok(());
        }
    };
    
    ctx.defer().await?;

    // 2. Parse Item Number
    let target_num = if let Some(idx) = item_query.find('#') {
        item_query[idx+1..].chars().take_while(|c| c.is_ascii_digit()).collect::<String>()
    } else {
        item_query.chars().take_while(|c| c.is_ascii_digit()).collect::<String>()
    };
    let target_num: i64 = target_num.parse().unwrap_or(0);
    
    // 3. Fetch Item Node ID and Current Value
    let query = serde_json::json!({
        "query": r#"
            query($id: ID!) {
                node(id: $id) {
                    ... on ProjectV2 {
                        items(first: 100) {
                            nodes {
                                id
                                content {
                                    ... on Issue { number title }
                                    ... on PullRequest { number title }
                                }
                                fieldValues(first: 20) {
                                    nodes {
                                        ... on ProjectV2ItemFieldTextValue { text field { ... on ProjectV2FieldCommon { name } } }
                                        ... on ProjectV2ItemFieldDateValue { date field { ... on ProjectV2FieldCommon { name } } }
                                        ... on ProjectV2ItemFieldSingleSelectValue { name field { ... on ProjectV2FieldCommon { name } } }
                                        ... on ProjectV2ItemFieldNumberValue { number field { ... on ProjectV2FieldCommon { name } } }
                                        ... on ProjectV2ItemFieldIterationValue { title field { ... on ProjectV2FieldCommon { name } } }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        "#,
        "variables": { "id": proj.id }
    });

    let resp = state.octocrab.graphql(&query).await?;
    let json_resp: serde_json::Value = resp;
    
    let mut item_node_id = String::new();
    let mut item_title = String::new();
    let mut current_val = "Empty".to_string();

    if let Some(items) = json_resp.get("data").and_then(|d| d.get("node")).and_then(|n| n.get("items")).and_then(|i| i.get("nodes")).and_then(|n| n.as_array()) {
        for item in items {
            let content = item.get("content");
            if let Some(num) = content.and_then(|c| c.get("number")).and_then(|n| n.as_i64()) {
                if num == target_num {
                    item_node_id = item.get("id").and_then(|s| s.as_str()).unwrap_or("").to_string();
                    item_title = content.and_then(|c| c.get("title")).and_then(|s| s.as_str()).unwrap_or("").to_string();
                    
                    // Find current value for this field
                     if let Some(fvs) = item.get("fieldValues").and_then(|f| f.get("nodes")).and_then(|n| n.as_array()) {
                        for fv in fvs {
                            let fname = fv.get("field").and_then(|f| f.get("name")).and_then(|n| n.as_str()).unwrap_or("");
                            if fname.eq_ignore_ascii_case(&field) {
                                // Extract value
                                if let Some(t) = fv.get("text").and_then(|s| s.as_str()) { current_val = t.to_string(); }
                                else if let Some(n) = fv.get("name").and_then(|s| s.as_str()) { current_val = n.to_string(); }
                                else if let Some(d) = fv.get("date").and_then(|s| s.as_str()) { current_val = d.to_string(); }
                                else if let Some(n) = fv.get("number").and_then(|f| f.as_f64()) { current_val = n.to_string(); }
                                else if let Some(t) = fv.get("title").and_then(|s| s.as_str()) { current_val = t.to_string(); }
                            }
                        }
                    }
                    break;
                }
            }
        }
    }

    if item_node_id.is_empty() {
        ctx.say(format!("❌ Item #{} not found in project.", target_num)).await?;
        return Ok(());
    }

    // 4. Confirmation
    let embed = serenity::CreateEmbed::new()
        .title("Confirm Edit")
        .description(format!(
            "**Project:** {}\n**Item:** #{} {}\n**Field:** {} ({})\n**Change:** ` {} ` ➔ ` {} `", 
            proj.title, target_num, item_title, target_field.name, target_field.data_type, current_val, value
        ))
        .color(0xFEE75C);

    let ctx_id = ctx.id();
    let confirm_id = format!("edit_confirm_{}", ctx_id);
    let cancel_id = format!("edit_cancel_{}", ctx_id);

    let components = vec![serenity::CreateActionRow::Buttons(vec![
        serenity::CreateButton::new(&confirm_id).label("Confirm").style(serenity::ButtonStyle::Success),
        serenity::CreateButton::new(&cancel_id).label("Cancel").style(serenity::ButtonStyle::Danger),
    ])];

    let reply = ctx.send(poise::CreateReply::default().embed(embed).components(components)).await?;
    
    // 5. Interaction Loop
    let interaction = reply.message().await?.await_component_interaction(ctx)
        .author_id(ctx.author().id)
        .timeout(Duration::from_secs(60))
        .await;

    if let Some(mci) = interaction {
        if mci.data.custom_id == confirm_id {
            mci.defer(ctx).await?;
            
            // Construct Mutation based on Type
            // If option_id exists, it's a Single Select or Iteration
            // If not, use known data type from cache to decide mutation input
            
            let mutation = if let Some(opt_id) = option_id {
                 serde_json::json!({
                    "query": r#"
                        mutation($projectId: ID!, $itemId: ID!, $fieldId: ID!, $optionId: String!) {
                            updateProjectV2ItemFieldValue(input: {
                                projectId: $projectId
                                itemId: $itemId
                                fieldId: $fieldId
                                value: { singleSelectOptionId: $optionId } 
                            }) { projectV2Item { id } }
                        }
                    "#,
                    "variables": {
                        "projectId": proj.id,
                        "itemId": item_node_id,
                        "fieldId": target_field.id,
                        "optionId": opt_id
                    }
                })
            } else {
                 match target_field.data_type.as_str() {
                    "NUMBER" => {
                        let num_val = value.parse::<f64>().unwrap_or(0.0);
                        serde_json::json!({
                            "query": r#"
                                mutation($projectId: ID!, $itemId: ID!, $fieldId: ID!, $numVal: Float!) {
                                    updateProjectV2ItemFieldValue(input: {
                                        projectId: $projectId
                                        itemId: $itemId
                                        fieldId: $fieldId
                                        value: { number: $numVal }
                                    }) { projectV2Item { id } }
                                }
                            "#,
                            "variables": { "projectId": proj.id, "itemId": item_node_id, "fieldId": target_field.id, "numVal": num_val }
                        })
                    },
                    "DATE" => {
                        // Date must be ISO-8601 string, handle "Today" helper
                        let date_val = if value.eq_ignore_ascii_case("Today") {
                            chrono::Utc::now().format("%Y-%m-%d").to_string()
                        } else {
                            value.clone()
                        };
                        
                        serde_json::json!({
                            "query": r#"
                                mutation($projectId: ID!, $itemId: ID!, $fieldId: ID!, $dateVal: Date!) {
                                    updateProjectV2ItemFieldValue(input: {
                                        projectId: $projectId
                                        itemId: $itemId
                                        fieldId: $fieldId
                                        value: { date: $dateVal }
                                    }) { projectV2Item { id } }
                                }
                            "#,
                            "variables": { "projectId": proj.id, "itemId": item_node_id, "fieldId": target_field.id, "dateVal": date_val }
                        })
                    },
                    _ => {
                        // TEXT and fallbacks
                         serde_json::json!({
                            "query": r#"
                                mutation($projectId: ID!, $itemId: ID!, $fieldId: ID!, $textVal: String!) {
                                    updateProjectV2ItemFieldValue(input: {
                                        projectId: $projectId
                                        itemId: $itemId
                                        fieldId: $fieldId
                                        value: { text: $textVal }
                                    }) { projectV2Item { id } }
                                }
                            "#,
                            "variables": { "projectId": proj.id, "itemId": item_node_id, "fieldId": target_field.id, "textVal": value }
                        })
                    }
                 }
            };

            match state.octocrab.graphql::<serde_json::Value>(&mutation).await {
                Ok(_) => {
                    let success_embed = serenity::CreateEmbed::new()
                        .title("✅ Edit Successful")
                        .description(format!("Updated **{}** to **{}**.", target_field.name, value))
                        .color(0x57F287);
                    mci.edit_response(ctx, serenity::EditInteractionResponse::new().embed(success_embed).components(vec![])).await?;
                },
                Err(e) => {
                     // If it failed, maybe we used wrong value type. Report error.
                    let fail_embed = serenity::CreateEmbed::new()
                        .title("❌ Edit Failed")
                        .description(format!("Error: {}\n*Check if value type (Text/Number/Date) matches field.*", e))
                        .color(0xED4245);
                    mci.edit_response(ctx, serenity::EditInteractionResponse::new().embed(fail_embed).components(vec![])).await?;
                }
            }
        } else {
             mci.create_response(ctx, serenity::CreateInteractionResponse::UpdateMessage(
                serenity::CreateInteractionResponseMessage::new().content("❌ Cancelled.").components(vec![])
            )).await?;
        }
    } else {
         let _ = reply.edit(ctx, poise::CreateReply::default().content("⏰ Timed out.").components(vec![])).await;
    }
    Ok(())
}

/// Manage User Identity
#[poise::command(slash_command, subcommands("connect", "view", "disconnect"))]
pub async fn user(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

/// Disconnect your Discord account from GitHub
#[poise::command(slash_command)]
pub async fn disconnect(ctx: Context<'_>) -> Result<(), Error> {
    let discord_id = ctx.author().id.get();
    let state = ctx.data();

    let removed = {
        let mut mapping = state.user_mapping.write().await;
        let res = mapping.map.remove(&discord_id);
        if res.is_some() {
            mapping.save();
        }
        res
    };

    match removed {
        Some(gh_user) => {
            ctx.say(format!("✅ Successfully disconnected from GitHub user **{}**.", gh_user)).await?;
        }
        None => {
            ctx.say("ℹ️ You are not currently connected to any GitHub account.").await?;
        }
    }
    Ok(())
}

/// Connect your Discord account to GitHub using OAuth Device Flow
#[poise::command(slash_command, ephemeral)]
pub async fn connect(ctx: Context<'_>) -> Result<(), Error> {
    let state = ctx.data();
    let client_id = &state.github_client_id;
    let discord_id = ctx.author().id.get();
    
    // Check if already connected
    {
        let mapping = state.user_mapping.read().await;
        if let Some(gh_user) = mapping.map.get(&discord_id) {
            ctx.say(format!("✅ You are already connected as GitHub user **{}**.\nUse `/user disconnect` if you wish to change accounts.", gh_user)).await?;
            return Ok(());
        }
    }
    
    ctx.defer_ephemeral().await?;

    // 1. Request Device Code
    let params = [("client_id", client_id.as_str()), ("scope", "read:user")];
    let res = state.http_client.post("https://github.com/login/device/code")
        .header("Accept", "application/json")
        .form(&params)
        .send()
        .await?;
    
    if !res.status().is_success() {
        ctx.say(format!("❌ Failed to initiate device flow: {}", res.status())).await?;
        return Ok(());
    }

    let body: serde_json::Value = res.json().await?;
    let device_code = body["device_code"].as_str().ok_or("missing device_code")?.to_string();
    let user_code = body["user_code"].as_str().ok_or("missing user_code")?.to_string();
    let verification_uri = body["verification_uri"].as_str().ok_or("missing verification_uri")?.to_string();
    let interval = body["interval"].as_u64().unwrap_or(5);

    // 2. Instruct User
    let embed = serenity::CreateEmbed::new()
        .title("🔗 Connect to GitHub")
        .description(format!("To link your account, please complete the device flow:\n\n1. Click **[Login to GitHub]({})**\n2. Enter code: `{}`", verification_uri, user_code))
        .footer(serenity::CreateEmbedFooter::new("I will automatically check when you are done..."))
        .color(0x5865F2);

    let reply_handle = ctx.send(poise::CreateReply::default().embed(embed)).await?;

    // 3. Poll for Token
    let start_time = std::time::Instant::now();
    
    let access_token = loop {
        if start_time.elapsed().as_secs() > 900 { // 15 min timeout
            reply_handle.edit(ctx, poise::CreateReply::default().content("⏰ **Timeout**: Connection cancelled.")).await?;
            return Ok(());
        }
        
        tokio::time::sleep(Duration::from_secs(interval + 1)).await;
        
        let params = [
            ("client_id", client_id.as_str()), 
            ("device_code", device_code.as_str()), 
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code")
        ];
        
        let res = state.http_client.post("https://github.com/login/oauth/access_token")
            .header("Accept", "application/json")
            .form(&params)
            .send()
            .await?;
            
        if let Ok(body) = res.json::<serde_json::Value>().await {
            if let Some(token) = body["access_token"].as_str() {
                break token.to_string();
            }
            if let Some(err) = body["error"].as_str() {
                if err == "access_denied" {
                     reply_handle.edit(ctx, poise::CreateReply::default().content("❌ **Denied**: Access denied by user.")).await?;
                     return Ok(());
                }
                // "authorization_pending" is expected, continue loop
            }
        }
    };

    // 4. Fetch User Identity with Token
    let user_res = state.http_client.get("https://api.github.com/user")
        .header("Authorization", format!("Bearer {}", access_token))
        .header("User-Agent", "usthingy-bot")
        .send()
        .await?;

    if !user_res.status().is_success() {
         reply_handle.edit(ctx, poise::CreateReply::default().content("❌ **Error**: Failed to fetch user info after auth.")).await?;
         return Ok(());
    }

    let user_body: serde_json::Value = user_res.json().await?;
    let github_login = user_body["login"].as_str().ok_or("missing login")?.to_string();

    // 5. Save Mapping
    {
        let mut mapping = state.user_mapping.write().await;
        mapping.map.insert(discord_id, github_login.clone());
        mapping.save();
    }

    reply_handle.edit(ctx, poise::CreateReply::default().content(format!("✅ **Success!** Linked to GitHub account **{}**.", github_login))).await?;
    Ok(())
}

/// View a user's assigned issues, PRs, and review requests
#[poise::command(slash_command)]
pub async fn view(
    ctx: Context<'_>,
    #[description = "GitHub Username (defaults to you if connected)"] 
    #[autocomplete = "user_autocomplete"]
    user: Option<String>,
) -> Result<(), Error> {
    let state = ctx.data();
    let org = &state.github_org;
    
    // Determine target user
    let target_user = if let Some(u) = user {
        u
    } else {
        // Default to self
        check_auth(ctx).await?
    };
    
    ctx.defer().await?;

    // 1. Assigned Issues
    let issues_query = format!("org:{} assignee:{} is:issue is:open", org, target_user);
    // 2. Open PRs
    let prs_query = format!("org:{} author:{} is:pr is:open", org, target_user);
    // 3. Review Requests
    let reviews_query = format!("org:{} review-requested:{} is:pr is:open", org, target_user);

    let (issues_res, prs_res, reviews_res) = tokio::join!(
        state.octocrab.search().issues_and_pull_requests(&issues_query).per_page(5).send(),
        state.octocrab.search().issues_and_pull_requests(&prs_query).per_page(5).send(),
        state.octocrab.search().issues_and_pull_requests(&reviews_query).per_page(5).send()
    );

    let mut embed = serenity::CreateEmbed::new()
        .title(format!("User: {}", target_user))
        .url(format!("https://github.com/{}", target_user))
        .color(0x5865F2);

    // Helper to format list
    fn format_list(items: Vec<octocrab::models::issues::Issue>) -> String {
        if items.is_empty() { return "None".to_string(); }
        items.into_iter().map(|i| {
            let repo = i.repository_url.as_str().split('/').last().unwrap_or("?");
            format!("• **{}/#{}** [{}]({})", repo, i.number, i.title, i.html_url)
        }).collect::<Vec<_>>().join("\n")
    }

    if let Ok(page) = issues_res {
        embed = embed.field("🛠️ Assigned Issues", format_list(page.items), false);
    }
    if let Ok(page) = prs_res {
        embed = embed.field("🚀 Open PRs", format_list(page.items), false);
    }
    if let Ok(page) = reviews_res {
        embed = embed.field("👀 Review Requests", format_list(page.items), false);
    }

    ctx.send(poise::CreateReply::default().embed(embed)).await?;

    Ok(())
}

/// Manually trigger cache refresh
#[poise::command(slash_command, owners_only)]
pub async fn refresh(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say("🔄 Refreshing cache...").await?;
    refresh_cache(&ctx.data()).await?;
    ctx.say("✅ Cache refreshed!").await?;
    Ok(())
}