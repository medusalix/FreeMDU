use freemdu::device::{Date, Property, Value};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Color, Style, Stylize},
    symbols::line,
    text::Line,
    widgets::{Block, BorderType, LineGauge, Padding, Paragraph, Widget, Wrap},
};

#[derive(Debug)]
enum Cell {
    Text(String),
    Gauge(String, f64),
}

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
        // Convert properties to cells before rendering to avoid empty rows
        let cells = self
            .data
            .iter()
            .filter_map(|(prop, val)| Self::prop_to_cell(prop, val).map(|cell| (prop, cell)));
        let layout = Layout::horizontal([Constraint::Fill(1); 2]);
        let mut offset = 0;

        for ((prop, cell), row) in cells.zip(area.rows()) {
            let [mut left, mut right] = layout.areas(row);

            left.y += offset;
            right.y += offset;

            // Abort if row exceeds table bounds
            if Self::row_height_out_of_bounds(right, area) {
                break;
            }

            match cell {
                Cell::Text(txt) => {
                    let par = Paragraph::new(txt).wrap(Wrap { trim: false });

                    right.height = par.line_count(right.width) as u16;

                    // Abort if wrapped paragraph exceeds table bounds
                    if Self::row_height_out_of_bounds(right, area) {
                        break;
                    }

                    par.render(right, buf);
                }
                Cell::Gauge(label, ratio) => LineGauge::default()
                    .filled_symbol(line::THICK_HORIZONTAL)
                    .filled_style(self.color)
                    .ratio(ratio)
                    .label(label)
                    .render(right, buf),
            }

            prop.name.bold().render(left, buf);
            offset += right.height - 1;
        }
    }

    fn prop_to_cell(prop: &Property, val: &Value) -> Option<Cell> {
        match val {
            &Value::Bool(val) => {
                if val {
                    Some(Cell::Text("Yes".to_string()))
                } else {
                    Some(Cell::Text("No".to_string()))
                }
            }
            Value::Number(num) => {
                if let Some(unit) = prop.unit {
                    Some(Cell::Text(format!("{num} {unit}")))
                } else {
                    Some(Cell::Text(num.to_string()))
                }
            }
            &Value::Sensor(current, target) => {
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

                Some(Cell::Gauge(txt, ratio))
            }
            Value::String(string) => Some(Cell::Text(string.clone())),
            Value::Duration(dur) => {
                let total_mins = dur.as_secs() / 60;
                let hours = total_mins / 60;
                let mins = total_mins % 60;

                Some(Cell::Text(format!("{hours}h {mins}min")))
            }
            Value::Date(Date { year, month, day }) => {
                Some(Cell::Text(format!("{year}-{month:02}-{day:02}")))
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
            .border_type(BorderType::Rounded)
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
