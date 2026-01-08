use crate::types::Context;

pub async fn repo_autocomplete<'a>(
    ctx: Context<'_>,
    partial: &'a str,
) -> impl Iterator<Item = String> + 'a {
    let state = ctx.data();
    let repos = state.repos.read().await;
    
    let partial = partial.to_lowercase();
    repos.iter()
        .map(|r| r.name.clone())
        .filter(move |name| name.to_lowercase().contains(&partial))
        .collect::<Vec<_>>()
        .into_iter()
}

pub async fn project_autocomplete<'a>(
    ctx: Context<'_>,
    partial: &'a str,
) -> impl Iterator<Item = String> + 'a {
    let state = ctx.data();
    let projects = state.projects.read().await;
    
    let partial = partial.to_lowercase();
    projects.iter()
        .map(|p| p.title.clone())
        .filter(move |title| title.to_lowercase().contains(&partial))
        .collect::<Vec<_>>()
        .into_iter()
}

pub async fn item_autocomplete<'a>(
    ctx: Context<'_>,
    partial: &'a str,
) -> impl Iterator<Item = String> + 'a {
    let state = ctx.data();
    let projects = state.projects.read().await;
    
    // Try to find the 'title' argument value from the interaction context
    // This is heuristic-based as Poise hides raw interaction details a bit in autocomplete
    // but often we can just search ALL cached items if partial is distinctive enough
    // OR we scan through projects to see if one matches the context.
    
    // If we assume the user already picked a project (or typed it), we can try to find it.
    // However, Poise/Discord doesn't send sibling options reliably in autocomplete trigger.
    // A better UX pattern: Search across ALL projects' items.
    
    let partial_lower = partial.to_lowercase();
    let mut suggestions = Vec::new();
    
    for proj in projects.iter() {
        for item in &proj.items {
            let label = format!("{} #{}", item.repo_name, item.number);
            let _val = format!("{} #{}", item.repo_name, item.number); // Value to return
            
            // Match against "123", "repo #123", "title"
            if label.to_lowercase().contains(&partial_lower) || 
               item.title.to_lowercase().contains(&partial_lower) ||
               item.number.to_string().contains(&partial_lower) 
            {
                // Filter out closed items if possible
                let is_closed = item.state.eq_ignore_ascii_case("CLOSED") || item.state.eq_ignore_ascii_case("MERGED");
                if is_closed { continue; }

                // Format: "Repo #123: Title (Project)"
                let display = format!("{} #{}: {} ({})", item.repo_name, item.number, 
                    if item.title.len() > 30 { format!("{}...", &item.title[..30]) } else { item.title.clone() },
                    if proj.title.len() > 15 { format!("{}...", &proj.title[..15]) } else { proj.title.clone() }
                );
                suggestions.push(display); // We return the display as value for now or we need a KV structure
                                           // Discord autocomplete allows Name/Value pairs. Poise Iterator<String> uses Name=Value.
                                           // We'll return the string the user should "send", which is "Repo #123"
                                           
                if suggestions.len() >= 25 { break; } 
            }
        }
        if suggestions.len() >= 25 { break; }
    }
    
    // Fallback if empty or just basic input
    if suggestions.is_empty() {
        suggestions.push(partial.to_string());
    }
    
    suggestions.into_iter()
}

pub async fn user_autocomplete<'a>(
    ctx: Context<'_>,
    partial: &'a str,
) -> impl Iterator<Item = String> + 'a {
    let state = ctx.data();
    let users = state.users.read().await;
    
    let partial = partial.to_lowercase();
    users.iter()
        .map(|u| u.login.clone())
        .filter(move |login| login.to_lowercase().contains(&partial))
        .collect::<Vec<_>>()
        .into_iter()
}

pub async fn field_autocomplete<'a>(
    ctx: Context<'_>,
    partial: &'a str,
) -> impl Iterator<Item = String> + 'a {
    let state = ctx.data();
    let projects = state.projects.read().await;
    
    // Heuristic: try to find which project the user selected.
    // Poise doesn't give us easy access to other args in autocomplete.
    // We'll return ALL unique fields across all projects or just common ones if we can't tell.
    // Better: Filter by project if 'title' arg was filled? (Not possible easily)
    // We'll just list all unique field names found in cache.
    
    let partial = partial.to_lowercase();
    let mut fields = std::collections::HashSet::new();
    
    for p in projects.iter() {
        for f in &p.fields {
            fields.insert(f.name.clone());
        }
    }
    
    fields.into_iter()
        .filter(move |name| name.to_lowercase().contains(&partial))
        .collect::<Vec<_>>()
        .into_iter()
}

pub async fn value_autocomplete<'a>(
    ctx: Context<'_>,
    partial: &'a str,
) -> impl Iterator<Item = String> + 'a {
    // Quick exit if partial is too long
    if partial.len() > 50 {
        return vec![partial.to_string()].into_iter();
    }

    let state = ctx.data();
    
    // 1. Determine selected field from interaction context (no lock needed yet)
    let mut selected_field_name = None;
    if let poise::Context::Application(app_ctx) = ctx {
        let data = &app_ctx.interaction.data;
        for opt in &data.options {
            if opt.name == "field" {
                if let Some(val) = opt.value.as_str() {
                    selected_field_name = Some(val.to_string());
                }
            }
        }
    }

    // 2. Fetch options efficiently (minimize lock time)
    let options = {
        let projects = state.projects.read().await;
        let mut opts = Vec::new();
        let mut seen = std::collections::HashSet::new();
        
        if let Some(field_name) = selected_field_name {
            let field_name_lower = field_name.to_lowercase();
            // Iterate all projects to find this field definition
            // We stop after finding enough matches or scanning all
            
            for p in projects.iter() {
                if let Some(f) = p.fields.iter().find(|f| f.name.to_lowercase() == field_name_lower) {
                    match f.data_type.as_str() {
                        "SINGLE_SELECT" | "ITERATION" | "STATUS" => {
                            for (opt_name, _) in &f.options {
                                if seen.insert(opt_name.clone()) {
                                    opts.push(opt_name.clone());
                                }
                            }
                        },
                        "DATE" => {
                            opts.push("Today".to_string());
                            opts.push("YYYY-MM-DD".to_string());
                        },
                        _ => {} 
                    }
                }
            }
        }
        opts
    }; // Lock dropped here

    // 3. Filter match (outside lock)
    let partial = partial.to_lowercase();
    let mut suggestions = options.into_iter()
        .filter(|o| o.to_lowercase().contains(&partial))
        .take(25)
        .collect::<Vec<_>>();

    // Always suggest the raw input if it's new (for text/date/number fields)
    if !partial.is_empty() && !suggestions.iter().any(|s| s.to_lowercase() == partial) {
        suggestions.push(partial.clone());
    }
    
    suggestions.into_iter()
}