//! The toolbar.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use netcanv_renderer::paws::{AlignH, AlignV, Alignment, Layout};

use crate::common::ControlFlow;
use crate::config::{config, ToolbarPosition};
use crate::ui::view::{self, Dimensions, View};
use crate::ui::wm::{WindowContent, WindowContentArgs, WindowId, WindowManager};
use crate::ui::{Button, ButtonArgs};

use super::tools::Tool;

/// Arguments for processing the toolbar.
pub struct ToolbarArgs<'a> {
   pub wm: &'a mut WindowManager,
   pub parent_view: &'a View,
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
      let view = View::new(ToolbarWindow::dimensions(Self::position(), 0));
      let content = ToolbarWindow::new();
      let tools = Rc::new(RefCell::new(Vec::new()));
      let data = ToolbarData::new(Rc::clone(&tools));
      let window = wm.open_window(view, content, data).set_pinned(true).finish();
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
         ToolbarPosition::Top => (AlignH::Center, AlignV::Top),
         ToolbarPosition::Right => (AlignH::Right, AlignV::Middle),
         ToolbarPosition::Bottom => (AlignH::Center, AlignV::Bottom),
      }
   }

   /// Processes the toolbar.
   pub fn process(&mut self, ToolbarArgs { wm, parent_view }: ToolbarArgs) {
      let position = Self::position();

      // Update the view's size and lay it out in the parent view.
      wm.view_mut(&self.window).dimensions = ToolbarWindow::dimensions(position, self.tool_count());
      view::layout::align(
         parent_view,
         wm.view_mut(&self.window),
         Self::view_alignment(position),
      );

      let data = wm.window_data_mut(&self.window);
      if let Some(tool_id) = data.selected_tool.take() {
         self.current_tool = tool_id;
      }
      data.current_tool = self.current_tool;
   }
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

   fn new() -> Self {
      Self {}
   }

   fn dimensions(position: ToolbarPosition, n_tools: usize) -> Dimensions {
      let length = 4.0 + n_tools as f32 * (Self::TOOL_SIZE + 4.0);
      match position {
         ToolbarPosition::Left | ToolbarPosition::Right => (Self::TOOLBAR_SIZE, length),
         ToolbarPosition::Top | ToolbarPosition::Bottom => (length, Self::TOOLBAR_SIZE),
      }
      .into()
   }
}

impl WindowContent for ToolbarWindow {
   type Data = ToolbarData;

   fn process(
      &mut self,
      WindowContentArgs {
         ui, assets, input, ..
      }: &mut WindowContentArgs,
      data: &mut Self::Data,
   ) {
      ui.push(
         ui.size(),
         match Toolbar::position() {
            ToolbarPosition::Top | ToolbarPosition::Bottom => Layout::Horizontal,
            ToolbarPosition::Left | ToolbarPosition::Right => Layout::Vertical,
         },
      );

      ui.fill_rounded(assets.colors.panel, ui.width().min(ui.height()) / 2.0);
      ui.pad(4.0);

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
