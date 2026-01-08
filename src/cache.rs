use std::collections::HashMap;
use std::sync::Arc;
use crate::types::{BotState, CachedRepo, CachedUser, CachedProject, CachedItem, CachedField, Error};

pub async fn refresh_cache(state: &Arc<BotState>) -> Result<(), Error> {
    println!("🔄 Refreshing GitHub cache...");
    let org = &state.github_org;
    let octocrab = &state.octocrab;

    // 1. Fetch Repos
    let mut all_repos = Vec::new();
    match octocrab.orgs(org).list_repos().per_page(100).send().await {
        Ok(page) => {
            match octocrab.all_pages(page).await {
                Ok(repos) => {
                     all_repos = repos.into_iter().map(|r| CachedRepo {
                        name: r.name,
                        full_name: r.full_name.unwrap_or_default(),
                    }).collect();
                }
                Err(e) => println!("⚠️ Failed to paginate repos: {}", e),
            }
        }
        Err(e) => println!("⚠️ Failed to fetch repos: {}", e),
    }
    *state.repos.write().await = all_repos;
    println!("✅ Cached {} repos", state.repos.read().await.len());

    // 2. Fetch Members (Users) & Outside Collaborators
    // Using all_pages to ensure we get everyone, and merging members + collaborators
    let mut all_users_map: HashMap<String, CachedUser> = HashMap::new();
    
    // A. Members
    match octocrab.orgs(org).list_members().per_page(100).send().await {
        Ok(page) => {
            match octocrab.all_pages(page).await {
                Ok(users) => {
                    for u in users {
                        all_users_map.insert(u.login.clone(), CachedUser {
                            login: u.login,
                            avatar_url: u.avatar_url.to_string(),
                        });
                    }
                }
                Err(e) => println!("⚠️ Failed to paginate members: {}", e),
            }
        }
        Err(e) => println!("⚠️ Failed to fetch members (check read:org scope): {}", e),
    }

    // B. Outside Collaborators (Manual request as helper might be missing/named differently)
    // Endpoint: /orgs/{org}/outside_collaborators
    let route = format!("/orgs/{}/outside_collaborators", org);
    match octocrab.get::<octocrab::Page<octocrab::models::Author>, _, _>(route, Some(&[("per_page", "100")])).await {
        Ok(page) => {
             match octocrab.all_pages(page).await {
                Ok(users) => {
                    for u in users {
                        all_users_map.entry(u.login.clone()).or_insert(CachedUser {
                            login: u.login,
                            avatar_url: u.avatar_url.to_string(),
                        });
                    }
                }
                Err(e) => println!("⚠️ Failed to paginate collaborators: {}", e),
            }
        }
        Err(e) => println!("⚠️ Failed to fetch collaborators (might need admin:org or just ignored): {}", e),
    }

    let all_users: Vec<CachedUser> = all_users_map.into_values().collect();
    *state.users.write().await = all_users;
    println!("✅ Cached {} users (Members + Collaborators)", state.users.read().await.len());

    // 3. Fetch Projects (GraphQL for V2)
    // Fetch items inside the project for autocomplete
    let query = serde_json::json!({
        "query": format!(r#"
            query {{
                organization(login: "{}") {{
                    projectsV2(first: 20) {{
                        nodes {{
                            id
                            title
                            url
                            number
                            fields(first: 20) {{
                                nodes {{
                                    ... on ProjectV2FieldCommon {{ id name dataType }}
                                    ... on ProjectV2SingleSelectField {{
                                        id name dataType options {{ id name }}
                                    }}
                                    ... on ProjectV2IterationField {{
                                        id name dataType configuration {{ iterations {{ id title }} }}
                                    }}
                                }}
                            }}
                            items(first: 50) {{
                                nodes {{
                                    content {{
                                        ... on Issue {{ title number repository {{ name }} state }}
                                        ... on PullRequest {{ title number repository {{ name }} state }}
                                    }}
                                }}
                            }}
                        }}
                    }}
                }}
            }}
        "#, org)
    });

    match octocrab.graphql(&query).await {
        Ok(resp) => {
             // Parse generic JSON response manually to avoid complex struct definitions
             let json_resp: serde_json::Value = resp;
             if let Some(data) = json_resp.get("data")
                .and_then(|d| d.get("organization"))
                .and_then(|d| d.get("projectsV2"))
                .and_then(|d| d.get("nodes"))
                .and_then(|d| d.as_array()) 
             {
                 let mut parsed_projects = Vec::new();
                 
                 for p in data.iter() {
                     if let (Some(id), Some(title), Some(url), Some(number)) = (
                         p.get("id").and_then(|s| s.as_str()),
                         p.get("title").and_then(|s| s.as_str()),
                         p.get("url").and_then(|s| s.as_str()),
                         p.get("number").and_then(|n| n.as_i64()),
                     ) {
                         // Extract Fields
                         let mut fields = Vec::new();
                         if let Some(field_nodes) = p.get("fields").and_then(|f| f.get("nodes")).and_then(|n| n.as_array()) {
                             for f in field_nodes {
                                 let f_id = f.get("id").and_then(|s| s.as_str()).unwrap_or("").to_string();
                                 let f_name = f.get("name").and_then(|s| s.as_str()).unwrap_or("").to_string();
                                 let f_type = f.get("dataType").and_then(|s| s.as_str()).unwrap_or("TEXT").to_string();
                                 let mut options = HashMap::new();
                                 
                                 // Single Select Options
                                 if let Some(opts) = f.get("options").and_then(|o| o.as_array()) {
                                     for opt in opts {
                                         if let (Some(o_id), Some(o_name)) = (opt.get("id").and_then(|s| s.as_str()), opt.get("name").and_then(|s| s.as_str())) {
                                             options.insert(o_name.to_string(), o_id.to_string());
                                         }
                                     }
                                 }
                                 // Iteration Options (treated as select for simplicity)
                                 if let Some(iters) = f.get("configuration").and_then(|c| c.get("iterations")).and_then(|i| i.as_array()) {
                                      for iter in iters {
                                          if let (Some(i_id), Some(i_title)) = (iter.get("id").and_then(|s| s.as_str()), iter.get("title").and_then(|s| s.as_str())) {
                                              options.insert(i_title.to_string(), i_id.to_string());
                                          }
                                      }
                                 }
                                 
                                 fields.push(CachedField { id: f_id, name: f_name, data_type: f_type, options });
                             }
                         }

                         // Extract cached items if available in the same query (we need to update query)
                         let mut items = Vec::new();
                         if let Some(nodes) = p.get("items").and_then(|i| i.get("nodes")).and_then(|n| n.as_array()) {
                             for item in nodes {
                                 let content = item.get("content");
                                 if let (Some(i_title), Some(i_num), Some(repo)) = (
                                     content.and_then(|c| c.get("title")).and_then(|s| s.as_str()),
                                     content.and_then(|c| c.get("number")).and_then(|n| n.as_i64()),
                                     content.and_then(|c| c.get("repository")).and_then(|r| r.get("name")).and_then(|s| s.as_str())
                                 ) {
                                     let state = content.and_then(|c| c.get("state")).and_then(|s| s.as_str()).unwrap_or("OPEN");
                                     items.push(CachedItem {
                                         title: i_title.to_string(),
                                         number: i_num,
                                         repo_name: repo.to_string(),
                                         state: state.to_string(),
                                     });
                                 }
                             }
                         }
                         
                         parsed_projects.push(CachedProject {
                             id: id.to_string(),
                             title: title.to_string(),
                             url: url.to_string(),
                             number,
                             items,
                             fields,
                         });
                     }
                 }
                 
                 *state.projects.write().await = parsed_projects;
                 println!("✅ Cached {} projects (V2)", state.projects.read().await.len());
             } else {
                 println!("⚠️ GraphQL response structure mismatch for Projects V2");
             }
        }
        Err(e) => println!("⚠️ Failed to fetch projects via GraphQL: {}", e),
    }

    Ok(())
}