use freemdu::device::{Action, ActionParameters};
use ratatui::{
    buffer::Buffer,
    crossterm::event::{Event, KeyCode, KeyEvent},
    layout::{Constraint, Layout, Margin, Position, Rect},
    style::Stylize,
    text::Line,
    widgets::{Block, BorderType, Clear, Padding, Paragraph, StatefulWidget, Widget, Wrap},
};
use tui_input::{Input, backend::crossterm::EventHandler};

#[derive(Debug)]
pub enum State {
    Open,
    Confirmed,
    Dismissed,
}

#[derive(Debug)]
pub enum Popup {
    TriggerAction(&'static Action, Input),
    InvalidActionArgument(&'static Action),
    InvalidActionState(&'static Action),
}

impl Popup {
    pub fn handle_event(&mut self, event: &Event) -> State {
        if let Some(KeyEvent { code, .. }) = event.as_key_press_event() {
            match code {
                KeyCode::Enter => {
                    return State::Confirmed;
                }
                KeyCode::Esc => {
                    return State::Dismissed;
                }
                _ => {}
            }
        }

        if let Self::TriggerAction(_, input) = self {
            input.handle_event(event);
        }

        State::Open
    }

    fn render_trigger_action_prompt(
        area: Rect,
        buf: &mut Buffer,
        action: &str,
        params: &ActionParameters,
        input: &Input,
    ) -> Position {
        let hint = match params {
            ActionParameters::Enumeration(vals) => vals.join(", "),
            ActionParameters::Flags(vals) => vals.join(" | "),
        };
        let par = Paragraph::new(vec![
            Line::from(vec![
                "Please specify an argument for the ".into(),
                action.bold(),
                " action.".into(),
            ]),
            Line::default(),
            Line::from(vec!["Possible values: ".into(), hint.bold(), ".".into()]),
        ])
        .wrap(Wrap { trim: false });

        // Split message into multiple lines if too long
        let width = par.line_width().min(area.width.saturating_sub(50) as usize);
        let lines = par.line_count(width as u16);

        let inner = Self::render_popup(area, buf, "Trigger action", width, lines + 2);
        let [top, bottom] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(inner);

        par.render(top, buf);
        input.value().render(bottom, buf);

        (bottom.x + input.visual_cursor() as u16, bottom.y).into()
    }

    fn render_trigger_action(area: Rect, buf: &mut Buffer, action: &str) {
        let msg = Line::from(vec![
            "Press enter to trigger the ".into(),
            action.bold(),
            " action.".into(),
        ]);
        let inner = Self::render_popup(area, buf, "Trigger action", msg.width(), 1);

        msg.render(inner, buf);
    }

    fn render_invalid_action_arg(area: Rect, buf: &mut Buffer, action: &str) {
        let msg = Line::from(vec![
            "The specified argument for the ".into(),
            action.bold(),
            " action is invalid.".into(),
        ]);
        let inner = Self::render_popup(area, buf, "Action failed", msg.width(), 1);

        msg.render(inner, buf);
    }

    fn render_invalid_action_state(area: Rect, buf: &mut Buffer, action: &str) {
        let msg = Line::from(vec![
            "The device is not in a valid state for the ".into(),
            action.bold(),
            " action.".into(),
        ]);
        let inner = Self::render_popup(area, buf, "Action failed", msg.width(), 1);

        msg.render(inner, buf);
    }

    fn render_popup(
        area: Rect,
        buf: &mut Buffer,
        title: &str,
        width: usize,
        height: usize,
    ) -> Rect {
        // Increase size by block padding and border
        let pad = Padding::proportional(1);
        let width = (width as u16) + pad.left + pad.right + 2;
        let height = (height as u16) + pad.top + pad.bottom + 2;
        let popup = area.centered(Constraint::Length(width), Constraint::Length(height));
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .padding(pad)
            .title(Line::from(vec![" ".into(), title.bold(), " ".into()]).centered());
        let inner = block.inner(popup);

        // Clear area around the block with additional margin
        Clear.render(popup.outer(Margin::new(2, 1)), buf);
        block.render(popup, buf);

        inner
    }
}

impl StatefulWidget for &Popup {
    type State = Option<Position>;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        match self {
            Popup::TriggerAction(action, input) => {
                if let Some(params) = &action.params {
                    // Update state with current input prompt cursor position
                    *state = Some(Popup::render_trigger_action_prompt(
                        area,
                        buf,
                        action.name,
                        params,
                        input,
                    ));
                } else {
                    Popup::render_trigger_action(area, buf, action.name);
                }
            }
            Popup::InvalidActionArgument(action) => {
                Popup::render_invalid_action_arg(area, buf, action.name);
            }
            Popup::InvalidActionState(action) => {
                Popup::render_invalid_action_state(area, buf, action.name);
            }
        }
    }
}
