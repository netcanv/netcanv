use std::cell::{Ref, RefMut};

use skulpin::skia_safe::*;

use crate::util::RcFont;

pub mod input;
mod expand;
mod slider;
mod textfield;

pub use expand::*;
pub use input::*;
pub use slider::*;
pub use textfield::*;

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

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
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

    pub fn align(&mut self, alignment: Alignment) {
        assert!(self.group_stack.len() >= 2, "at least two groups (parent and child) must be present for alignment");

        let mut iter = self.group_stack.iter_mut();
        let child = iter.next_back().unwrap();
        let parent = iter.next_back().unwrap();
        assert!(parent.layout == Layout::Freeform, "the parent must have Freeform layout");

        let x = match alignment.0 {
            AlignH::Left => parent.rect.left,
            AlignH::Center => parent.rect.center_x() - child.rect.width() / 2.0,
            AlignH::Right => parent.rect.right - child.rect.width(),
        };
        let y = match alignment.1 {
            AlignV::Top => parent.rect.top,
            AlignV::Middle => parent.rect.center_y() - child.rect.height() / 2.0,
            AlignV::Bottom => parent.rect.bottom - child.rect.height(),
        };
        child.rect.set_xywh(x, y, child.rect.width(), child.rect.height());
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

    pub fn clip(&self, canvas: &mut Canvas) {
        canvas.clip_rect(self.top().rect, ClipOp::Intersect, false);
    }

    fn text_size_impl(&self, text: &str, font: &mut Font) -> (f32, f32) {
        let original_size = font.size();
        font.set_size(self.top().font_size);
        let (advance, _) = font.measure_str(text, None);
        font.set_size(original_size);
        (advance, self.top().font_height_in_pixels)
    }

    pub fn font(&self) -> Option<&RcFont> {
        self.top().font.as_ref()
    }

    pub fn font_size(&self) -> f32 {
        self.top().font_size
    }

    fn borrow_font(&self) -> Ref<Font> {
        self.top()
            .font.as_ref()
            .expect("a font must be provided first")
            .borrow()
    }

    fn borrow_font_mut(&self) -> RefMut<Font> {
        self.top()
            .font.as_ref()
            .expect("a font must be provided first")
            .borrow_mut()
    }

    fn recalculate_font_metrics(&mut self) {
        let font = self.borrow_font()
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

    fn text_origin_impl(&self, text: &str, alignment: Alignment, font: &mut Font) -> (Point, f32) {
        let rect = self.top().rect;

        let (text_width, text_height) = self.text_size_impl(text, font);
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
        (Point::new(x, y), text_width)
    }

    pub fn text(&self, canvas: &mut Canvas, text: &str, color: impl Into<Color4f>, alignment: Alignment) -> f32 {
        assert!(self.top().font_size >= 0.0, "font size must be provided");

        let mut font = self.borrow_font_mut();
        let original_size = font.size();
        font.set_size(self.top().font_size);

        let mut paint = Paint::new(color.into(), None);
        let (origin, advance) = self.text_origin_impl(text, alignment, &mut font);
        paint.set_anti_alias(true);
        canvas.draw_str(text, origin, &font, &paint);

        font.set_size(original_size);

        advance
    }

    pub fn text_size(&self, text: &str) -> (f32, f32) {
        self.text_size_impl(text, &mut self.borrow_font_mut())
    }

    pub fn text_origin(&self, text: &str, alignment: Alignment) -> Point {
        self.text_origin_impl(text, alignment, &mut self.borrow_font_mut()).0
    }

    pub fn icon(
        &mut self,
        canvas: &mut Canvas,
        icon: &Image,
        color: impl Into<Color4f>,
        group_size: Option<(f32, f32)>
    ) {
        let group_size = group_size.unwrap_or((icon.width() as f32, icon.height() as f32));
        self.push_group(group_size, Layout::Freeform);

        // probably quite horrible but there aren't that many icons drawn to the screen at once in the first place
        let image_bounds = IRect::new(0, 0, icon.width(), icon.height());
        let color_filter = color_filters::blend(color.into().to_color(), BlendMode::SrcATop).unwrap();
        let filter = image_filters::color_filter(color_filter, None, None).unwrap();
        let colored_icon = icon.new_with_filter(None, &filter, image_bounds, image_bounds).unwrap().0;

        let x = self.top().rect.left + self.width() / 2.0 - icon.width() as f32 / 2.0;
        let y = self.top().rect.top + self.height() / 2.0 - icon.height() as f32 / 2.0;
        canvas.draw_image(colored_icon, (x, y), None);
        self.pop_group();
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

pub trait Focus {
    fn focused(&self) -> bool;
    fn set_focus(&mut self, focused: bool);
}

pub fn chain_focus(input: &Input, fields: &mut [&mut dyn Focus]) {
    if input.key_just_typed(VirtualKeyCode::Tab) {
        let mut had_focus = false;
        for text_field in fields.iter_mut() {
            if had_focus {
                text_field.set_focus(true);
                return
            }
            if text_field.focused() {
                text_field.set_focus(false);
                had_focus = true;
            }
        }
        if !fields.is_empty() {
            fields[0].set_focus(true);
        }
    }
}
