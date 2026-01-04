use std::io;
use std::time::Duration;

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use tokio::sync::mpsc;

mod api;
mod app;
mod ui;

use api::ApiClient;
use app::{App, AppEvent, View};

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    // Get server URL from environment
    let server_url = std::env::var("TODO_SERVER_URL")
        .unwrap_or_else(|_| "http://localhost:3000".to_string());

    // Create API client
    let mut api = ApiClient::new(&server_url);
    let has_tokens = api.load_tokens().unwrap_or(false);

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run app
    let app = App::new(api, has_tokens);
    let res = run_app(&mut terminal, app).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("Error: {:?}", err);
    }

    Ok(())
}

async fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    mut app: App,
) -> Result<()> {
    // Create event channel
    let (tx, mut rx) = mpsc::channel::<AppEvent>(100);

    // Spawn input handler
    let tx_input = tx.clone();
    tokio::spawn(async move {
        loop {
            if event::poll(Duration::from_millis(100)).unwrap_or(false) {
                if let Ok(Event::Key(key)) = event::read() {
                    if key.kind == KeyEventKind::Press {
                        let _ = tx_input.send(AppEvent::Key(key)).await;
                    }
                }
            }
            // Send tick events for UI refresh
            let _ = tx_input.send(AppEvent::Tick).await;
        }
    });

    // Verify tokens on startup if we have them
    if app.view == View::VerifyingAuth {
        let tx_verify = tx.clone();
        tokio::spawn(async move {
            let _ = tx_verify.send(AppEvent::VerifyAuth).await;
        });
    }

    loop {
        terminal.draw(|f| ui::draw(f, &app))?;

        if let Some(event) = rx.recv().await {
            match event {
                AppEvent::Key(key) => {
                    if app.handle_key(key, tx.clone()).await? {
                        return Ok(());
                    }
                }
                AppEvent::Tick => {
                    // Just refresh UI
                }
                AppEvent::VerifyAuth => {
                    app.verify_auth().await;
                }
                AppEvent::AuthSuccess => {
                    app.on_auth_success().await;
                }
                AppEvent::AuthFailed(msg) => {
                    app.on_auth_failed(msg);
                }
                AppEvent::WorkspacesLoaded(workspaces) => {
                    app.on_workspaces_loaded(workspaces);
                }
                AppEvent::WorkspaceDataLoaded { statuses, tasks } => {
                    app.on_workspace_data_loaded(statuses, tasks);
                }
                AppEvent::Error(msg) => {
                    app.set_error(msg);
                }
            }
        }
    }
}
