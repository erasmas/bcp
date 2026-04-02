mod app;
mod auth;
mod bandcamp;
mod cache;
mod config;
mod events;
mod player;
mod ui;

use anyhow::Result;
use clap::{Parser, Subcommand};
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use std::io;
use std::time::Duration;

use app::App;
use events::EventHandler;

#[derive(Parser)]
#[command(name = "bcp", about = "Bandcamp collection player for the terminal")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Log in to Bandcamp (opens browser)
    Login,
    /// Clear stored authentication
    Logout,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Login) => {
            println!("Opening Bandcamp login in your browser...");
            auth::open_login_page()?;
            println!("Log in to Bandcamp, then press Enter here.");
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;

            println!("Extracting cookie from browser...");
            match auth::extract_bandcamp_cookie()? {
                Some(cookie) => {
                    let auth_data = bandcamp::models::AuthData {
                        identity_cookie: cookie,
                        fan_id: None,
                        username: None,
                    };
                    auth::save_auth(&auth_data)?;
                    println!("Authenticated successfully! Run `bcp` to start the player.");
                }
                None => {
                    println!("\nCould not find cookie automatically.");
                    println!("You can paste it manually:");
                    println!("  1. Open your browser's dev tools (F12)");
                    println!("  2. Go to Application/Storage > Cookies > bandcamp.com");
                    println!("  3. Copy the value of the 'identity' cookie");
                    println!("\nPaste cookie value (or press Enter to cancel):");
                    let mut cookie_input = String::new();
                    io::stdin().read_line(&mut cookie_input)?;
                    let cookie_input = cookie_input.trim();
                    if cookie_input.is_empty() {
                        println!("Cancelled.");
                    } else {
                        let auth_data = bandcamp::models::AuthData {
                            identity_cookie: cookie_input.to_string(),
                            fan_id: None,
                            username: None,
                        };
                        auth::save_auth(&auth_data)?;
                        println!("Authenticated! Run `bcp` to start the player.");
                    }
                }
            }
            return Ok(());
        }
        Some(Commands::Logout) => {
            auth::clear_auth()?;
            println!("Logged out. Stored credentials removed.");
            return Ok(());
        }
        None => {}
    }

    // Launch TUI
    run_tui().await
}

async fn run_tui() -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_app(&mut terminal).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

async fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    let mut app = App::new();
    app.init().await?;

    let mut events = EventHandler::new(Duration::from_millis(500));

    // If we have auth, kick off collection loading
    let needs_load = app.screen == app::AppScreen::Loading;

    // Draw initial frame
    terminal.draw(|f| app.draw(f))?;

    if needs_load {
        if let Err(e) = app.load_collection().await {
            app.status_msg = format!("Error: {}", e);
            app.screen = app::AppScreen::Login;
            app.login_step = app::LoginStep::Prompt;
        }
        terminal.draw(|f| app.draw(f))?;
    }

    loop {
        if app.should_quit {
            break;
        }

        if let Some(event) = events.next().await {
            app.handle_event(event).await?;

            // Check if we need to transition to loading
            if app.screen == app::AppScreen::Loading && app.albums.is_empty() {
                terminal.draw(|f| app.draw(f))?;
                if let Err(e) = app.load_collection().await {
                    app.status_msg = format!("Error: {}", e);
                    if app.auth.is_none() {
                        app.screen = app::AppScreen::Login;
                        app.login_step = app::LoginStep::Prompt;
                    }
                }
            }

            if app.dirty {
                terminal.draw(|f| app.draw(f))?;
                app.dirty = false;
            }
        }
    }

    Ok(())
}
