# TODO TUI

A terminal-based TODO application with kanban boards, full-text search, and multi-user workspaces. Built with Rust for performance and reliability.

## Features

- **Kanban Board** - Organize tasks in customizable columns with drag-and-drop style movement
- **Vim-style Navigation** - Efficient keyboard-driven workflow with familiar keybindings
- **Full-text Search** - PostgreSQL-powered search with fuzzy matching support
- **Multi-user Workspaces** - Role-based access control (owner, admin, editor, reader)
- **Workspace Invitations** - Invite members via token, manage roles
- **Task Management** - Priority levels, due dates, time estimates, and assignees
- **Tags** - Color-coded labels for task organization
- **Comments** - Threaded discussions on tasks with author attribution
- **Filtering & Sorting** - Filter by priority, tags, assignee, due date; save presets
- **Knowledge Base** - Hierarchical document tree for notes and documentation
- **Email Verification** - Secure account activation with one-time codes
- **Self-hosted** - Run on your own infrastructure

## Tech Stack

| Component | Technologies |
|-----------|-------------|
| **TUI Client** | Rust, ratatui, crossterm |
| **Backend** | Rust, Axum, SQLx |
| **Database** | PostgreSQL |
| **Auth** | JWT with refresh tokens, Argon2 |

## Project Structure

```
todo-tui/
├── crates/
│   ├── todo-shared/   # Shared types and API models
│   ├── todo-server/   # REST API backend
│   └── todo-tui/      # Terminal UI client
├── migrations/        # PostgreSQL migrations
└── docs/              # Documentation
```

## Getting Started

### Prerequisites

- Rust 1.75+
- PostgreSQL 12+
- SQLx CLI (`cargo install sqlx-cli`)

### Setup

1. Clone and configure:
```bash
git clone <repo-url>
cd todo-tui
cp .env.example .env
# Edit .env with your database credentials
```

2. Setup database:
```bash
createdb todo_tui
sqlx migrate run
```

3. Run the server:
```bash
cargo run -p todo-server
```

4. Run the TUI client (in another terminal):
```bash
cargo run -p todo-tui
```

### Environment Variables

```bash
# Required
DATABASE_URL=postgres://user:password@localhost:5432/todo_tui
JWT_SECRET=your-secret-key

# Optional
PORT=3000
JWT_EXPIRES_IN=900
REFRESH_TOKEN_EXPIRES_IN=604800
```

## Keyboard Shortcuts

### Navigation
| Key | Action |
|-----|--------|
| `h/l` | Move between columns |
| `j/k` | Move between tasks |
| `Enter` | Open task details |
| `Backspace` | Go back |
| `Ctrl+W` | Workspace switcher |

### Task Actions
| Key | Action |
|-----|--------|
| `n` | New task |
| `d` | Delete task |
| `m` + `h/l` | Move task to another column |
| `e` | Edit task (in detail view) |
| `a` | Add comment (in detail view) |

### Search & Filter
| Key | Action |
|-----|--------|
| `/` | Search |
| `f` | Toggle filter bar |
| `F` | Open filter panel |
| `P` | Filter presets |

### Workspace & Members
| Key | Action |
|-----|--------|
| `M` | Member panel |
| `i` | Invite member (in member panel) |
| `r` | Change role (in member panel) |
| `T` | Tag management |

### Knowledge Base
| Key | Action |
|-----|--------|
| `Ctrl+K` | Open Knowledge Base |
| `j/k` | Navigate documents |
| `l` | Expand document |
| `h` | Collapse document |
| `n` | New document (child if expanded) |
| `e` | Edit document |
| `d` | Delete document |
| `Alt+Enter` | Save (when editing) |

### General
| Key | Action |
|-----|--------|
| `Esc` | Cancel / close |
| `Tab` | Next field / cycle options |
| `q` | Quit |

## CLI Usage

```bash
# Run the TUI
cargo run -p todo-tui

# Accept a workspace invitation
cargo run -p todo-tui -- --accept-invite <TOKEN>

# Show help
cargo run -p todo-tui -- --help
```

## Development Status

See [docs/roadmap.md](docs/roadmap.md) for the full development roadmap.

**Completed:**
- Phase 1: Foundation (auth, database, basic TUI)
- Phase 2: Core Task Management (CRUD, kanban, comments)
- Phase 3: Search & Filtering (FTS, fuzzy matching, filter presets)
- Phase 4: Workspaces & Multi-user (invitations, member management, role-based access)
- Phase 5.1-5.2: Knowledge Base (document storage API, tree navigation TUI)

**Next:**
- Phase 5.3-5.4: Task-document linking, document search
- Phase 6: Integrations (YouTrack, GitHub, Telegram)

## License

MIT
