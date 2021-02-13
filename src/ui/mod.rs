use skulpin::skia_safe::*;

use crate::util::RcFont;

pub mod input;
mod slider;

pub use input::*;
pub use slider::*;

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

pub type Alignment = (AlignH, AlignV);

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
    font: Option<RcFont>,
    font_size: f32,
    font_height_in_pixels: f32
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
            font: None,
            font_size: -1.0, // invalid font size by default to trigger an error in text()
            font_height_in_pixels: 0.0,
        };
        self.group_stack.push(group);
    }

    pub fn push_group(&mut self, size: (f32, f32), layout: Layout) {
        let top_position = Point::new(self.top().rect.left, self.top().rect.top);
        let group = Group {
            rect: Rect::from_point_and_size(top_position + self.top().layout_position, size),
            layout,
            layout_position: Point::new(0.0, 0.0),
            font: self.top().font.clone(),
            .. *self.top()
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

    pub fn space(&mut self, offset: f32) {
        match self.top().layout {
            Layout::Freeform => panic!("only Vertical and Horizontal layouts can be spaced"),
            Layout::Horizontal => self.top_mut().layout_position.x += offset,
            Layout::Vertical => self.top_mut().layout_position.y += offset,
        }
    }

    pub fn fill(&self, canvas: &mut Canvas, color: impl Into<Color4f>) {
        let mut paint = Paint::new(color.into(), None);
        paint.set_anti_alias(false);
        canvas.draw_rect(self.top().rect, &paint);
    }

    fn text_size_impl(&self, text: &str, font: &mut Font) -> Size {
        let original_size = font.size();
        font.set_size(self.top().font_size);
        let (advance, _) = font.measure_str(text, None);
        font.set_size(original_size);
        Size::new(advance, self.top().font_height_in_pixels)
    }

    fn recalculate_font_metrics(&mut self) {
        let font = self.top().font.as_ref()
            .expect("a font must be provided first")
            .borrow()
            .with_size(self.top().font_size)
            .unwrap();
        let (_, metrics) = font.metrics();
        self.top_mut().font_height_in_pixels = metrics.cap_height.abs();
    }

    pub fn set_font(&mut self, new_font: RcFont) {
        self.top_mut().font = Some(new_font);
        if self.top().font_size > 0.0 {
            self.recalculate_font_metrics();
        }
    }

    pub fn set_font_size(&mut self, new_font_size: f32) {
        assert!(new_font_size >= 0.0, "font size must be zero or positive");
        self.top_mut().font_size = new_font_size;
        self.recalculate_font_metrics();
    }

    pub fn text(&self, canvas: &mut Canvas, text: &str, color: impl Into<Color4f>, alignment: Alignment) {
        assert!(self.top().font_size >= 0.0, "font size must be provided");

        // ↓ hell on earth
        let mut font = self.top().font.as_ref()
            .expect("cannot draw text without a font")
            .borrow_mut();
        let original_size = font.size();
        font.set_size(self.top().font_size);

        let rect = self.top().rect;
        let Size { width: text_width, height: text_height } = self.text_size_impl(text, &mut font);
        let x = match alignment.0 {
            AlignH::Left => rect.left,
            AlignH::Center => rect.center_x() - text_width / 2.0,
            AlignH::Right => rect.right - text_width,
        };
        let y = match alignment.1 {
            AlignV::Top => rect.top + text_height,
            AlignV::Middle => rect.center_y() + text_height / 2.0,
            AlignV::Bottom => rect.bottom,
        };

        let mut paint = Paint::new(color.into(), None);
        paint.set_anti_alias(true);
        canvas.draw_str(
            text,
            Point::new(x, y),
            &font,
            &paint,
        );

        font.set_size(original_size);
    }

    pub fn draw_on_canvas(&self, canvas: &mut Canvas, callback: impl FnOnce(&mut Canvas)) {
        let offset = Point::new(self.top().rect.left, self.top().rect.top);
        canvas.save();
        canvas.translate(offset);
        callback(canvas);
        canvas.restore();
    }

    pub fn mouse_position(&self, input: &Input) -> Point {
        input.mouse_position() - self.top().rect.to_quad()[0]
    }

    pub fn has_mouse(&self, input: &Input) -> bool {
        let mouse = self.mouse_position(input);
        let Size { width, height } = self.top().rect.size();
        mouse.x >= 0.0 && mouse.x <= width && mouse.y >= 0.0 && mouse.y <= height
    }

}
