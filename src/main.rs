use crossterm::{
    event::{self, Event as CEvent, KeyCode},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fs::{File, OpenOptions};
use std::io::{stdout, BufReader, Write};
use std::path::Path;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};
use tui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
    Terminal,
};
enum Event<T> {
    Input(T),
    Tick,
}

#[derive(Serialize, Deserialize)]
struct Time {
    h: i32,
    m: i32,
    s: i32,
    ms: i32,
}

#[derive(Serialize, Deserialize)]
struct Milestone {
    name: String,
    time: Time,
    result: Option<f32>,
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

    let mut start_time: Option<Instant> = None;
    let mut is_started = false;
    let mut milestones: Vec<Milestone> = load_json(Path::new("./target/debug/test.json")).unwrap();
    let mut current_idx = 0;

    loop {
        let mut current_time = Time {
            h: 0,
            m: 0,
            s: 0,
            ms: 0,
        };

        if let Some(start_time) = start_time {
            let (h, m, s, ms) = parse_millis(start_time.elapsed());
            current_time = Time { h, m, s, ms };
        }

        terminal.draw(|f| {
            // Split window (TOP - BOTTOM)
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(30), Constraint::Percentage(70)].as_ref())
                .split(f.size());

            let left_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints(
                    [
                        Constraint::Percentage(10),
                        Constraint::Percentage(50),
                        Constraint::Percentage(40),
                    ]
                    .as_ref(),
                )
                .split(chunks[0]);

            let right_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(100)].as_ref())
                .split(chunks[1]);

            let create_block = |title| {
                Block::default().borders(Borders::ALL).title(Span::styled(
                    format!(" {} ", title),
                    Style::default().add_modifier(Modifier::BOLD),
                ))
            };

            let create_span = |t: &Time| {
                Span::from(format!(
                    "{}:{}:{}.{}",
                    format_tens(t.h),
                    format_tens(t.m),
                    format_tens(t.s),
                    format_hundreds(t.ms)
                ))
            };

            // Timer pane
            let timer = Paragraph::new(create_span(&current_time)).block(create_block("time"));
            f.render_widget(timer, left_chunks[0]);

            // Hotlap instructions pane
            let hotlap_text = vec![
                Spans::from("<space>: start/next"),
                Spans::from("s: save best"),
                Spans::from("r: reset"),
                Spans::from("q: quit"),
            ];

            let hotlap = Paragraph::new(hotlap_text.clone()).block(create_block("hotlap"));

            f.render_widget(hotlap, left_chunks[1]);

            // Mileshtones
            if milestones.len() > 0 {
                let mut rows: Vec<Row> = vec![];

                for m in milestones.iter() {
                    let mut row: Vec<Cell> = vec![];
                    row.push(Cell::from(Span::styled(
                        format!("{}", &m.name),
                        Style::default().add_modifier(Modifier::BOLD | Modifier::ITALIC),
                    )));

                    // +- from last Time
                    match m.result {
                        Some(r) => {
                            let mut style = Style::default();
                            if r > 0.0 {
                                style = Style::default().fg(Color::Red);
                            } else if r < 0.0 {
                                style = Style::default().fg(Color::Green);
                            }

                            row.push(Cell::from(Span::styled(format!("{:.3}", r), style)));
                        }
                        None => row.push(Cell::from("")),
                    };

                    row.push(Cell::from(create_span(&m.time)));

                    rows.push(Row::new(row));
                }

                let table = Table::new(rows).block(create_block("milestones")).widths(&[
                    Constraint::Percentage(50),
                    Constraint::Percentage(15),
                    Constraint::Percentage(35),
                ]);
                f.render_widget(table, right_chunks[0]);
            } else {
                let paragraph =
                    Paragraph::new(Spans::from("No data")).block(create_block("milestones"));

                f.render_widget(paragraph, right_chunks[0]);
            }
        })?;

        match rx.recv()? {
            Event::Input(event) => match event.code {
                KeyCode::Char('q') => {
                    disable_raw_mode()?;
                    break;
                }
                KeyCode::Char(' ') => {
                    if !is_started {
                        is_started = true;
                        current_idx = 0;
                        start_time = Some(Instant::now());
                    } else {
                        let old_milestone = &milestones[current_idx];
                        let duration = ((current_time.h * 60 * 60
                            + current_time.m * 60
                            + current_time.s) as f32
                            + current_time.ms as f32 / 100f32)
                            - ((old_milestone.time.h * 60 * 60
                                + old_milestone.time.m * 60
                                + old_milestone.time.s) as f32
                                + old_milestone.time.ms as f32 / 100f32);
                        let milestone = Milestone {
                            name: String::from(&milestones[current_idx].name),
                            time: current_time,
                            result: Some(duration),
                        };
                        let _ = std::mem::replace(&mut milestones[current_idx], milestone);

                        if current_idx + 1 < milestones.len() {
                            current_idx += 1;
                        } else {
                            is_started = false;
                            start_time = None;
                        }
                    }
                }
                KeyCode::Char('r') => {
                    milestones = load_json(Path::new("")).unwrap();
                    is_started = false;
                    start_time = None;
                }
                KeyCode::Char('s') => {
                    if !is_started {
                        save_json("./target/debug/test.json", &milestones)?;
                    }
                }
                _ => {}
            },
            Event::Tick => {}
        }
    }
    Ok(())
}

fn parse_millis(duration: Duration) -> (i32, i32, i32, i32) {
    let millis = duration.as_millis();
    let hours = (millis / (1000 * 60 * 60)) % 24;
    let minutes = (millis / (1000 * 60)) % 60;
    let seconds = (millis / 1000) % 60;
    let millis = millis % 1000;

    (hours as i32, minutes as i32, seconds as i32, millis as i32)
}

fn format_tens(digit: i32) -> String {
    if digit < 10 {
        format!("0{}", digit)
    } else {
        format!("{}", digit)
    }
}

fn format_hundreds(digit: i32) -> String {
    if digit < 10 {
        format!("00{}", digit)
    } else if digit < 100 {
        format!("0{}", digit)
    } else {
        format!("{}", digit)
    }
}

fn load_json<T: AsRef<Path>>(path: T) -> Result<Vec<Milestone>, Box<dyn Error>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let milestones = serde_json::from_reader(reader)?;
    Ok(milestones)
}

fn save_json<T: AsRef<Path>>(path: T, milestones: &Vec<Milestone>) -> std::io::Result<()> {
    let json = serde_json::to_string(milestones)?;

    let mut f = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)?;
    f.write_all(&json.as_bytes())?;

    Ok(())
}
