use skulpin::app::{InputState, PhysicalPosition};
use skulpin::skia_safe::*;

#[derive(Copy, Clone, Debug)]
pub enum AlignH {
    Left,
    Center,
    Right,
}

#[derive(Copy, Clone, Debug)]
pub enum AlignV {
    Top,
    Middle,
    Bottom,
}

#[derive(Copy, Clone, Debug)]
pub struct Alignment(AlignH, AlignV);

#[derive(Copy, Clone, Debug)]
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

    pub fn width(&self) -> f32 {
        self.size().0
    }

    pub fn height(&self) -> f32 {
        self.size().1
    }

    pub fn remaining_width(&self) -> f32 {
        self.size().0 - self.top().layout_position.x
    }

    pub fn remaining_height(&self) -> f32 {
        self.size().1 - self.top().layout_position.y
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
        let top_position = Point::new(self.top().rect.left, self.top().rect.top);
        let mut group = Group {
            rect: Rect::from_point_and_size(top_position + self.top().layout_position, size),
            layout,
            layout_position: Point::new(0.0, 0.0),
        };
        match self.top().layout {
            Layout::Freeform => (),
            Layout::Horizontal => {
                self.top_mut().layout_position.x += group.rect.width();
            },
            Layout::Vertical => {
                self.top_mut().layout_position.y += group.rect.height();
            },
        }
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

    pub fn draw_on_canvas(&self, canvas: &mut Canvas, callback: impl FnOnce(&mut Canvas)) {
        let offset = Point::new(self.top().rect.left, self.top().rect.top);
        canvas.save();
        canvas.translate(offset);
        callback(canvas);
        canvas.restore();
    }

    pub fn mouse_position(&self, input: &InputState) -> Point {
        let PhysicalPosition { x: xd, y: yd } = input.mouse_position();
        Point::new(xd as f32, yd as f32)
    }

    pub fn has_mouse(&self, input: &InputState) -> bool {
        self.top().rect.contains(self.mouse_position(input))
    }

}
