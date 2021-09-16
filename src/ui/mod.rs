//! NetCanv's bespoke immediate UI library.

use std::cell::{Ref, RefMut};

use skulpin::skia_safe::*;

use crate::common::RcFont;

mod button;
mod expand;
pub mod input;
mod slider;
mod textfield;

pub use button::*;
pub use expand::*;
pub use input::*;
pub use slider::*;
pub use textfield::*;

/// Horizontal alignment.
#[derive(Copy, Clone, Debug)]
pub enum AlignH {
    Left,
    Center,
    Right,
}

/// Vertical alignment.
#[derive(Copy, Clone, Debug)]
pub enum AlignV {
    Top,
    Middle,
    Bottom,
}

/// A tuple storing both horizontal and vertical alignment.
pub type Alignment = (AlignH, AlignV);

/// A group layout.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Layout {
    Freeform,
    Horizontal,
    HorizontalRev,
    Vertical,
    VerticalRev,
}

/// A group.
struct Group {
    rect: Rect,
    layout: Layout,
    layout_position: Point,
    font: Option<RcFont>,
    font_size: f32,
    font_height_in_pixels: f32,
}

/// UI state.
pub struct Ui {
    group_stack: Vec<Group>,
}

impl Ui {
    /// Creates a new UI state.
    pub fn new() -> Self {
        Self {
            group_stack: Vec::new(),
        }
    }

    /// Returns a reference the group on the very top of the stack.
    fn top(&self) -> &Group {
        self.group_stack.last().unwrap()
    }

    /// Returns a mutable reference to the group on the very top of the stack.
    fn top_mut(&mut self) -> &mut Group {
        self.group_stack.last_mut().unwrap()
    }

    /// Returns the size of the topmost group.
    pub fn size(&self) -> (f32, f32) {
        let Size { width, height } = self.top().rect.size();
        (width, height)
    }

    /// Returns the width of the topmost group.
    pub fn width(&self) -> f32 {
        self.size().0
    }

    /// Returns the height of the topmost group.
    pub fn height(&self) -> f32 {
        self.size().1
    }

    /// Returns the "remaining width" in the group, that is, the maximum width of a group for it to
    /// not overflow the current group's size.
    pub fn remaining_width(&self) -> f32 {
        self.size().0 - self.top().layout_position.x
    }

    /// Returns the "remaining height" in the group, that is, the maximum height of a group for it
    /// to not overflow the current group's size.
    pub fn remaining_height(&self) -> f32 {
        self.size().1 - self.top().layout_position.y
    }

    /// Returns the "remaining size" of the group, as computed by `remaining_width` and
    /// `remaining_height`.
    pub fn remaining_size(&self) -> (f32, f32) {
        (self.remaining_width(), self.remaining_height())
    }

    /// Begins a UI frame.
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

    /// Pushes a new group onto the stack.
    pub fn push_group(&mut self, size: (f32, f32), layout: Layout) {
        let top_rect = self.top().rect;
        let position = match self.top().layout {
            Layout::Freeform | Layout::Horizontal | Layout::Vertical =>
                Point::new(top_rect.left, top_rect.top) + self.top().layout_position,
            Layout::HorizontalRev =>
                Point::new(top_rect.right, top_rect.top) + self.top().layout_position - Point::new(size.0, 0.0),
            Layout::VerticalRev =>
                Point::new(top_rect.left, top_rect.bottom) + self.top().layout_position - Point::new(0.0, size.1),
        };
        let group = Group {
            rect: Rect::from_point_and_size(position, size),
            layout,
            layout_position: Point::new(0.0, 0.0),
            font: self.top().font.clone(),
            ..*self.top()
        };
        self.group_stack.push(group);
    }

    /// Pops the topmost group off of the stack.
    pub fn pop_group(&mut self) {
        let group = self.group_stack.pop().expect("unbalanced group stack");
        match self.top().layout {
            Layout::Freeform => (),
            Layout::Horizontal => {
                self.top_mut().layout_position.x += group.rect.width();
            },
            Layout::HorizontalRev => {
                self.top_mut().layout_position.x -= group.rect.width();
            },
            Layout::Vertical => {
                self.top_mut().layout_position.y += group.rect.height();
            },
            Layout::VerticalRev => {
                self.top_mut().layout_position.y -= group.rect.height();
            },
        }
    }

