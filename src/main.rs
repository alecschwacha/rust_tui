use ratatui::{layout::Alignment, widgets::Block, DefaultTerminal, Frame};

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    ratatui::run(app)?;
    Ok(())
}

fn app(terminal: &mut DefaultTerminal) -> std::io::Result<()> {
    loop {
        terminal.draw(render)?;
        if crossterm::event::read()?.is_key_press() {
            break Ok(());
        }
    }
}

fn render(frame: &mut Frame) {
    let area = frame.area();
    let block = Block::default()
        .title_top("Hello, Ratatui!")
        .title_alignment(Alignment::Center)
        .borders(ratatui::widgets::Borders::ALL);
    frame.render_widget(block, area);
}
