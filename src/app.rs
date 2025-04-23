use ratatui::crossterm::event;
use ratatui::crossterm::event::{Event, KeyCode};
use ratatui::{DefaultTerminal, Frame};
use std::time::{Duration, Instant};

#[derive(Default)]
struct App {
    pub page: u32,
    pub per_page: u32,
    pub teb_state: String,
    pub displayed_data: Vec<String>,
}

impl App {
    pub fn run(mut self, mut terminal: DefaultTerminal) -> anyhow::Result<()> {
        let tick_rate = Duration::from_millis(250);
        let mut last_tick = Instant::now();
        loop {
            terminal.draw(|frame| self.draw(frame))?;

            let timeout = tick_rate.saturating_sub(last_tick.elapsed());
            if event::poll(timeout)? {
                if let Event::Key(key) = event::read()? {
                    match key.code {
                        KeyCode::Char('q') => return Ok(()),
                        KeyCode::Down => {
                            todo!()
                        }
                        KeyCode::Up => {
                            todo!()
                        }
                        KeyCode::Left => {
                            todo!()
                        }
                        KeyCode::Right => {
                            todo!()
                        }
                        _ => {}
                    }
                }
            }
            if last_tick.elapsed() >= tick_rate {
                last_tick = Instant::now();
            }
        }
    }

    fn draw(&mut self, frame: &mut Frame) {
        todo!()
    }
}
