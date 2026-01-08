# Usthingy - GitHub Project V2 Discord Bot

A powerful Discord bot built with Rust (Poise + Serenity) to manage GitHub Projects V2. Features a rich, interactive UI with caching, smart autocomplete, and direct editing capabilities.

## Features

*   **View Projects**: browse items with `/proj view`.
*   **Interactive Editing**:
    *   Click "✏️ Edit Item" to open an interactive editing flow.
    *   Dropdown selection for fields (Status, Priority, etc.).
    *   Context-aware modals for text, numbers, and dates (supports "Today" shortcut).
    *   Dynamic dropdowns for Single Select options.
*   **Smart Autocomplete**: Context-aware suggestions for items, fields, and values.
*   **Auth**: Secure OAuth device flow to link Discord users to GitHub accounts.
*   **Live Updates**: Changes reflected immediately in GitHub.

## Architecture

The project is structured into modular components for maintainability:

*   `src/main.rs`: Entry point and dependency injection.
*   `src/types.rs`: Data structures (`CachedProject`, `BotState`).
*   `src/cache.rs`: Logic for refreshing GitHub data (Repos, Users, Projects V2).
*   `src/commands.rs`: Slash command definitions (`/proj`, `/repo`, `/user`).
*   `src/handler.rs`: Event handler for interactive components (buttons, selects, modals).
*   `src/autocomplete.rs`: efficient, lock-minimized autocomplete logic.
*   `src/utils.rs`: Helper functions and embed builders.

## Setup

1.  **Environment Variables**:
    *   `DISCORD_TOKEN`: Your Discord Bot Token.
    *   `GITHUB_TOKEN`: A Personal Access Token (PAT) with `repo`, `read:org`, `project` scopes.
    *   `GITHUB_ORG`: The target GitHub Organization.
    *   `GITHUB_CLIENT_ID`: Client ID for the OAuth App (for user linking).

2.  **Run**:
    ```bash
    cargo run
    ```

## Development

The bot uses a heavy caching strategy to avoid GitHub API rate limits and ensure instant Discord interaction speeds. The `refresh_cache` function populates:
*   Repositories
*   Users (Members & Collaborators)
*   Projects V2 (including Items and Field definitions)

GraphQL is used extensively for Project V2 data as the REST API is limited.
A Discord bot to manage GitHub projects using slash commands. Built with Rust, Poise, and Serenity.

## Features

- **Slash Commands**: `/proj` command to manage issues and PRs.
- **Autofill**: Autocomplete for repository names and actions.
- **GitHub Integration**: Assign issues, target deadlines (extensible).
- **Action Rows**: Interactive buttons for confirmations.

## Setup

1. **Clone the repository:**
   ```bash
   git clone https://github.com/your-org/usthingy.git
   cd usthingy
   ```

2. **Environment Variables:**
   Copy `.env.example` to `.env` and fill in your tokens:
   ```bash
   cp .env.example .env
   ```
   - `DISCORD_TOKEN`: Your Discord Bot Token.
   - `GITHUB_TOKEN`: Your GitHub Personal Access Token (repo scope).
   - `GITHUB_ORG`: The GitHub Organization name.

3. **Run locally:**
   ```bash
   cargo run
   ```

4. **Run with Docker:**
   ```bash
   docker build -t usthingy .
   docker run --env-file .env usthingy
   ```

## Usage

### 🚀 Getting Started
1. **Connect your GitHub**: `/user connect`
   - Follow the link to authorize the bot securely.
2. **View your Dashboard**: `/user view`
   - See assigned issues and PRs waiting for you.

### 📦 Project Management (`/proj`)
- **/proj view** `<project_title>`
  - View items in a project board (e.g., "Backlog").
  - Supports pagination and filtering.
- **/proj view-item** `<project_title> <item_id>`
  - See detailed info for a specific task (description, labels, assignees).
- **/proj list**
  - List all projects in the organization.

### 🛠️ Repository & Issues (`/repo`)
- **/repo assign** `<repo> <issue> <user>`
  - Assign an issue to a user.
- **/repo target** `<repo> <issue> <args>`
  - Set targets/deadlines.
- **/repo issues** `<repo>`
  - List open issues in a repo.

### 👤 User Management (`/user`)
- **/user view [username]**
  - View a user's workload (issues, PRs, reviews).

## Architecture & Dev Experience

- **Framework**: Rust + [Poise](https://github.com/serenity-rs/poise).
- **GitHub API**: [Octocrab](https://github.com/XAMPPRocky/octocrab) + GraphQL for Projects V2.
- **Caching**: In-memory caching for instant autocomplete of Repos, Users, and Projects.
- **Auth**: OAuth Device Flow for secure, token-less user mapping.

### Setup for Developers
1. **Env Vars**:
   - `DISCORD_TOKEN`: Bot Token.
   - `GITHUB_TOKEN`: PAT for bot operations.
   - `GITHUB_ORG`: Target Organization.
   - `GITHUB_CLIENT_ID`: OAuth App Client ID (for user auth).
2. **Run**: `cargo run`

- **GitHub Client**: [Octocrab](https://github.com/XAMPPRocky/octocrab).
- **Runtime**: Tokio.
