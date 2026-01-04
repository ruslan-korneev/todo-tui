# TODO TUI - Development Roadmap

A terminal-based TODO application with kanban boards, knowledge base, and integrations.

## Tech Stack

- **TUI Client**: Rust + ratatui + crossterm
- **Backend**: Rust + Axum + PostgreSQL
- **Hosting**: Self-hosted VPS

---

## Phase 1: Foundation

### 1.1 Project Setup
- [x] Initialize Cargo workspace with 3 crates
- [x] Create .env.example
- [x] Add .gitignore
- [x] Add rustfmt.toml and clippy config
- [x] Write roadmap

### 1.2 Shared Types (`crates/todo-shared/`)
- [x] Task model (id, title, status, description, priority, due_date, time_estimate)
- [x] User model
- [x] Workspace model (with roles: owner, admin, editor, reader)
- [x] Comment model
- [x] Document model (knowledge base)
- [x] API request/response types

### 1.3 Database Schema (`migrations/`)
- [x] Users & authentication tables
- [x] Workspaces & membership (with roles)
- [x] Task statuses (kanban columns)
- [x] Tasks table with all fields
- [x] Comments table
- [x] Tags table
- [x] Documents table (ltree for hierarchy)
- [x] Full-text search indexes (pg_trgm)
- [x] Integration settings tables

### 1.4 Basic Backend (`crates/todo-server/`)
- [x] Axum app skeleton with config
- [x] Database connection pool (SQLx)
- [x] Error handling infrastructure
- [x] Auth endpoints (register, login, JWT)
- [x] Auth middleware
- [x] Refresh token handling

### 1.5 Basic TUI (`crates/todo-tui/`)
- [x] Terminal setup/teardown
- [x] Basic app state structure
- [x] Event handling loop
- [x] Basic UI rendering
- [x] Login view
- [x] API client skeleton

---

## Phase 2: Core Task Management

### 2.1 Task API
- [x] GET /api/v1/workspaces/{wid}/tasks - List tasks
- [x] POST /api/v1/workspaces/{wid}/tasks - Create task
- [x] GET /api/v1/workspaces/{wid}/tasks/{id} - Get task
- [x] PUT /api/v1/workspaces/{wid}/tasks/{id} - Update task
- [x] DELETE /api/v1/workspaces/{wid}/tasks/{id} - Delete task
- [x] PUT /api/v1/workspaces/{wid}/tasks/{id}/move - Move task
- [x] Status CRUD endpoints

### 2.2 Kanban TUI
- [x] KanbanBoard component with columns
- [x] Column component
- [x] TaskCard component (with priority indicator and due date)
- [x] Vim navigation (h/j/k/l)
- [x] Task selection highlighting
- [x] Move task between columns (m + h/l)
- [x] Create task (n key)
- [x] Delete task (d key)
- [x] Scroll within columns (with scroll indicators)

### 2.3 Task Detail
- [x] TaskDetail side panel
- [x] Title/description editor
- [x] Priority selector (lowest/low/medium/high/highest)
- [x] Due date picker
- [x] Time estimate input
- [x] Assignee selector (h/l to cycle through workspace members)
- [x] Default assignee on task creation (workspace setting override)

### 2.4 Comments
- [x] GET /api/v1/workspaces/{wid}/tasks/{id}/comments
- [x] POST /api/v1/workspaces/{wid}/tasks/{id}/comments
- [x] PUT /api/v1/workspaces/{wid}/tasks/{id}/comments/{cid}
- [x] DELETE /api/v1/workspaces/{wid}/tasks/{id}/comments/{cid}
- [x] Comments list in task detail TUI
- [x] Add comment with timestamp

---

## Phase 3: Search & Filtering

### 3.1 Search Backend
- [x] Full-text search with PostgreSQL tsvector
- [x] Trigram fuzzy search (ripgrep-like speed)
- [x] GET /api/v1/workspaces/{wid}/search?q=...
- [x] Search pagination

### 3.2 Search TUI
- [x] Search panel (/ command)
- [x] Real-time search results
- [x] Navigate and select results
- [x] Highlight matches

### 3.3 Filtering & Ordering
- [x] Filter by: status, priority, assignee, due date, tags
- [x] Order by: title, due_date, priority, created_at, position
- [x] Filter bar in kanban view
- [x] Command mode filters (`:filter priority=high`)
- [x] Save filter presets

---

## Phase 4: Workspaces & Multi-user

