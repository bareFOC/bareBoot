use ratatui::style::Style;
use ratatui::text::Line;

#[derive(Clone)]
pub enum FieldKind {
    Text(String),
    Toggle(bool),
    Number(i64),
    Select(Vec<String>, usize),
}

#[derive(Clone)]
pub struct FormField {
    pub label: String,
    pub kind: FieldKind,
    pub error: Option<String>,
}

impl FormField {
    pub fn text(label: &str, value: &str) -> Self {
        Self { label: label.into(), kind: FieldKind::Text(value.into()), error: None }
    }
    pub fn value_text(&self) -> String {
        match &self.kind {
            FieldKind::Text(s) => s.clone(),
            FieldKind::Toggle(b) => if *b { "on".into() } else { "off".into() },
            FieldKind::Number(n) => n.to_string(),
            FieldKind::Select(opts, i) => opts.get(*i).cloned().unwrap_or_default(),
        }
    }
    pub fn render_line(&self, focused: bool) -> Line<'static> {
        let marker = if focused { "> " } else { "  " };
        let val = self.value_text();
        let err = self.error.clone().unwrap_or_default();
        Line::from(format!("{marker}{}: {}  {}", self.label, val, err))
    }
}

#[derive(Clone)]
pub struct Form {
    pub fields: Vec<FormField>,
    pub focus: usize,
}

impl Form {
    pub fn new(fields: Vec<FormField>) -> Self { Self { fields, focus: 0 } }
    pub fn next(&mut self) { if !self.fields.is_empty() { self.focus = (self.focus + 1) % self.fields.len(); } }
    pub fn prev(&mut self) { if !self.fields.is_empty() { self.focus = if self.focus == 0 { self.fields.len()-1 } else { self.focus-1 }; } }
    pub fn focused_field(&mut self) -> Option<&mut FormField> { self.fields.get_mut(self.focus) }
    pub fn handle_char(&mut self, c: char) {
        if let Some(f) = self.focused_field() {
            match &mut f.kind {
                FieldKind::Text(s) => s.push(c),
                FieldKind::Number(n) => { if c.is_ascii_digit() { *n = *n * 10 + (c as i64 - '0' as i64); } }
                _ => {}
            }
        }
    }
    pub fn handle_backspace(&mut self) {
        if let Some(f) = self.focused_field() {
            match &mut f.kind {
                FieldKind::Text(s) => { s.pop(); }
                FieldKind::Number(n) => { *n /= 10; }
                _ => {}
            }
        }
    }
    pub fn handle_toggle(&mut self) {
        if let Some(f) = self.focused_field() {
            if let FieldKind::Toggle(b) = &mut f.kind { *b = !*b; }
            if let FieldKind::Select(opts, i) = &mut f.kind { if !opts.is_empty() { *i = (*i + 1) % opts.len(); } }
        }
    }
    pub fn render_lines(&self) -> Vec<Line<'static>> {
        self.fields.iter().enumerate().map(|(i, f)| f.render_line(i == self.focus)).collect()
    }
}