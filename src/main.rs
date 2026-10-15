mod github;
mod tasks;
mod tui;

use std::{path::Path, time::Duration};

use crossterm::event::{self, Event, KeyEventKind};
use github::GithubClient;
use ratatui::DefaultTerminal;
use tui::App;

fn main() -> anyhow::Result<()> {
    let path = tasks::path();
    // A gh failure is shown in the PR panel instead of killing the app.
    let mut app = App::new(tasks::Tasks::load(&path)?, GithubClient::new().my_open_prs());

    let mut terminal = ratatui::init();
    let result = run(&mut terminal, &mut app, &path);
    ratatui::restore();
    result
}

fn run(terminal: &mut DefaultTerminal, app: &mut App, path: &Path) -> anyhow::Result<()> {
    while !app.quit {
        terminal.draw(|frame| tui::render(frame, app))?;

        // ~60fps while the panes slide, otherwise just often enough to keep the clock current.
        let wait = Duration::from_millis(if app.animating() { 16 } else { 250 });
        if event::poll(wait)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    app.handle_key(key);
                }
            }
        }
        app.tick();
        if app.dirty {
            app.tasks.save(path)?;
            app.dirty = false;
        }
    }
    Ok(())
}
