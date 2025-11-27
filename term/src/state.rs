use ratatui::{
    Terminal,
    prelude::Backend,
    style::{Style, Stylize},
    text::Line,
    widgets::{Block, BorderType, Padding, Paragraph, Wrap},
};

use crate::parse::Config;

pub struct TermState<B: Backend> {
    terminal: Terminal<B>,
    config: Config,
}

impl<B: Backend> TermState<B> {
    pub fn new(backend: B) -> Self {
        Self {
            terminal: Terminal::new(backend).unwrap(),
            config: Config::new(),
        }
    }

    pub fn update_state(&mut self, line: &str) {
        self.draw(line);
    }

    fn draw(&mut self, r: &str) {
        self.terminal
            .draw(|frame| {
                let block = Block::bordered()
                    .border_style(Style::new().fg(self.config.border()).bg(self.config.bg()))
                    .bg(self.config.bg())
                    .title_top(
                        Line::from("Terminal")
                            .centered()
                            .bold()
                            .fg(self.config.title()),
                    )
                    .border_type(BorderType::Rounded)
                    .padding(Padding::new(5, 5, 5, 5));
                let paragraph = Paragraph::new(r)
                    .block(block)
                    .wrap(Wrap { trim: true })
                    .fg(self.config.text())
                    .bg(self.config.bg());
                frame.render_widget(paragraph, frame.area())
            })
            .unwrap();
    }
}
