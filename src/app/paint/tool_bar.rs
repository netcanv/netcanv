//! The toolbar.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use netcanv_renderer::paws::{
   point, vector, AlignH, AlignV, Alignment, Color, Layout, LineCap, Point, Rect, Renderer,
};
use netcanv_renderer_opengl::winit::event::MouseButton;

use crate::common::{ControlFlow, RectMath};
use crate::config::{self, config, ToolbarPosition};
use crate::ui::view::{self, Dimensions, View};
use crate::ui::wm::{HitTest, WindowContent, WindowContentArgs, WindowId, WindowManager};
use crate::ui::{Button, ButtonArgs, ButtonState, Input, Ui, UiElements, UiInput};

use super::tools::Tool;

/// Arguments for processing the toolbar.
pub struct ToolbarArgs<'a> {
   pub wm: &'a mut WindowManager,
   pub colors: &'a ToolbarColors,
   pub parent_view: &'a View,
}

/// The toolbar's color scheme.
#[derive(Clone)]
pub struct ToolbarColors {
   pub position_highlight: Color,
}

/// The unique index of a tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolId(usize);

/// The supervisor of the toolbar window. Draws overlays when the toolbar is being dragged around,
/// and manages the window.
pub struct Toolbar {
   window: WindowId<ToolbarData>,
   tools: Rc<RefCell<Vec<Box<dyn Tool>>>>,
   tools_by_name: HashMap<String, ToolId>,
   current_tool: ToolId,
}

impl Toolbar {
   /// Creates a new, empty toolbar.
   pub fn new(wm: &mut WindowManager) -> Self {
      let view = View::new(ToolbarWindow::dimensions(0));
      let content = ToolbarWindow::new();
      let tools = Rc::new(RefCell::new(Vec::new()));
      let data = ToolbarData::new(Rc::clone(&tools));
      let window =
         wm.open_window(view, content, data).set_pinned(true).set_focusable(false).finish();
      Self {
         window,
         tools,
         tools_by_name: HashMap::new(),
         current_tool: ToolId(0),
      }
   }

   /// Returns the toolbar position, as declared in the user config.
   fn position() -> ToolbarPosition {
      config().ui.toolbar_position
   }

   /// Adds a tool into the toolbar.
   pub fn add_tool(&mut self, tool: impl Tool + 'static) -> ToolId {
      let boxed = Box::new(tool);
      let mut tools = self.tools.borrow_mut();
      let id = ToolId(tools.len());
      self.tools_by_name.insert(boxed.name().to_owned(), id);
      tools.push(boxed);
      id
   }

   /// Returns the number of tools in the toolbar.
   pub fn tool_count(&self) -> usize {
      self.tools.borrow().len()
   }

   /// Returns the ID of the tool with the given name, or `None` if there is no such tool.
   pub fn tool_by_name(&self, name: &str) -> Option<ToolId> {
      self.tools_by_name.get(name).copied()
   }

   /// Returns the ID of the currently selected tool.
   pub fn current_tool(&self) -> ToolId {
      self.current_tool
   }

   /// Sets the current tool.
   pub fn set_current_tool(&mut self, tool: ToolId) {
      self.current_tool = tool;
   }

   /// Returns a copy of the tool with the given ID.
   pub fn clone_tool_name(&self, tool: ToolId) -> String {
      self.tools.borrow()[tool.0].name().to_owned()
   }

   /// Borrows the given tool mutably to the given closure.
   pub fn with_tool<R>(&mut self, tool: ToolId, f: impl FnOnce(&mut Box<dyn Tool>) -> R) -> R {
      let mut tools = self.tools.borrow_mut();
      f(&mut tools[tool.0])
   }

   /// Borrows the current tool mutably, for the duration of the given closure.
   ///
   /// Note that this must not be called recursively.
   pub fn with_current_tool<R>(&mut self, f: impl FnOnce(&mut Box<dyn Tool>) -> R) -> R {
      self.with_tool(self.current_tool, f)
   }

   /// Borrows each tool mutably to the given closure.
   pub fn with_each_tool<B>(
      &mut self,
      mut f: impl FnMut(ToolId, &mut Box<dyn Tool>) -> ControlFlow<B>,
   ) -> Option<B> {
      let mut value = None;
      let mut tools = self.tools.borrow_mut();
      for (i, tool) in tools.iter_mut().enumerate() {
         match f(ToolId(i), tool) {
            ControlFlow::Continue => (),
            ControlFlow::Break(v) => {
               value = Some(v);
               break;
            }
         }
      }
      value
   }

   /// Returns the alignment of the toolbar window, according to a toolbar position.
   fn view_alignment(position: ToolbarPosition) -> Alignment {
      match position {
         ToolbarPosition::Left => (AlignH::Left, AlignV::Middle),
         ToolbarPosition::Right => (AlignH::Right, AlignV::Middle),
      }
   }

   /// Finds out the toolbar's snapped position, using the given "precise" position.
   fn snap_position(parent_view: &View, position: Point) -> Option<ToolbarPosition> {
      let rect = parent_view.rect();
      let left = Rect {
         size: vector(rect.width() / 2.0, rect.height()),
         ..rect
      };
      let right = Rect {
         position: left.position + vector(left.size.x, 0.0),
         ..left
      };
      if left.contains(position) {
         Some(ToolbarPosition::Left)
      } else if right.contains(position) {
         Some(ToolbarPosition::Right)
      } else {
         None
      }
   }

