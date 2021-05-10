use std::io::{BufReader, stdout};
use std::error::Error;
use std::fs::File;
use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};
use std::thread;
use crossterm::{
    event::{self, Event as CEvent, KeyCode},
    terminal::{disable_raw_mode, enable_raw_mode}
};
use tui::{
    Terminal,
    backend::CrosstermBackend,
    widgets::{Block, Borders, Paragraph},
    layout::{Layout, Constraint, Direction},
    style::{Modifier, Style},
    text:: {Span, Spans}
};
use serde::{Serialize, Deserialize};
enum Event<T> {
    Input(T),
    Tick,
}

#[derive(Serialize, Deserialize)]
struct Milestone {
    name: String,
    hours: i32,
    minutes: i32,
    seconds: i32,
    millis: i32
}

fn main() -> Result<(), Box<dyn Error>> {
    let stdout = stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    terminal.clear()?;

    // Channel for keyboard inputs
    enable_raw_mode().expect("can run in raw mode");

    let (tx, rx) = mpsc::channel();
    let tick_rate = Duration::from_millis(2);
    thread::spawn(move || {
        let mut last_tick = Instant::now();
        loop {
            let timeout = tick_rate
                .checked_sub(last_tick.elapsed())
                .unwrap_or_else(|| Duration::from_millis(0));

            if event::poll(timeout).expect("poll works") {
                if let CEvent::Key(key) = event::read().expect("can read events") {
                    tx.send(Event::Input(key)).expect("can send events");
                }
            }

            if last_tick.elapsed() >= tick_rate {
                if let Ok(_) = tx.send(Event::Tick) {
                    last_tick = Instant::now();
                }
            }
        }
    });

    let mut now = Instant::now();
    let mut hours = 0;
    let mut minutes = 0;
    let mut seconds = 0;
    let mut milliseconds = 0;

    loop {
        let (h, m, s, ms) = parse_millis(now.elapsed());
        hours = h;
        minutes = m;
        seconds = s;
        milliseconds = ms

        terminal.draw(|f| {
            // Split window (TOP - BOTTOM)
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(30),
                    Constraint::Percentage(70)
                ].as_ref())
                .split(f.size());
                
            let left_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Percentage(10),
                    Constraint::Percentage(50),
                    Constraint::Percentage(40),
                ].as_ref())
                .split(chunks[0]);

            let create_block = |title| {
                Block::default()
                    .borders(Borders::ALL)
                    .title(Span::styled(format!(" {} ", title), Style::default().add_modifier(Modifier::BOLD)))
            };

            // Timer pane
            let timer = Paragraph::new(
                    Spans::from(format!("{}:{}:{}.{}",
                            format_tens(hours),
                            format_tens(minutes),
                            format_tens(seconds),
                            format_hundreds(milliseconds))))
                    .block(create_block("time"));

            f.render_widget(timer, left_chunks[0]);

            // Hotlap instructions pane
            let hotlap_text = vec![
                Spans::from("<space>: start/next"),
                Spans::from("r: reset"),
                Spans::from("q: quit")
            ];

            let hotlap = Paragraph::new(hotlap_text.clone())
                .block(create_block("hotlap"));

            f.render_widget(hotlap, left_chunks[1]);

            // Mileshtones
            let milestones = Paragraph::new(Spans::from("No data"))
                .block(create_block("milestones"));

            f.render_widget(milestones, chunks[1]);
        })?;

        match rx.recv()? {
            Event::Input(event) => match event.code {
                KeyCode::Char('q') => {
                    paused = true;
                    disable_raw_mode()?;
                    break;
                },
                KeyCode::Char(' ') => {
                    paused = !paused;
                }
                _ => {}
            },
            Event::Tick => {}
        }
    };
    Ok(())
}

fn parse_millis(duration:Duration) -> (i32, i32, i32, i32) {
    let millis = duration.as_millis();
    let hours = (millis / (1000*60*60)) % 24;
    let minutes = (millis / (1000 * 60)) % 60;
    let seconds = (millis / 1000) % 60;
    let millis = millis % 1000 ;
    
    (hours as i32,minutes as i32,seconds as i32,millis as i32)
}

fn format_tens(digit: i32) -> String {
    if digit < 10 {
        format!("0{}", digit)
    }
    else {
        format!("{}", digit)
    }
}

fn format_hundreds(digit: i32) -> String {
    if digit < 10 {
        format!("00{}", digit)
    }
    else if digit < 100 {
        format!("0{}", digit)
    }
    else {
        format!("{}", digit)
    }
}

fn load_json<T: AsRef<Path>>(path: T) -> Result<Milestone, Box<dyn Error>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let milestones = serde_json::from_reader(reader)?;
    Ok(milestones)
}
