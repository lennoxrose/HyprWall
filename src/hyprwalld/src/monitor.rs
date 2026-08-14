#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    /// Union of one or more rects: the smallest rect containing all of them.
    pub fn union(rects: &[Rect]) -> Option<Rect> {
        let first = *rects.first()?;
        let (mut min_x, mut min_y) = (first.x, first.y);
        let (mut max_x, mut max_y) = (first.x + first.w, first.y + first.h);
        for r in &rects[1..] {
            min_x = min_x.min(r.x);
            min_y = min_y.min(r.y);
            max_x = max_x.max(r.x + r.w);
            max_y = max_y.max(r.y + r.h);
        }
        Some(Rect { x: min_x, y: min_y, w: max_x - min_x, h: max_y - min_y })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Monitor {
    pub name: String,
    pub logical: Rect,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn union_of_single_rect_is_itself() {
        let r = Rect { x: 0, y: 0, w: 1920, h: 1080 };
        assert_eq!(Rect::union(&[r]), Some(r));
    }

    #[test]
    fn union_of_two_side_by_side_rects() {
        let a = Rect { x: 0, y: 0, w: 1920, h: 1080 };
        let b = Rect { x: 1920, y: 0, w: 1920, h: 1080 };
        assert_eq!(Rect::union(&[a, b]), Some(Rect { x: 0, y: 0, w: 3840, h: 1080 }));
    }

    #[test]
    fn union_of_empty_is_none() {
        assert_eq!(Rect::union(&[]), None);
    }
}
