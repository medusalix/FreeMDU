use freemdu::device::{Property, Value};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Color, Style, Stylize},
    symbols::line,
    text::Line,
    widgets::{Block, BorderType, LineGauge, Padding, Paragraph, Widget, Wrap},
};

#[derive(Debug)]
pub struct PropertyTable {
    title: &'static str,
    color: Color,
    data: Vec<(&'static Property, Value)>,
}

impl PropertyTable {
    pub fn new(title: &'static str, color: Color) -> Self {
        Self {
            title,
            color,
            data: Vec::new(),
        }
    }

    pub fn update(&mut self, data: Vec<(&'static Property, Value)>) {
        self.data = data;
    }

    fn render_rows(&self, area: Rect, buf: &mut Buffer) {
        let mut offset = 0;

        for ((prop, val), row) in self.data.iter().zip(area.rows().take(self.data.len())) {
            let [mut left, mut right] = Layout::horizontal([Constraint::Fill(1); 2]).areas(row);

            left.y += offset;
            right.y += offset;

            // Abort if row exceeds table bounds
            if PropertyTable::row_height_out_of_bounds(right, area) {
                break;
            }

            match PropertyTable::format_prop(prop, val) {
                (text, None) => {
                    let par = Paragraph::new(text).wrap(Wrap { trim: false });

                    right.height = par.line_count(right.width) as u16;

                    // Abort if wrapped paragraph exceeds table bounds
                    if PropertyTable::row_height_out_of_bounds(right, area) {
                        break;
                    }

                    par.render(right, buf);
                }
                (text, Some(ratio)) => LineGauge::default()
                    .filled_symbol(line::THICK_HORIZONTAL)
                    .filled_style(self.color)
                    .ratio(ratio)
                    .label(text)
                    .render(right, buf),
            }

            prop.name.bold().render(left, buf);
            offset += right.height - 1;
        }
    }

    fn format_prop(prop: &Property, val: &Value) -> (String, Option<f64>) {
        match *val {
            Value::Bool(val) => {
                if val {
                    ("Yes".to_string(), None)
                } else {
                    ("No".to_string(), None)
                }
            }
            Value::Number(num) => {
                if let Some(unit) = prop.unit {
                    (format!("{num} {unit}"), None)
                } else {
                    (num.to_string(), None)
                }
            }
            Value::Sensor(current, target) => {
                let txt = if let Some(unit) = prop.unit {
                    format!("{current} / {target} {unit}")
                } else {
                    format!("{current} / {target}")
                };

                let ratio = if target > 0 {
                    (f64::from(current) / f64::from(target)).clamp(0.0, 1.0)
                } else {
                    0.0
                };

                (txt, Some(ratio))
            }
            Value::String(ref string) => (string.clone(), None),
            Value::Duration(dur) => {
                let total_mins = dur.as_secs() / 60;
                let hours = total_mins / 60;
                let mins = total_mins % 60;

                (format!("{hours}h {mins}min"), None)
            }
        }
    }

    fn row_height_out_of_bounds(row: Rect, area: Rect) -> bool {
        row.y + row.height > area.y + area.height
    }
}

impl Widget for &PropertyTable {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::bordered()
            .border_type(BorderType::Thick)
            .border_style(self.color)
            .padding(Padding::proportional(1))
            .title(Line::from(vec![" ".into(), self.title.bold(), " ".into()]).centered())
            .title_style(Style::reset());
        let inner = block.inner(area);

        if self.data.is_empty() {
            "No properties available.".bold().render(inner, buf);
        } else {
            self.render_rows(inner, buf);
        }

        block.render(area, buf);
    }
}