### 4.1 Workspace API
- [x] GET /api/v1/workspaces - List user's workspaces
- [x] POST /api/v1/workspaces - Create workspace
- [x] GET /api/v1/workspaces/{id} - Get workspace
- [x] PUT /api/v1/workspaces/{id} - Update workspace
- [x] DELETE /api/v1/workspaces/{id} - Delete workspace
- [x] Role-based permissions middleware

### 4.2 Members & Invitations
- [x] GET /api/v1/workspaces/{id}/members
- [ ] POST /api/v1/workspaces/{id}/invites - Send invite
- [ ] GET /api/v1/invites/{token} - Get invite details
- [ ] POST /api/v1/invites/{token}/accept - Accept invite
- [ ] PUT /api/v1/workspaces/{id}/members/{uid} - Update role
- [ ] DELETE /api/v1/workspaces/{id}/members/{uid} - Remove member

### 4.3 Workspace TUI
- [ ] Workspace switcher (Ctrl+W)
- [ ] Member list view
- [ ] Role management UI (for admins/owners)
- [ ] Invite flow

---

## Phase 5: Knowledge Base

### 5.1 Document Storage
- [ ] Document schema with ltree (hierarchical paths)
- [ ] GET /api/v1/workspaces/{wid}/documents - List tree
- [ ] POST /api/v1/workspaces/{wid}/documents - Create document
- [ ] GET /api/v1/workspaces/{wid}/documents/{id} - Get document
- [ ] PUT /api/v1/workspaces/{wid}/documents/{id} - Update document
- [ ] DELETE /api/v1/workspaces/{wid}/documents/{id} - Delete (cascade)
- [ ] PUT /api/v1/workspaces/{wid}/documents/{id}/move - Move in tree

### 5.2 Knowledge Base TUI
- [ ] DocumentTree component (collapsible, Tab to expand)
- [ ] DocumentViewer (markdown rendering)
- [ ] DocumentEditor
- [ ] Navigate tree with j/k, expand/collapse with Tab
- [ ] Create/delete documents (Ctrl+N, Ctrl+D)

### 5.3 Task-Document Linking
- [ ] POST /api/v1/workspaces/{wid}/documents/{id}/tasks - Link task
- [ ] DELETE /api/v1/workspaces/{wid}/documents/{id}/tasks/{tid} - Unlink
- [ ] Show linked documents in task detail
- [ ] Show linked tasks in document view

### 5.4 Document Search
- [ ] Full-text search for documents
- [ ] Unified search (tasks + documents)

---

## Phase 6: Integrations

### 6.1 YouTrack Sync
- [ ] YouTrack API client (REST)
- [ ] Field mapping configuration
- [ ] POST /api/v1/workspaces/{wid}/integrations - Configure
- [ ] POST /api/v1/workspaces/{wid}/integrations/youtrack/sync - Trigger sync
- [ ] Bidirectional sync engine
- [ ] Conflict resolution (last-write-wins)
- [ ] Background sync job

### 6.2 GitHub/GitLab
- [ ] POST /api/v1/webhooks/github - Webhook receiver
- [ ] POST /api/v1/webhooks/gitlab - Webhook receiver
- [ ] Signature verification
- [ ] Extract task reference from PR title/branch (e.g., TODO-123)
- [ ] Auto-update task status:
  - PR opened → "In Review"
  - PR merged → "Done"
- [ ] Store PR link in task external_refs

### 6.3 Telegram Notifications
- [ ] Telegram bot setup (teloxide)
- [ ] /start command - generate link token
- [ ] Link Telegram chat to user account
- [ ] PUT /api/v1/workspaces/{wid}/notifications/settings
- [ ] Notification triggers:
  - Task assigned
  - Task updated
  - New comment
  - Due date approaching (1 day before)
- [ ] Background notification job

### 6.4 Claude Code (MCP Research)
- [ ] Research Model Context Protocol compatibility
- [ ] Evaluate if MCP tools can expose task data
- [ ] Prototype if viable
- [ ] Document findings

---

## Phase 7: Polish

### 7.1 UX Improvements
- [ ] Command palette (Ctrl+P)
- [ ] Help overlay (?)
- [ ] Keyboard shortcut reference
- [ ] Status bar (sync status, workspace, user)
- [ ] Notification toasts
- [ ] Undo/redo for task changes
- [ ] Offline mode (queue changes)

### 7.2 Performance
- [ ] Database query optimization
- [ ] Add indexes for common queries
- [ ] Lazy loading for large task lists
- [ ] Connection pooling tuning
- [ ] TUI rendering optimization

