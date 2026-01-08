use poise::serenity_prelude as serenity;
use std::sync::Arc;
use crate::types::{BotState, Error};
use crate::utils::build_item_embed;
use serenity::all::{CreateSelectMenu, CreateSelectMenuKind, CreateSelectMenuOption, CreateModal, CreateInputText};

// Event Handler for Components
pub async fn event_handler(
    ctx: &serenity::Context,
    event: &serenity::FullEvent,
    _framework: poise::FrameworkContext<'_, Arc<BotState>, Error>,
    data: &Arc<BotState>,
) -> Result<(), Error> {
    if let serenity::FullEvent::InteractionCreate { interaction } = event {
        match interaction {
            serenity::Interaction::Component(component) => {
                let custom_id = &component.data.custom_id;
                
                // Format: proj_page_<title>_<page_num>
                if custom_id.starts_with("proj_page_") {
                    let parts: Vec<&str> = custom_id.split('_').collect();
                    if parts.len() >= 4 {
                        let title = parts[2];
                        let page_num: usize = parts[3].parse().unwrap_or(1);
                        
                        let _ = component.defer(ctx).await;
                        
                         let project_opt = {
                            let projects = data.projects.read().await;
                            projects.iter().find(|p| p.title.eq_ignore_ascii_case(title)).cloned()
                        };

                        if let Some(proj) = project_opt {
                             let query = serde_json::json!({
                                "query": r#"query($id: ID!) { node(id: $id) { ... on ProjectV2 { items(first: 100) { nodes { content { ... on Issue { title number url repository { name } state } ... on PullRequest { title number url repository { name } state } ... on DraftIssue { title } } } } } } }"#,
                                "variables": { "id": proj.id }
                            });
                            
                            if let Ok(resp) = data.octocrab.graphql::<serde_json::Value>(&query).await {
                                 let json_resp: serde_json::Value = resp;
                                 
                                 // Redoing the list building logic (simplified)
                                 let mut display_lines = Vec::new();
                                 let mut menu_options = Vec::new();

                                 if let Some(nodes) = json_resp.get("data").and_then(|d| d.get("node")).and_then(|d| d.get("items")).and_then(|d| d.get("nodes")).and_then(|d| d.as_array()) {
                                     for item in nodes {
                                         let content = item.get("content");
                                          if let Some(issue) = content.and_then(|c| c.get("number")) {
                                                let title = content.and_then(|c| c.get("title")).and_then(|t| t.as_str()).unwrap_or("?");
                                                let repo = content.and_then(|c| c.get("repository")).and_then(|r| r.get("name")).and_then(|n| n.as_str()).unwrap_or("?");
                                                let number = issue.as_i64().unwrap_or(0);
                                                let url = content.and_then(|c| c.get("url")).and_then(|u| u.as_str()).unwrap_or("");
                                                let state = content.and_then(|c| c.get("state")).and_then(|s| s.as_str()).unwrap_or("");
                                                
                                                let icon = match state {
                                                    "OPEN" => "<:issue:1458877117176742065>",
                                                    "CLOSED" => "<:issue_neutral:1458877524015579209>",
                                                    "MERGED" => "<:pr_merged:1458877132414517360>",
                                                    _ => "⚪",
                                                };
                                                display_lines.push(format!("{} **{}/[#{}]({})** {}", icon, repo, number, url, title));
                                                menu_options.push((number, title.to_string(), repo.to_string()));
                                          } else if let Some(draft) = content.and_then(|c| c.get("title")).and_then(|t| t.as_str()) {
                                                display_lines.push(format!("<:project_item_draft:1458877119789797580> **Draft:** {}", draft));
                                          }
                                     }
                                 }
                                 
                                 let page_size = 20;
                                 let total_items = display_lines.len();
                                 let start_idx = (page_num - 1) * page_size;
                                 let end_idx = std::cmp::min(start_idx + page_size, total_items);
                                 
                                 if start_idx < total_items {
                                     let page_display = &display_lines[start_idx..end_idx];
                                     let page_menu_opts = menu_options.iter().skip(start_idx).take(page_size);
                                     
                                     let embed = serenity::CreateEmbed::new()
                                        .title(format!("Project: {}", proj.title))
                                        .url(&proj.url)
                                        .description(page_display.join("\n"))
                                        .footer(serenity::CreateEmbedFooter::new(format!("Page {}/{} • Total: {}", page_num, (total_items + page_size - 1) / page_size, total_items)))
                                        .color(0xEB459E);
                                        
                                     let mut components = Vec::new();

                                     // Select Menu
                                    let mut select_opts_vec = Vec::new();
                                    for (num, title, repo) in page_menu_opts {
                                        let label = format!("{} #{}: {}", repo, num, title);
                                        let label = if label.len() > 95 { format!("{}...", &label[..95]) } else { label };
                                        select_opts_vec.push(CreateSelectMenuOption::new(label, num.to_string()));
                                    }
                                    if !select_opts_vec.is_empty() {
                                        let menu_id = format!("proj_select_{}", proj.id);
                                        let menu = CreateSelectMenu::new(menu_id, CreateSelectMenuKind::String { options: select_opts_vec })
                                            .placeholder("🔍 Select an item to view details...");
                                        components.push(serenity::CreateActionRow::SelectMenu(menu));
                                    }
                                        
                                     let mut buttons = Vec::new();
                                    if page_num > 1 {
                                        buttons.push(serenity::CreateButton::new(format!("proj_page_{}_{}", title, page_num - 1)).label("◀️ Prev").style(serenity::ButtonStyle::Secondary));
                                    }
                                    buttons.push(serenity::CreateButton::new_link(&proj.url).label("Open Board"));
                                    if end_idx < total_items {
                                         buttons.push(serenity::CreateButton::new(format!("proj_page_{}_{}", title, page_num + 1)).label("Next ▶️").style(serenity::ButtonStyle::Secondary));
                                    }
                                    components.push(serenity::CreateActionRow::Buttons(buttons));
                                    
                                    let _ = component.edit_response(ctx, serenity::EditInteractionResponse::new().embed(embed).components(components)).await;
                                 }
                            }
                        }
                    }
                } else if custom_id.starts_with("proj_select_") {
                     let proj_id = custom_id.trim_start_matches("proj_select_");
                     
                     if let serenity::ComponentInteractionDataKind::StringSelect { values } = &component.data.kind {
                         if let Some(val) = values.first() {
                             let target_num: i64 = val.parse().unwrap_or(0);
                             let _ = component.defer(ctx).await;
                             
                             let query = serde_json::json!({
                                "query": r#"
                                    query($id: ID!) {
                                        node(id: $id) {
                                            ... on ProjectV2 {
                                                items(first: 100) {
                                                    nodes {
                                                        content {
                                                            ... on Issue {
                                                                title number body url repository { name } state assignees(first: 3) { nodes { login } } labels(first: 5) { nodes { name } }
                                                            }
                                                            ... on PullRequest {
                                                                title number body url repository { name } state assignees(first: 3) { nodes { login } }
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
                                "variables": { "id": proj_id }
                            });

                            if let Ok(resp) = data.octocrab.graphql::<serde_json::Value>(&query).await {
                                 let json_resp: serde_json::Value = resp;
                                 if let Some(nodes) = json_resp.get("data").and_then(|d| d.get("node")).and_then(|d| d.get("items")).and_then(|d| d.get("nodes")).and_then(|d| d.as_array()) {
                                     for item in nodes {
                                         if let Some(embed) = build_item_embed(item, target_num) {
                                             let components = vec![serenity::CreateActionRow::Buttons(vec![
                                                serenity::CreateButton::new(format!("edit:item:{}:{}", proj_id, target_num))
                                                    .label("✏️ Edit Item")
                                                    .style(serenity::ButtonStyle::Secondary)
                                             ])];
                                             let _ = component.create_followup(ctx, serenity::CreateInteractionResponseFollowup::new().embed(embed).components(components).ephemeral(true)).await;
                                             return Ok(());
                                         }
                                     }
                                     let _ = component.create_followup(ctx, serenity::CreateInteractionResponseFollowup::new().content("❌ Item not found (it might have been moved).").ephemeral(true)).await;
                                 }
                            }
                         }
                     }
                } else if custom_id.starts_with("edit:item:") {
                    // edit:item:{proj_id}:{num}
                    let parts: Vec<&str> = custom_id.split(':').collect();
                    if parts.len() >= 4 {
                        let proj_id = parts[2];
                        let target_num: i64 = parts[3].parse().unwrap_or(0);
                        
                        let projects = data.projects.read().await;
                        if let Some(proj) = projects.iter().find(|p| p.id == proj_id) {
                             let mut options = Vec::new();
                             for f in &proj.fields {
                                 if f.name == "Title" || f.name == "Assignees" || f.name == "Labels" || f.name == "Repository" || f.name == "Milestone" || f.name == "Linked pull requests" { continue; }
                                 let label = format!("{} ({})", f.name, f.data_type);
                                 options.push(CreateSelectMenuOption::new(label, &f.id));
                             }
                             
                             if options.is_empty() {
                                 let _ = component.create_response(ctx, serenity::CreateInteractionResponse::Message(
                                     serenity::CreateInteractionResponseMessage::new().content("⚠️ No editable custom fields found.").ephemeral(true)
                                 )).await;
                             } else {
                                 let menu_id = format!("field:sel:{}:{}", proj_id, target_num);
                                 let menu = CreateSelectMenu::new(menu_id, CreateSelectMenuKind::String { options })
                                     .placeholder("Select a field to edit...");
                                 
                                 let _ = component.create_response(ctx, serenity::CreateInteractionResponse::Message(
                                     serenity::CreateInteractionResponseMessage::new()
                                         .content(format!("Select a field to edit for Item #{}", target_num))
                                         .components(vec![serenity::CreateActionRow::SelectMenu(menu)])
                                         .ephemeral(true)
                                 )).await;
                             }
                        }
                    }
                } else if custom_id.starts_with("field:sel:") {
                    // field:sel:{proj_id}:{num}
                    let parts: Vec<&str> = custom_id.split(':').collect();
                    if parts.len() >= 4 {
                        let proj_id = parts[2];
                        let target_num: i64 = parts[3].parse().unwrap_or(0);
                        
                        if let serenity::ComponentInteractionDataKind::StringSelect { values } = &component.data.kind {
                            if let Some(field_id) = values.first() {
                                let projects = data.projects.read().await;
                                if let Some(proj) = projects.iter().find(|p| p.id == proj_id) {
                                    if let Some(field) = proj.fields.iter().find(|f| f.id == *field_id) {
                                        match field.data_type.as_str() {
                                            "SINGLE_SELECT" | "ITERATION" | "STATUS" => {
                                                let mut sorted_opts: Vec<_> = field.options.iter().collect();
                                                sorted_opts.sort_by_key(|(name, _)| name.to_lowercase());
                                                
                                                let mut options = Vec::new();
                                                for (name, id) in sorted_opts {
                                                    options.push(CreateSelectMenuOption::new(name, id));
                                                }
                                                
                                                if options.is_empty() {
                                                     let _ = component.create_response(ctx, serenity::CreateInteractionResponse::Message(
                                                        serenity::CreateInteractionResponseMessage::new().content("❌ No options found.").ephemeral(true)
                                                    )).await;
                                                } else {
                                                    let menu_id = format!("val:sel:{}:{}:{}", proj_id, target_num, field_id);
                                                    let menu = CreateSelectMenu::new(menu_id, CreateSelectMenuKind::String { options })
                                                        .placeholder(format!("Select value for {}...", field.name));
                                                    
                                                    let _ = component.create_response(ctx, serenity::CreateInteractionResponse::UpdateMessage(
                                                        serenity::CreateInteractionResponseMessage::new()
                                                            .content(format!("Update **{}** for Item #{}", field.name, target_num))
                                                            .components(vec![serenity::CreateActionRow::SelectMenu(menu)])
                                                    )).await;
                                                }
                                            },
                                            _ => {
                                                let modal_id = format!("val:modal:{}:{}:{}", proj_id, target_num, field_id);
                                                let input = CreateInputText::new(serenity::InputTextStyle::Short, "Value", "value")
                                                    .placeholder(format!("Enter new {}...", field.data_type.to_lowercase()));
                                                
                                                let modal = CreateModal::new(modal_id, format!("Edit {} (#{})", field.name, target_num))
                                                    .components(vec![serenity::CreateActionRow::InputText(input)]);
                                                
                                                let _ = component.create_response(ctx, serenity::CreateInteractionResponse::Modal(modal)).await;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                } else if custom_id.starts_with("val:sel:") {
                    // val:sel:{proj_id}:{num}:{field_id}
                    let parts: Vec<&str> = custom_id.split(':').collect();
                    if parts.len() >= 5 {
                        let proj_id = parts[2];
                        let target_num: i64 = parts[3].parse().unwrap_or(0);
                        let field_id = parts[4];
                        
                        if let serenity::ComponentInteractionDataKind::StringSelect { values } = &component.data.kind {
                             if let Some(opt_id) = values.first() {
                                 let _ = component.defer(ctx).await;
                                 
                                 // Fetch Item Node ID
                                 let query = serde_json::json!({
                                    "query": r#"query($id: ID!) { node(id: $id) { ... on ProjectV2 { items(first: 100) { nodes { id content { ... on Issue { number } ... on PullRequest { number } } } } } } }"#,
                                    "variables": { "id": proj_id }
                                });
                                
                                let mut item_node_id = String::new();
                                if let Ok(resp) = data.octocrab.graphql::<serde_json::Value>(&query).await {
                                     let json_resp: serde_json::Value = resp;
                                     if let Some(nodes) = json_resp.get("data").and_then(|d| d.get("node")).and_then(|d| d.get("items")).and_then(|d| d.get("nodes")).and_then(|d| d.as_array()) {
                                         for item in nodes {
                                             if let Some(num) = item.get("content").and_then(|c| c.get("number")).and_then(|n| n.as_i64()) {
                                                 if num == target_num {
                                                     item_node_id = item.get("id").and_then(|s| s.as_str()).unwrap_or("").to_string();
                                                     break;
                                                 }
                                             }
                                         }
                                     }
                                }
                                
                                if !item_node_id.is_empty() {
                                    let mutation = serde_json::json!({
                                        "query": r#"
                                            mutation($projectId: ID!, $itemId: ID!, $fieldId: ID!, $optionId: String!) {
                                                updateProjectV2ItemFieldValue(input: {
                                                    projectId: $projectId, itemId: $itemId, fieldId: $fieldId, value: { singleSelectOptionId: $optionId } 
                                                }) { projectV2Item { id } }
                                            }
                                        "#,
                                        "variables": { "projectId": proj_id, "itemId": item_node_id, "fieldId": field_id, "optionId": opt_id }
                                    });
                                    
                                    if let Ok(_) = data.octocrab.graphql::<serde_json::Value>(&mutation).await {
                                         let _ = component.edit_response(ctx, serenity::EditInteractionResponse::new().content("✅ Updated successfully!").components(vec![])).await;
                                    } else {
                                         let _ = component.edit_response(ctx, serenity::EditInteractionResponse::new().content("❌ Update failed.").components(vec![])).await;
                                    }
                                }
                             }
                        }
                    }
                } else if custom_id.starts_with("edit_item_") {
                     let _ = component.create_response(ctx, serenity::CreateInteractionResponse::Message(
                        serenity::CreateInteractionResponseMessage::new()
                            .content("⚠️ This button is outdated. Please run `/proj view` again.").ephemeral(true)
                    )).await;
                }
            },
            serenity::Interaction::Modal(modal) => {
                let custom_id = &modal.data.custom_id;
                 if custom_id.starts_with("val:modal:") {
                    // val:modal:{proj_id}:{num}:{field_id}
                    let parts: Vec<&str> = custom_id.split(':').collect();
                     if parts.len() >= 5 {
                        let proj_id = parts[2];
                        let target_num: i64 = parts[3].parse().unwrap_or(0);
                        let field_id = parts[4];
                        
                        let mut value_opt = None;
                        for row in &modal.data.components {
                            for comp in &row.components {
                                if let serenity::all::ActionRowComponent::InputText(input) = comp {
                                    value_opt = input.value.clone();
                                    break;
                                }
                            }
                        }

                        if let Some(value) = value_opt {
                                     let _ = modal.defer(ctx).await;
                                     
                                     // Get data type
                                     let mut data_type = "TEXT".to_string();
                                     {
                                         let projects = data.projects.read().await;
                                         if let Some(p) = projects.iter().find(|p| p.id == proj_id) {
                                             if let Some(f) = p.fields.iter().find(|f| f.id == field_id) {
                                                 data_type = f.data_type.clone();
                                             }
                                         }
                                     }

                                    // Fetch Item Node ID
                                    let query = serde_json::json!({
                                        "query": r#"query($id: ID!) { node(id: $id) { ... on ProjectV2 { items(first: 100) { nodes { id content { ... on Issue { number } ... on PullRequest { number } } } } } } }"#,
                                        "variables": { "id": proj_id }
                                    });
                                    
                                    let mut item_node_id = String::new();
                                    if let Ok(resp) = data.octocrab.graphql::<serde_json::Value>(&query).await {
                                         let json_resp: serde_json::Value = resp;
                                         if let Some(nodes) = json_resp.get("data").and_then(|d| d.get("node")).and_then(|d| d.get("items")).and_then(|d| d.get("nodes")).and_then(|d| d.as_array()) {
                                             for item in nodes {
                                                 if let Some(num) = item.get("content").and_then(|c| c.get("number")).and_then(|n| n.as_i64()) {
                                                     if num == target_num {
                                                         item_node_id = item.get("id").and_then(|s| s.as_str()).unwrap_or("").to_string();
                                                         break;
                                                     }
                                                 }
                                             }
                                         }
                                    }
                                    
                                    if !item_node_id.is_empty() {
                                        let mutation = match data_type.as_str() {
                                            "NUMBER" => {
                                                let num_val = value.parse::<f64>().unwrap_or(0.0);
                                                serde_json::json!({
                                                    "query": r#"mutation($p: ID!, $i: ID!, $f: ID!, $v: Float!) { updateProjectV2ItemFieldValue(input: { projectId: $p, itemId: $i, fieldId: $f, value: { number: $v } }) { projectV2Item { id } } }"#,
                                                    "variables": { "p": proj_id, "i": item_node_id, "f": field_id, "v": num_val }
                                                })
                                            },
                                            "DATE" => {
                                                let date_val = if value.eq_ignore_ascii_case("Today") {
                                                    chrono::Utc::now().format("%Y-%m-%d").to_string()
                                                } else {
                                                    value.clone()
                                                };
                                                 serde_json::json!({
                                                    "query": r#"mutation($p: ID!, $i: ID!, $f: ID!, $v: Date!) { updateProjectV2ItemFieldValue(input: { projectId: $p, itemId: $i, fieldId: $f, value: { date: $v } }) { projectV2Item { id } } }"#,
                                                    "variables": { "p": proj_id, "i": item_node_id, "f": field_id, "v": date_val }
                                                })
                                            },
                                            _ => {
                                                serde_json::json!({
                                                    "query": r#"mutation($p: ID!, $i: ID!, $f: ID!, $v: String!) { updateProjectV2ItemFieldValue(input: { projectId: $p, itemId: $i, fieldId: $f, value: { text: $v } }) { projectV2Item { id } } }"#,
                                                    "variables": { "p": proj_id, "i": item_node_id, "f": field_id, "v": value }
                                                })
                                            }
                                        };

                                        if let Ok(_) = data.octocrab.graphql::<serde_json::Value>(&mutation).await {
                                             let _ = modal.edit_response(ctx, serenity::EditInteractionResponse::new().content(format!("✅ Updated to: {}", value)).components(vec![])).await;
                                        } else {
                                             let _ = modal.edit_response(ctx, serenity::EditInteractionResponse::new().content("❌ Update failed.").components(vec![])).await;
                                        }
                                    }
                        }
                    }
                 }
             },
             _ => {}
        }
    }
    Ok(())
}