/// UI module for the Night Tool application.
/// It defines the application state and rendering logic using the Ratatui library.
/// # Imports
/// - `crate::scanner::ScanResult` - Struct representing the result of a port scan.
/// - `ratatui` - Library for building terminal user interfaces.
/// - `tokio::sync::mpsc` - Tokio's multi-producer, single-consumer channel for asynchronous communication.
/// - `std::time::Instant` - Standard library time utility for measuring elapsed time.
/// # Structs
/// - `App` - Struct representing the application state, including user inputs, scan results, log events, and scanning status.
/// # Functions
/// - `draw(f: &mut Frame, app: &App)` - Renders the entire UI based on the current application state.
/// - `draw_top_bar(f: &mut Frame, area: Rect, app: &App)` - Renders the top bar of the UI displaying target info and status.
/// - `draw_main(f: &mut Frame, area: Rect, app: &App)` - Renders the main area of the UI showing scan results and details.
/// - `draw_bottom_bar(f: &mut Frame, area: Rect, app: &App)` - Renders the bottom bar of the UI with control instructions.
/// # Examples
/// ```no_run
/// use ratatui::Terminal;
/// use crate::ui::{App, draw};
/// let mut app = App::new(rx);
/// terminal.draw(|f| draw(f, &app))?;
/// ```

use crate::scanner::ScanResult;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph, Row, Table},
    Frame,
};
use tokio::sync::mpsc;
use std::time::Instant;

pub struct App {
    pub host_input: String,
    pub start_port_input: String,
    pub end_port_input: String,
    pub results: Vec<ScanResult>,
    pub log_events: Vec<String>,
    pub is_scanning: bool,
    pub input_focus: usize,
    pub rx: mpsc::Receiver<ScanResult>,
    pub total_scanned: usize,
    pub started_at: Option<Instant>,
}

impl App {
    pub fn new(rx: mpsc::Receiver<ScanResult>) -> Self {
        Self {
            host_input: "".to_string(),
            start_port_input: "1".to_string(),
            end_port_input: "1000".to_string(),
            results: Vec::new(),
            log_events: Vec::new(),
            is_scanning: false,
            input_focus: 0,
            rx,
            total_scanned: 0,
            started_at: None,
        }
    }

    pub fn handle_char_input(&mut self, c: char) {
        match self.input_focus {
            0 => self.host_input.push(c),
            1 => self.start_port_input.push(c),
            2 => self.end_port_input.push(c),
            _ => {}
        }
    }

    pub fn handle_backspace(&mut self) {
        match self.input_focus {
            0 => { self.host_input.pop(); }
            1 => { self.start_port_input.pop(); }
            2 => { self.end_port_input.pop(); }
            _ => {}
        }
    }
}

pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(6), Constraint::Min(8), Constraint::Length(3)].as_ref())
        .split(f.size());

    draw_top_bar(f, chunks[0], app);
    draw_main(f, chunks[1], app);
    draw_bottom_bar(f, chunks[2], app);
}

fn draw_top_bar(f: &mut Frame, area: Rect, app: &App) {
    let host_display = if app.host_input.is_empty() {
        "Enter IP or domain...".to_string()
    } else {
        app.host_input.clone()
    };

    let left = format!("Target: {}", host_display);
    let mid = if app.is_scanning {
        match app.started_at {
            Some(t0) => format!("Status: LIVE | Elapsed: {:.1}s", t0.elapsed().as_secs_f64()),
            None => "Status: LIVE".to_string(),
        }
    } else {
        "Status: IDLE".to_string()
    };
    let right = format!("Open: {}  Scanned: {}", app.results.iter().filter(|r| r.status=="open").count(), app.total_scanned);

    let row = Layout::default().direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(30), Constraint::Percentage(20)].as_ref())
        .split(area);

    f.render_widget(Paragraph::new(left).block(Block::default().borders(Borders::ALL).title("Target")), row[0]);
    f.render_widget(Paragraph::new(mid).block(Block::default().borders(Borders::ALL).title("Status")), row[1]);
    f.render_widget(Paragraph::new(right).block(Block::default().borders(Borders::ALL).title("Counters")), row[2]);
}

fn draw_main(f: &mut Frame, area: Rect, app: &App) {
    let cols = Layout::default().direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(65), Constraint::Percentage(35)].as_ref())
        .split(area);

    let header = Row::new(vec!["Port", "State", "Service", "Resp(ms)"]).style(Style::default().add_modifier(Modifier::BOLD));
    let rows = app.results.iter().map(|r| {
        let color = match r.status.as_str() {
            "open" => Color::Green,
            "closed" => Color::Gray,
            "timeout" => Color::Yellow,
            _ => Color::White,
        };
        Row::new(vec![
            r.port.to_string(),
            r.status.clone(),
            r.service.clone(),
            r.response_ms.to_string(),
        ]).style(Style::default().fg(color))
    });

    let table = Table::new(rows, [Constraint::Length(8), Constraint::Length(10), Constraint::Length(16), Constraint::Length(10)])
        .header(header)
        .block(Block::default().borders(Borders::ALL).title("Results"));

    f.render_widget(table, cols[0]);

    let mut detail = String::new();
    if let Some(r) = app.results.last() {
        detail.push_str(&format!("Port: {}\nState: {}\nService: {}\nResp: {}ms\n\n", r.port, r.status, r.service, r.response_ms));
        if let Some(b) = &r.banner {
            detail.push_str(&format!("Banner:\n{}\n", b));
        }
    } else {
        detail.push_str("No selection\n");
    }

    let log_text = if app.log_events.is_empty() {
        "No events".to_string()
    } else {
        let start = if app.log_events.len() > 100 { app.log_events.len() - 100 } else { 0 };
        app.log_events[start..].join("\n")
    };

    let right_chunks = Layout::default().direction(Direction::Vertical)
        .constraints([Constraint::Length(8), Constraint::Min(4)].as_ref())
        .split(cols[1]);

    f.render_widget(Paragraph::new(detail).block(Block::default().borders(Borders::ALL).title("Detail")), right_chunks[0]);
    f.render_widget(Paragraph::new(log_text).block(Block::default().borders(Borders::ALL).title("Log")), right_chunks[1]);
}

fn draw_bottom_bar(f: &mut Frame, area: Rect, _app: &App) {
    let chunks = Layout::default().direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(20), Constraint::Percentage(60), Constraint::Percentage(20)].as_ref())
        .split(area);

    f.render_widget(Paragraph::new("F1: Help  S/Enter: Start  T: TopScan  C: Cancel  Q: Quit")
        .block(Block::default().borders(Borders::ALL).title("Controls")), chunks[1]);
}