### 7.3 Deployment
- [ ] Deployment scripts for VPS
- [ ] Systemd service files
- [ ] Nginx reverse proxy config
- [ ] TLS setup (Let's Encrypt / certbot)
- [ ] Database backup scripts

---

## Vim Keybindings

```
Normal Mode:
  h/l       - Navigate columns
  j/k       - Navigate tasks
  gg/G      - First/last task
  Enter     - Open task detail
  i         - Insert mode (edit)
  a         - Add comment
  n         - New task
  d         - Delete task
  m + h/l   - Move task to column
  Space     - Toggle done
  /         - Search
  :         - Command mode
  Ctrl+P    - Command palette
  Ctrl+W    - Workspace switcher
  Ctrl+K    - Knowledge base
  ?         - Help
  q         - Quit/close

Insert Mode:
  Esc       - Return to normal
  Ctrl+Enter- Save and exit
  Tab       - Next field
  Shift+Tab - Previous field

Command Mode:
  :w        - Save
  :q        - Quit
  :new      - New task
  :filter   - Apply filter
  :sync     - Trigger sync
  :help     - Show help
```

---

## Database Schema Overview

```sql
-- Core entities
users (id, email, password_hash, display_name, created_at)
workspaces (id, name, slug, owner_id, settings jsonb)
workspace_members (workspace_id, user_id, role)

-- Tasks
task_statuses (id, workspace_id, name, slug, color, position, is_done)
tasks (id, workspace_id, status_id, title, description, priority,
       due_date, time_estimate_minutes, position, created_by, assigned_to,
       external_refs jsonb)
task_comments (id, task_id, user_id, content, created_at)
tags (id, workspace_id, name, color)
task_tags (task_id, tag_id)

-- Knowledge base
documents (id, workspace_id, path ltree, parent_id, title, slug, content)
task_document_links (task_id, document_id)

-- Integrations
workspace_integrations (id, workspace_id, integration_type, config, enabled)
user_notification_settings (user_id, workspace_id, telegram_chat_id, ...)
```

---

## API Endpoints Summary

### Auth
```
POST   /api/v1/auth/register
POST   /api/v1/auth/login
POST   /api/v1/auth/refresh
POST   /api/v1/auth/logout
GET    /api/v1/auth/me
```

### Workspaces
```
GET    /api/v1/workspaces
POST   /api/v1/workspaces
GET    /api/v1/workspaces/{id}
PUT    /api/v1/workspaces/{id}
DELETE /api/v1/workspaces/{id}
GET    /api/v1/workspaces/{id}/members
POST   /api/v1/workspaces/{id}/invites
```

### Statuses
```
GET    /api/v1/workspaces/{wid}/statuses
POST   /api/v1/workspaces/{wid}/statuses
PUT    /api/v1/workspaces/{wid}/statuses/{id}
DELETE /api/v1/workspaces/{wid}/statuses/{id}
PUT    /api/v1/workspaces/{wid}/statuses/reorder
```

### Tasks
```
GET    /api/v1/workspaces/{wid}/tasks
POST   /api/v1/workspaces/{wid}/tasks
GET    /api/v1/workspaces/{wid}/tasks/{id}
PUT    /api/v1/workspaces/{wid}/tasks/{id}
DELETE /api/v1/workspaces/{wid}/tasks/{id}
PUT    /api/v1/workspaces/{wid}/tasks/{id}/move
```

### Comments
```
GET    /api/v1/workspaces/{wid}/tasks/{id}/comments
POST   /api/v1/workspaces/{wid}/tasks/{id}/comments
PUT    /api/v1/workspaces/{wid}/tasks/{id}/comments/{cid}
DELETE /api/v1/workspaces/{wid}/tasks/{id}/comments/{cid}
```

### Documents
```
GET    /api/v1/workspaces/{wid}/documents
POST   /api/v1/workspaces/{wid}/documents
GET    /api/v1/workspaces/{wid}/documents/{id}
PUT    /api/v1/workspaces/{wid}/documents/{id}
DELETE /api/v1/workspaces/{wid}/documents/{id}
```

### Search
```
GET    /api/v1/workspaces/{wid}/search?q=...&type=task|document|all
```

### Integrations
```
GET    /api/v1/workspaces/{wid}/integrations
POST   /api/v1/workspaces/{wid}/integrations
POST   /api/v1/workspaces/{wid}/integrations/{type}/sync
POST   /api/v1/webhooks/github
POST   /api/v1/webhooks/gitlab
```
