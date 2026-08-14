use crate::monitor::Rect;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Zone {
    pub id: u64,
    pub monitors: Vec<String>,
    pub path: Option<String>,
    pub bounding_box: Option<Rect>,
}
