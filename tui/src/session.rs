use crate::{
    bar::CommandBar,
    popup::{Popup, State},
    table::PropertyTable,
    worker::{Request, Response},
};
use anyhow::Result;
use freemdu::device::{Action, DeviceKind, PropertyKind, Value};
use ratatui::{
    buffer::Buffer,
    crossterm::event::Event,
    layout::{Constraint, Layout, Position, Rect},
    style::{Color, Stylize},
    text::Line,
    widgets::{Block, Borders, Padding, StatefulWidget, Widget},
};
use tokio::sync::mpsc::UnboundedSender;
use tui_input::Input;

#[derive(Debug)]
pub struct Session {
    software_id: u16,
    kind: DeviceKind,
    tables: Vec<(PropertyKind, PropertyTable)>,
    bar: CommandBar,
    popup: Option<Popup>,
    update_counter: usize,
    tx: UnboundedSender<Request>,
}

impl Session {
    pub fn create(
        software_id: u16,
        kind: DeviceKind,
        actions: &'static [Action],
        tx: UnboundedSender<Request>,
    ) -> Result<Self> {
        let mut sess = Session {
            software_id,
            kind,
            tables: vec![
                (
                    PropertyKind::General,
                    PropertyTable::new("General Information", Color::Green),
                ),
                (
                    PropertyKind::Fault,
                    PropertyTable::new("Fault Information", Color::Red),
                ),
                (
                    PropertyKind::Operation,
                    PropertyTable::new("Operating State", Color::Blue),
                ),
                (
                    PropertyKind::Io,
                    PropertyTable::new("Input/Output State", Color::Magenta),
                ),
            ],
            bar: CommandBar::new(actions),
            popup: None,
            update_counter: 0,
            tx,
        };

        sess.schedule_prop_update()?;

        Ok(sess)
    }

    pub fn handle_event(&mut self, event: &Event) -> Result<bool> {
        if let Some(popup) = &mut self.popup {
            match popup.handle_event(event) {
                State::Dismissed => {
                    self.popup = None;
                }
                State::Confirmed => {
                    if let Popup::TriggerAction(action, input) = popup {
                        // Use input value if action has parameters
                        // Only string parameters are currently supported
                        let param = if action.params.is_some() {
                            Some(Value::String(input.value().trim().to_string()))
                        } else {
                            None
                        };

                        self.tx.send(Request::TriggerAction(action, param))?;
                    }

                    self.popup = None;
                }
                State::Open => {}
            }

            Ok(true)
        } else if let Some(action) = self.bar.event_to_action(event) {
            self.popup = Some(Popup::TriggerAction(action, Input::default()));

            Ok(true)
        } else {
            // Event wasn't handled
            Ok(false)
        }
    }

    pub fn handle_worker_response(&mut self, resp: Response) -> Result<()> {
        match resp {
            Response::DeviceConnected { .. } => {}
            Response::PropertiesQueried(kind, data) => {
                if let Some((_, table)) = self.tables.iter_mut().find(|(k, _)| k == &kind) {
                    table.update(data);
                }

                self.schedule_prop_update()?;
            }
            Response::InvalidActionArgument(action) => {
                self.popup = Some(Popup::InvalidActionArgument(action));
            }
            Response::InvalidActionState(action) => {
                self.popup = Some(Popup::InvalidActionState(action));
            }
        }

        Ok(())
    }

    fn schedule_prop_update(&mut self) -> Result<()> {
        // Select next property kind to update
        let kind = match self.update_counter {
            0 => PropertyKind::General,
            1 => PropertyKind::Fault,
            2 => PropertyKind::Operation,
            3 => PropertyKind::Io,
            cnt if cnt % 90 == 0 => PropertyKind::General,
            cnt if cnt % 30 == 0 => PropertyKind::Fault,
            cnt if cnt % 3 == 0 => PropertyKind::Operation,
            _ => PropertyKind::Io,
        };

        self.tx.send(Request::QueryProperties(kind))?;
        self.update_counter += 1;

        Ok(())
    }

    fn render_tables(&self, area: Rect, buf: &mut Buffer) {
        let [top, bottom] = Layout::vertical(vec![Constraint::Fill(1); 2])
            .spacing(1)
            .areas(area);
        let [top_left, top_right] = Layout::horizontal(vec![Constraint::Fill(1); 2])
            .spacing(2)
            .areas(top);
        let [bottom_left, bottom_right] = Layout::horizontal(vec![Constraint::Fill(1); 2])
            .spacing(2)
            .areas(bottom);
        let areas = [top_left, bottom_left, top_right, bottom_right];

        for ((_, table), inner) in self.tables.iter().zip(areas) {
            table.render(inner, buf);
        }
    }

    fn render_bar(&self, area: Rect, buf: &mut Buffer) {
        let block = Block::bordered()
            .borders(Borders::TOP)
            .padding(Padding::proportional(1))
            .title("Actions ".bold())
            .title(
                Line::from(vec![
                    " ".into(),
                    self.kind.to_string().into(),
                    ", Software ID: ".into(),
                    self.software_id.to_string().into(),
                    " ".into(),
                    self.spinner().green(),
                    " ".into(),
                ])
                .bold()
                .right_aligned(),
            );

        self.bar.render(block.inner(area), buf);
        block.render(area, buf);
    }

    fn spinner(&self) -> String {
        let symbols = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
        let index = self.update_counter % symbols.len();

        symbols[index].to_string()
    }
}

impl StatefulWidget for &Session {
    type State = Option<Position>;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let [top, bottom] = Layout::vertical([Constraint::Fill(1), Constraint::Length(4)])
            .spacing(1)
            .areas(area);

        self.render_tables(top, buf);
        self.render_bar(bottom, buf);

        if let Some(popup) = &self.popup {
            // Pass cursor position state to popup
            popup.render(top, buf, state);
        }
    }
}
