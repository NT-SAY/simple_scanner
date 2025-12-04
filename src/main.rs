/// Main entry point for the Night Tool application. 
/// This application provides a terminal-based UI for scanning network ports on a specified host.
/// It utilizes asynchronous programming with Tokio for efficient scanning and
/// the Ratatui library for rendering the UI.

mod scanner;
mod services;
mod ui;

use scanner::ScanResult;
use tokio::sync::mpsc;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io;
use std::time::Instant;

#[tokio::main]

async fn main() -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let (tx, rx) = mpsc::channel::<ScanResult>(2048);
    let mut app = ui::App::new(rx);

    let tick_rate = std::time::Duration::from_millis(80);
    let mut last_tick = Instant::now();
    let mut scan_task: Option<tokio::task::JoinHandle<()>> = None;
    let mut scan_started_at: Option<Instant> = None;

    loop {
        terminal.draw(|f| ui::draw(f, &app))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| std::time::Duration::from_secs(0));

        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => {
                        if let Some(handle) = scan_task.take() {
                            handle.abort();
                        }
                        break;
                    }

                    KeyCode::Char('s') | KeyCode::Enter => {
                        let target_host = app.host_input.trim().to_string();
                        let start_port = app.start_port_input.parse::<u16>().unwrap_or(1);
                        let end_port = app.end_port_input.parse::<u16>().unwrap_or(65535);

                        if target_host.is_empty() {
                            app.log_events.push("Host is empty. Enter IP or domain.".to_string());
                            continue;
                        }

                        if scan_task.is_some() {
                            app.log_events.push("Scan already running".to_string());
                            continue;
                        }

                        if start_port == 0 || end_port == 0 || start_port > end_port {
                            app.log_events.push("Invalid port range".to_string());
                            continue;
                        }

                        app.results.clear();
                        app.total_scanned = 0;
                        let tx_clone = tx.clone();
                        let host_for_task = target_host.clone();

                        let handle = tokio::spawn(async move {
                            scanner::scan_range(&host_for_task, start_port, end_port, tx_clone).await;
                        });

                        scan_task = Some(handle);
                        scan_started_at = Some(Instant::now());
                        app.is_scanning = true;
                        app.log_events.push(format!("Scan started: {}:{}-{}", target_host, start_port, end_port));
                    }

                    KeyCode::Char('t') => {
                        let target_host = app.host_input.trim().to_string();

                        if target_host.is_empty() {
                            app.log_events.push("Host is empty. Enter IP or domain.".to_string());
                            continue;
                        }

                        if scan_task.is_some() {
                            app.log_events.push("Scan already running".to_string());
                            continue;
                        }

                        app.results.clear();
                        app.total_scanned = 0;
                        let tx_clone = tx.clone();
                        let host_for_task = target_host.clone();

                        let handle = tokio::spawn(async move {
                            scanner::scan_top_ports(&host_for_task, tx_clone).await;
                        });

                        scan_task = Some(handle);
                        scan_started_at = Some(Instant::now());
                        app.is_scanning = true;
                        app.log_events.push(format!("Top ports scan started for {}", target_host));
                    }

                    KeyCode::Char('c') => {
                        if let Some(handle) = scan_task.take() {
                            handle.abort();
                            app.is_scanning = false;
                            let elapsed = scan_started_at.map(|t| t.elapsed()).unwrap_or_default();
                            app.log_events.push(format!("Scan aborted ({}s)", elapsed.as_secs()));
                            scan_started_at = None;
                        } else {
                            app.log_events.push("No active scan".to_string());
                        }
                    }

                    KeyCode::Tab => {
                        app.input_focus = (app.input_focus + 1) % 3;
                    }

                    KeyCode::Char(c) => {
                        app.handle_char_input(c);
                    }

                    KeyCode::Backspace => {
                        app.handle_backspace();
                    }

                    _ => {}
                }
            }
        }

        while let Ok(result) = app.rx.try_recv() {
            if result.port == 0 && result.status == "DONE" {
                app.is_scanning = false;
                if let Some(t0) = scan_started_at.take() {
                    let elapsed = t0.elapsed();
                    app.log_events.push(format!("Scan finished in {:.2}s", elapsed.as_secs_f64()));
                } else {
                    app.log_events.push("Scan finished".to_string());
                }
                scan_task.take();
            } else {
                app.results.push(result);
                app.total_scanned += 1;
            }
        }

        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}