    /// Aligns the current group's position inside of the parent group.
    pub fn align(&mut self, alignment: Alignment) {
        assert!(
            self.group_stack.len() >= 2,
            "at least two groups (parent and child) must be present for alignment"
        );

        let mut iter = self.group_stack.iter_mut();
        let child = iter.next_back().unwrap();
        let parent = iter.next_back().unwrap();

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

    /// Adds padding across the edges of the topmost group.
    pub fn pad(&mut self, padding: (f32, f32)) {
        self.top_mut().rect.left += padding.0 / 2.0;
        self.top_mut().rect.right -= padding.0 / 2.0;
        self.top_mut().rect.top += padding.1 / 2.0;
        self.top_mut().rect.bottom -= padding.1 / 2.0;
    }

    /// Adds spacing between the last and next element in the group.
    pub fn space(&mut self, offset: f32) {
        match self.top().layout {
            Layout::Freeform => panic!("only Vertical and Horizontal layouts can be spaced"),
            Layout::Horizontal => self.top_mut().layout_position.x += offset,
            Layout::HorizontalRev => self.top_mut().layout_position.x -= offset,
            Layout::Vertical => self.top_mut().layout_position.y += offset,
            Layout::VerticalRev => self.top_mut().layout_position.y -= offset,
        }
    }

    /// Offsets placement of elements in the group.
    pub fn offset(&mut self, vector: impl Into<Vector>) {
        self.top_mut().layout_position.offset(vector.into());
    }

    /// Shrinks the size of the topmost group to fit its children.
    pub fn fit(&mut self) {
        let (x, y) = (self.top().rect.left, self.top().rect.top);
        let (mut width, mut height) = self.size();
        match self.top().layout {
            Layout::Horizontal => width = self.top().layout_position.x,
            Layout::Vertical => height = self.top().layout_position.y,
            _ => panic!("fit can only be used on Horizontal and Vertical layouts"),
        }
        self.top_mut().rect.set_xywh(x, y, width, height);
    }

    /// Fills the topmost group's area with the given color.
    pub fn fill(&self, canvas: &mut Canvas, color: impl Into<Color4f>) {
        let mut paint = Paint::new(color.into(), None);
        paint.set_anti_alias(false);
        canvas.draw_rect(self.top().rect, &paint);
    }

    /// Outlines the topmost group's area with the given color and stroke thickness.
    pub fn outline(&self, canvas: &mut Canvas, color: impl Into<Color4f>, thickness: f32) {
        let mut paint = Paint::new(color.into(), None);
        paint.set_anti_alias(false);
        paint.set_style(paint::Style::Stroke);
        paint.set_stroke_width(thickness);
        let mut rect = self.top().rect;
        rect.left += 1.0;
        rect.top += 1.0;
        canvas.draw_rect(rect, &paint);
    }

    /// Clips rendering in the canvas to the area of the topmost group.
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

    /// Returns the font used by the current group.
    pub fn font(&self) -> Option<&RcFont> {
        self.top().font.as_ref()
    }

    /// Returns the size of the font used by the current group.
    pub fn font_size(&self) -> f32 {
        self.top().font_size
    }

    fn borrow_font(&self) -> Ref<Font> {
        self.top()
            .font
            .as_ref()
            .expect("a font must be provided first")
            .borrow()
    }

    fn borrow_font_mut(&self) -> RefMut<Font> {
        self.top()
            .font
            .as_ref()
            .expect("a font must be provided first")
            .borrow_mut()
    }

    fn recalculate_font_metrics(&mut self) {
        let font = self.borrow_font().with_size(self.top().font_size).unwrap();
        let (_, metrics) = font.metrics();
        self.top_mut().font_height_in_pixels = metrics.cap_height.abs();
    }

    /// Sets the font used by the current group.
    pub fn set_font(&mut self, new_font: RcFont) {
        self.top_mut().font = Some(new_font);
        if self.top().font_size > 0.0 {
            self.recalculate_font_metrics();
        }
    }

    /// Sets the size of the font used by the current group.
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

    /// Draws text aligned inside of the group.
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

    /// Returns the width and height of the given string, using the current font and font size.
    pub fn text_size(&self, text: &str) -> (f32, f32) {
        self.text_size_impl(text, &mut self.borrow_font_mut())
    }

    /// Returns the position of where text would be drawn in the current group, given the provided
    /// alignment.
    pub fn text_origin(&self, text: &str, alignment: Alignment) -> Point {
        self.text_origin_impl(text, alignment, &mut self.borrow_font_mut()).0
    }

    /// Draws an icon inside the current group.
    ///
    /// This creates a new group, and so offsets placement of any elements after it.
    ///
    /// The `color` is used to tint the icon with a specific color.
    ///
    /// The `group_size`, if provided, can override the size of the group created to fit the icon.
    pub fn icon(
        &mut self,
        canvas: &mut Canvas,
        icon: &Image,
        color: impl Into<Color4f>,
        group_size: Option<(f32, f32)>,
    ) {
        let group_size = group_size.unwrap_or((icon.width() as f32, icon.height() as f32));
        self.push_group(group_size, Layout::Freeform);

        // probably quite horrible but there aren't that many icons drawn to the screen at once in the first
        // place
        let image_bounds = IRect::new(0, 0, icon.width(), icon.height());
        let color_filter = color_filters::blend(color.into().to_color(), BlendMode::SrcATop).unwrap();
        let filter = image_filters::color_filter(color_filter, None, None).unwrap();
        let colored_icon = icon
            .new_with_filter(None, &filter, image_bounds, image_bounds)
            .unwrap()
            .0;

        let x = self.top().rect.left + self.width() / 2.0 - icon.width() as f32 / 2.0;
        let y = self.top().rect.top + self.height() / 2.0 - icon.height() as f32 / 2.0;
        canvas.draw_image(colored_icon, (x, y), None);
        self.pop_group();
    }

    /// Draws lines of text.
    ///
    /// The lines of text create new groups, so this function offsets the element placement
    /// position.
    pub fn paragraph(
        &mut self,
        canvas: &mut Canvas,
        color: impl Into<Color4f>,
        alignment: AlignH,
        line_spacing: Option<f32>,
        text: &[&str],
    ) {
        let line_spacing = line_spacing.unwrap_or(1.2);
        let line_height = self.font_size() * line_spacing;
        let height = (line_height * text.len() as f32).round();
        let color = color.into();
        self.push_group((self.width(), height), Layout::Vertical);
        for line in text {
            self.push_group((self.width(), line_height), Layout::Freeform);
            self.text(canvas, line, color.clone(), (alignment, AlignV::Middle));
            self.pop_group();
        }
        self.pop_group();
    }

    /// Performs custom drawing on the canvas, with the transform matrix translated to the topmost
    /// group's position.
    pub fn draw_on_canvas(&self, canvas: &mut Canvas, callback: impl FnOnce(&mut Canvas)) {
        let offset = Point::new(self.top().rect.left, self.top().rect.top);
        canvas.save();
        canvas.translate(offset);
        callback(canvas);
        canvas.restore();
    }

    /// Returns the position of the mouse inside the current group.
    pub fn mouse_position(&self, input: &Input) -> Point {
        input.mouse_position() - self.top().rect.to_quad()[0]
    }

    /// Returns whether the topmost group has the mouse cursor.
    pub fn has_mouse(&self, input: &Input) -> bool {
        let mouse = self.mouse_position(input);
        let Size { width, height } = self.top().rect.size();
        mouse.x >= 0.0 && mouse.x <= width && mouse.y >= 0.0 && mouse.y <= height
    }
}

/// A trait implemented by elements that can be (un)focused.
pub trait Focus {
    fn focused(&self) -> bool;
    fn set_focus(&mut self, focused: bool);
}

/// Creates a _focus chain_, that is, a list of elements that can be `Tab`bed between.
pub fn chain_focus(input: &Input, fields: &mut [&mut dyn Focus]) {
    if input.key_just_typed(VirtualKeyCode::Tab) {
        let mut had_focus = false;
        for element in fields.iter_mut() {
            if had_focus {
                element.set_focus(true);
                return
            }
            if element.focused() {
                element.set_focus(false);
                had_focus = true;
            }
        }
        if !fields.is_empty() {
            fields[0].set_focus(true);
        }
    }
}
