use skulpin::skia_safe::*;

pub enum AlignH {
    Left,
    Center,
    Right,
}

pub enum AlignV {
    Top,
    Middle,
    Bottom,
}

pub struct Alignment(AlignH, AlignV);

pub enum Layout {
    Freeform,
    Horizontal,
    Vertical,
}

struct Group {
    rect: Rect,
    layout: Layout,
    layout_position: Point,
}

pub struct Ui {
    group_stack: Vec<Group>,
}

impl Ui {

    pub fn new() -> Self {
        Self {
            group_stack: Vec::new(),
        }
    }

    fn top(&self) -> &Group {
        self.group_stack.last().unwrap()
    }

    fn top_mut(&mut self) -> &mut Group {
        self.group_stack.last_mut().unwrap()
    }

    pub fn size(&self) -> (f32, f32) {
        let Size { width, height } = self.top().rect.size();
        (width, height)
    }

    pub fn begin(&mut self, window_size: (f32, f32), layout: Layout) {
        self.group_stack.clear();
        let group = Group {
            rect: Rect::from_point_and_size((0.0, 0.0), window_size),
            layout,
            layout_position: Point::new(0.0, 0.0),
        };
        self.group_stack.push(group);
    }

    pub fn push_group(&mut self, size: (f32, f32), layout: Layout) {
        let group = Group {
            rect: Rect::from_point_and_size(self.top().layout_position, size),
            layout,
            layout_position: self.top().layout_position,
        };
        self.group_stack.push(group);
    }

    pub fn pop_group(&mut self) {
        self.group_stack.pop();
    }

    pub fn pad(&mut self, padding: (f32, f32)) {
        self.top_mut().rect.left += padding.0 / 2.0;
        self.top_mut().rect.right -= padding.0 / 2.0;
        self.top_mut().rect.top += padding.1 / 2.0;
        self.top_mut().rect.bottom -= padding.1 / 2.0;
    }

    pub fn fill(&self, canvas: &mut Canvas, color: Color4f) {
        let mut paint = Paint::new(color, None);
        paint.set_anti_alias(false);
        canvas.draw_rect(self.top().rect, &paint);
    }


}
