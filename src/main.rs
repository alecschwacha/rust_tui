mod github;
mod tui;

use crossterm::event::{self, Event, KeyCode};
use github::GithubClient;
use ratatui::DefaultTerminal;
use tui::render;

fn main() -> anyhow::Result<()> {
    let github = GithubClient::new();

    let _prs = github.my_open_prs()?;

    let mut terminal = ratatui::init();

    let result = run(&mut terminal, &_prs);

    result?;

    Ok(())
}

fn run(terminal: &mut DefaultTerminal, prs: &[github::PRRequest]) -> std::io::Result<()> {
    loop {
        terminal.draw(|frame| {
            tui::render(frame, prs);
        })?;

        if let Event::Key(key) = event::read()? {
            if key.code == KeyCode::Char('q') {
                break;
            }
        }
    }
    Ok(())
}
