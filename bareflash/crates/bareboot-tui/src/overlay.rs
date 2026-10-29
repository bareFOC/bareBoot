#[derive(Clone)]
pub enum Overlay {
    None,
    Help(String),
    Confirm { message: String },
    ProfilePicker { names: Vec<String>, selected: usize },
}

impl Overlay {
    pub fn is_none(&self) -> bool { matches!(self, Overlay::None) }
}