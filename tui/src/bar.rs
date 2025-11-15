use freemdu::device::Action;
use ratatui::{
    buffer::Buffer,
    crossterm::event::{Event, KeyCode, KeyEvent},
    layout::Rect,
    style::Stylize,
    text::Line,
    widgets::Widget,
};

// Maximum number of rendered commands, limited by number of usable function keys
const MAX_NUM_COMMANDS: usize = 10;

#[derive(Debug)]
pub struct CommandBar {
    actions: &'static [Action],
}

impl CommandBar {
    pub fn new(actions: &'static [Action]) -> Self {
        Self { actions }
    }

    pub fn event_to_action(&self, event: &Event) -> Option<&'static Action> {
        if let Some(KeyEvent {
            code: KeyCode::F(key),
            ..
        }) = event.as_key_press_event()
        {
            self.actions.get((key - 1) as usize)
        } else {
            None
        }
    }
}

impl Widget for &CommandBar {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let len = MAX_NUM_COMMANDS.min(self.actions.len());
        let spans = self
            .actions
            .iter()
            .take(MAX_NUM_COMMANDS)
            .enumerate()
            .flat_map(|(i, action)| {
                // Map actions to function keys
                let name = action.name.into();
                let key = format!("<F{}>", i + 1).bold();

                if i + 1 == len {
                    [name, " ".into(), key, "".into()]
                } else {
                    [name, " ".into(), key, " | ".into()]
                }
            })
            .collect::<Vec<_>>();

        if spans.is_empty() {
            "No actions available.".render(area, buf);
        } else {
            Line::from(spans).render(area, buf);
        }
    }
}