   fn position_view(parent_view: &View, view: &mut View, position: ToolbarPosition) {
      view::layout::align(parent_view, view, Self::view_alignment(position));
   }

   /// Processes the toolbar.
   pub fn process(
      &mut self,
      ui: &mut Ui,
      input: &Input,
      ToolbarArgs {
         wm,
         colors,
         parent_view,
      }: ToolbarArgs,
   ) -> ToolbarProcessResult {
      let position = Self::position();

      // Update the view's size and lay it out in the parent view.
      wm.view_mut(&self.window).dimensions = ToolbarWindow::dimensions(self.tool_count());
      if wm.dragging(&self.window) {
         let window_view = wm.view(&self.window).clone();
         let new_position =
            Self::snap_position(parent_view, window_view.rect().top_center()).unwrap_or(position);

         // Draw a preview for where the new position is going to be while dragging.
         let mut preview = View::new(window_view.dimensions);
         Self::position_view(parent_view, &mut preview, new_position);
         let rect = preview.rect();
         ui.render().outline(rect, colors.position_highlight, rect.width() / 2.0, 1.0);
         ui.render().fill(
            rect,
            colors.position_highlight.with_alpha(127),
            rect.width() / 2.0,
         );

         // Draw a guide line for where the boundary between the left and right side is.
         let parent_rect = parent_view.rect();
         let center_x = parent_rect.center_x();
         let top = parent_rect.top();
         let bottom = parent_rect.bottom();
         ui.render().line(
            point(center_x, top),
            point(center_x, bottom),
            colors.position_highlight,
            LineCap::Butt,
            1.0,
         );

         // Snap to the correct position if the mouse was released.
         if input.action(MouseButton::Left) == (true, ButtonState::Released)
            && new_position != position
         {
            config::write(|config| {
               config.ui.toolbar_position = new_position;
            })
         }
      } else {
         Self::position_view(parent_view, wm.view_mut(&self.window), position);
      }

      // Update the shared data to reflect the current state of the toolbar.
      let data = wm.window_data_mut(&self.window);
      let mut switched = None;
      if let Some(tool_id) = data.selected_tool.take() {
         let previous_tool = self.current_tool;
         self.current_tool = tool_id;
         switched = Some((previous_tool, self.current_tool));
      }
      data.current_tool = self.current_tool;

      ToolbarProcessResult { switched }
   }
}

#[must_use]
pub struct ToolbarProcessResult {
   pub switched: Option<(ToolId, ToolId)>,
}

/// The shared data between the toolbar window and the toolbar supervisor.
struct ToolbarData {
   tools: Rc<RefCell<Vec<Box<dyn Tool>>>>,
   current_tool: ToolId,
   selected_tool: Option<ToolId>,
}

impl ToolbarData {
   /// Creates new toolbar data, with the given tools list.
   fn new(tools: Rc<RefCell<Vec<Box<dyn Tool>>>>) -> Self {
      Self {
         tools,
         current_tool: ToolId(0),
         selected_tool: None,
      }
   }
}

/// The toolbar window.
struct ToolbarWindow;

impl ToolbarWindow {
   /// The width of the toolbar.
   const TOOLBAR_SIZE: f32 = 40.0;
   /// The width and height of a tool button.
   const TOOL_SIZE: f32 = Self::TOOLBAR_SIZE - 8.0;
   const DRAG_HANDLE_SIZE: f32 = 16.0;

   fn new() -> Self {
      Self {}
   }

   fn dimensions(n_tools: usize) -> Dimensions {
      let padding = 4.0;
      let length = padding + Self::DRAG_HANDLE_SIZE + n_tools as f32 * (Self::TOOL_SIZE + padding);
      Dimensions::from((Self::TOOLBAR_SIZE, length))
   }
}

impl WindowContent for ToolbarWindow {
   type Data = ToolbarData;

   fn process(
      &mut self,
      WindowContentArgs {
         ui,
         assets,
         input,
         hit_test,
         ..
      }: &mut WindowContentArgs,
      data: &mut Self::Data,
   ) {
      ui.push(ui.size(), Layout::Vertical);

      ui.fill_rounded(assets.colors.panel, ui.width().min(ui.height()) / 2.0);
      ui.pad(4.0);

      // The dragging handle.
      ui.push((ui.width(), Self::DRAG_HANDLE_SIZE), Layout::Freeform);
      ui.icon(
         &assets.icons.navigation.drag_horizontal,
         assets.colors.drag_handle,
         Some(ui.size()),
      );
      if ui.hover(input) {
         **hit_test = HitTest::Draggable;
      }
      ui.pop();

      // The tools.
      let tools = data.tools.borrow_mut();
      for (i, tool) in tools.iter().enumerate() {
         let i = ToolId(i);
         ui.push((Self::TOOL_SIZE, Self::TOOL_SIZE), Layout::Freeform);
         if Button::with_icon(
            ui,
            input,
            ButtonArgs {
               height: Self::TOOL_SIZE,
               colors: if data.current_tool == i {
                  &assets.colors.selected_toolbar_button
               } else {
                  &assets.colors.toolbar_button
               },
               corner_radius: ui.width() / 2.0,
            },
            tool.icon(),
         )
         .clicked()
         {
            data.selected_tool = Some(i);
         }
         ui.pop();
         ui.space(4.0);
      }

      ui.pop();
   }
}
