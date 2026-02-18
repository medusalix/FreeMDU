mod bar;
mod popup;
mod session;
mod table;
mod worker;

use crate::{
    session::Session,
    worker::{Response, Worker},
};
use anyhow::{Context, Result};
use clap::Parser;
use freemdu::serial::{self, Port};
use futures::{StreamExt, future::FutureExt};
use ratatui::{
    DefaultTerminal,
    buffer::Buffer,
    crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyModifiers},
    layout::{Constraint, Flex, Layout, Margin, Position, Rect},
    style::Stylize,
    text::Line,
    widgets::{Block, BorderType, Borders, Padding, StatefulWidget, Widget},
};
use tokio::task::LocalSet;

#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    /// Serial port path (e.g. /dev/ttyACM0)
    serial_port: String,
}

#[derive(Default, Debug)]
struct App {
    session: Option<Session>,
    should_exit: bool,
}

impl App {
    async fn run(&mut self, port: Port, term: &mut DefaultTerminal) -> Result<()> {
        let mut events = EventStream::new();
        let mut rx = Worker::start(port);

        while !self.should_exit {
            // Draw terminal widgets
            term.draw(|frame| {
                let mut cursor_pos = None;

                frame.render_stateful_widget(&*self, frame.area(), &mut cursor_pos);

                if let Some(pos) = cursor_pos {
                    frame.set_cursor_position(pos);
                }
            })?;

            // Handle terminal events and worker responses
            tokio::select! {
                Some(evt) = events.next().fuse() => self
                    .handle_event(&evt?).context("Failed to handle event")?,
                Some(resp) = rx.recv() => self
                    .handle_worker_response(resp)
                    .context("Failed to handle worker response")?,
            }
        }

        Ok(())
    }

    fn handle_event(&mut self, event: &Event) -> Result<()> {
        if let Some(sess) = &mut self.session
            && sess.handle_event(event)?
        {
            // Event was handled by session
            return Ok(());
        }

        if let Some(KeyEvent {
            code, modifiers, ..
        }) = event.as_key_press_event()
        {
            match code {
                KeyCode::Char('q') => self.should_exit = true,
                KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                    self.should_exit = true;
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn handle_worker_response(&mut self, resp: Response) -> Result<()> {
        match resp {
            Response::DeviceConnected {
                software_id,
                kind,
                actions,
                tx,
            } => {
                self.session = Some(Session::create(software_id, kind, actions, tx)?);
            }
            Response::DeviceDisconnected => self.session = None,
            _ => {
                if let Some(sess) = &mut self.session {
                    sess.handle_worker_response(resp)?;
                }
            }
        }

        Ok(())
    }
}

impl StatefulWidget for &App {
    type State = Option<Position>;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let area = area.inner(Margin::new(1, 0));
        let block = Block::bordered()
            .borders(Borders::TOP)
            .border_type(BorderType::Double)
            .padding(Padding::top(1))
            .title(
                Line::from(vec![
                    " ".into(),
                    "FreeMDU TUI ".into(),
                    env!("CARGO_PKG_VERSION").into(),
                    " ".into(),
                ])
                .bold()
                .centered(),
            );
        let inner = block.inner(area);

        if let Some(sess) = &self.session {
            // Session might set cursor position state
            sess.render(inner, buf, state);
        } else {
            let [center] = Layout::vertical([Constraint::Length(1)])
                .flex(Flex::Center)
                .areas(inner);

            "Waiting for device connection..."
                .bold()
                .into_centered_line()
                .render(center, buf);
        }

        block.render(area, buf);
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    env_logger::init();

    let args = Args::parse();
    let port = serial::open(&args.serial_port).context("Failed to open serial port")?;
    let mut term = ratatui::init();
    let res = LocalSet::new()
        .run_until(async move { App::default().run(port, &mut term).await })
        .await;

    ratatui::restore();

    res
}
