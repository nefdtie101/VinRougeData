use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers, MouseEvent};
use std::time::Duration;

#[derive(Debug, Clone)]
pub enum AppEvent {
    Key(KeyEvent),
    Mouse(MouseEvent),
}

pub struct EventHandler;

impl EventHandler {
    pub fn new() -> Self {
        Self
    }

    pub fn next(&mut self) -> Result<Option<AppEvent>> {
        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) => return Ok(Some(AppEvent::Key(key))),
                Event::Mouse(mouse) => return Ok(Some(AppEvent::Mouse(mouse))),
                _ => {}
            }
        }
        Ok(None)
    }
}

pub fn is_exit_key(key: &KeyEvent) -> bool {
    matches!(
        key,
        KeyEvent {
            code: KeyCode::Char('c'),
            modifiers: KeyModifiers::CONTROL,
            ..
        } | KeyEvent {
            code: KeyCode::Char('q'),
            ..
        }
    )
}
