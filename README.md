# TODO TUI

A terminal-based TODO application with kanban boards, full-text search, and multi-user workspaces. Built with Rust for performance and reliability.

## Features

- **Kanban Board** - Organize tasks in customizable columns with drag-and-drop style movement
- **Vim-style Navigation** - Efficient keyboard-driven workflow with familiar keybindings
- **Full-text Search** - PostgreSQL-powered search with fuzzy matching support
- **Multi-user Workspaces** - Role-based access control (owner, admin, editor, reader)
- **Task Management** - Priority levels, due dates, time estimates, and assignees
- **Comments** - Threaded discussions on tasks with author attribution
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

### Actions
| Key | Action |
|-----|--------|
| `n` | New task |
| `d` | Delete task |
| `m` + `h/l` | Move task to another column |
| `e` | Edit task (in detail view) |
| `a` | Add comment (in detail view) |
| `/` | Search |
| `Ctrl+F` | Toggle fuzzy search |
| `q` | Quit |

### Modes
| Key | Action |
|-----|--------|
| `i` | Enter insert mode |
| `Esc` | Return to normal mode |
| `Tab` | Next field |

## Development Status

See [docs/roadmap.md](docs/roadmap.md) for the full development roadmap.

**Completed:**
- Phase 1: Foundation (auth, database, basic TUI)
- Phase 2: Core Task Management (CRUD, kanban, comments)
- Phase 3: Search (FTS, fuzzy, search UI), Filtering, Ordering
- Phase 4.0: User enhancements (username, email verification, comment author display)

**In Progress:**
- Phase 4: Member invitations

## License

MIT
