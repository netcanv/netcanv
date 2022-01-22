// NetCanv - online collaborative paint canvas
// Copyright 2021-2022, liquidev
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//
//--------------------------------------------------------------------------------------------------
//
// Welcome to main.rs! You've come this far, and I'm happy to see you here.
// Here are some points of interest within the codebase:
//
//  - main.rs - handles platform details, such as opening a window and setting up the renderer.
//  - paint_canvas.rs - the infinite paint canvas.
//  - assets.rs - asset loading and color schemes.
//  - config.rs - user configuration.
//  - assets/ - does not contain any code, but rather actual assets, such as fonts and icons.
//  - app/ - contains app states (the lobby and paint UI).
//    - paint/ - contains the painting state. This is where you draw things with friends
//      - actions/ - actions available in the overflow menu
//      - tools/ - tools available in the toolbar on the left
//      - mod.rs - the state UI itself
//    - lobby.rs - the lobby UI
//  - net/ - contains networking-related code (communicating with the relay and other clients).
//  - ui/ - contains NetCanv's bespoke UI framework, as well as all the widgets.
//
// This list may become out of date with time, as the app gets refactored, so feel free to explore,
// and maybe even send a PR if you think something here is wrong.
//
// I hope you enjoy hacking on NetCanv!
//    - liquidev

// Prevent opening a console on Windows if this is a release build.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::fmt::Write;

use crate::backend::winit::dpi::LogicalSize;
use crate::backend::winit::event::{Event, WindowEvent};
use crate::backend::winit::event_loop::{ControlFlow, EventLoop};
#[cfg(target_family = "unix")]
use crate::backend::winit::platform::unix::*;
use crate::backend::winit::window::{CursorIcon, WindowBuilder};
use crate::ui::view::{self, View};
use backend::Backend;
use native_dialog::{MessageDialog, MessageType};
use netcanv_renderer::paws::{vector, Layout};

#[cfg(feature = "renderer-opengl")]
use netcanv_renderer_opengl::UiRenderFrame;
#[cfg(feature = "renderer-skia")]
use netcanv_renderer_skia::UiRenderFrame;

#[macro_use]
mod common;
mod app;
mod assets;
mod backend;
mod clipboard;
mod color;
mod config;
mod keymap;
mod net;
mod paint_canvas;
mod token;
mod ui;
mod viewport;

use app::*;
use assets::*;
use config::config;
use ui::{Input, Ui};

fn inner_main() -> anyhow::Result<()> {
   println!("test2");

   // Set up the winit event loop and open the window.
   let event_loop = EventLoop::new();
   let window_builder = {
      let b = WindowBuilder::new()
         .with_inner_size(LogicalSize::<u16>::new(1024, 600))
         .with_title("NetCanv")
         .with_resizable(true);
      // On Linux, winit doesn't seem to set the app ID properly so Wayland compositors can't tell
      // our window apart from others.
      #[cfg(target_os = "linux")]
      let b = b.with_app_id("netcanv".into());
      b
   };

   // Load the user configuration and color scheme.
   // TODO: User-definable color schemes, anyone?
   config::load_or_create()?;
   let color_scheme = ColorScheme::from(config().ui.color_scheme);

   // Build the render backend.
   let renderer = Backend::new(window_builder, &event_loop)?;
   // Also, initialize the clipboard because we now have a window handle.
   match clipboard::init() {
      Ok(_) => (),
      Err(error) => eprintln!("failed to initialize clipboard: {}", error),
   }

   // Build the UI.
   let mut ui = Ui::new(renderer);

   // Load all the assets, and start the first app state.
   let assets = Assets::new(ui.render(), color_scheme);
   let mut app: Option<Box<dyn AppState>> = Some(Box::new(lobby::State::new(assets)) as _);
   let mut input = Input::new();

   event_loop.run(move |event, _, control_flow| {
      *control_flow = ControlFlow::Poll;

      match event {
         Event::WindowEvent { event, .. } => {
            if let WindowEvent::CloseRequested = event {
               *control_flow = ControlFlow::Exit;
            } else {
               input.process_event(&event);
            }
         }

         Event::MainEventsCleared => {
            let window_size = ui.window().inner_size();
            match ui.render_frame(|ui| {
               ui.root(
                  vector(window_size.width as f32, window_size.height as f32),
                  Layout::Freeform,
               );
               let mut root_view = View::group_sized(ui);
               view::layout::full_screen(&mut root_view);

               input.set_cursor(CursorIcon::Default);
               app.as_mut().unwrap().process(StateArgs {
                  ui,
                  input: &mut input,
                  root_view,
               });
               app = Some(app.take().unwrap().next_state(ui.render()));
            }) {
               Err(error) => eprintln!("render error: {}", error),
               _ => (),
            }
            input.finish_frame(ui.window());
         }

         _ => (),
      }
   });
}

fn main() {
   let default_panic_hook = std::panic::take_hook();
   std::panic::set_hook(Box::new(move |panic_info| {
      // Pretty panic messages are only enabled in release mode, as they hinder debugging.
      #[cfg(not(debug_assertions))]
      {
         let mut message = heapless::String::<8192>::new();
         let _ = write!(message, "Oh no! A fatal error occured.\n{}", panic_info);
         let _ = write!(message, "\n\nThis is most definitely a bug, so please file an issue on GitHub. https://github.com/liquidev/netcanv");
         let _ = MessageDialog::new()
            .set_title("NetCanv - Fatal Error")
            .set_text(&message)
            .set_type(MessageType::Error)
            .show_alert();
      }
      default_panic_hook(panic_info);
   }));

   match inner_main() {
      Ok(()) => (),
      Err(payload) => {
         let mut message = String::new();
         let _ = write!(
            message,
            "An error occured:\n{}\n\nIf you think this is a bug, please file an issue on GitHub. https://github.com/liquidev/netcanv",
            payload
         );
         eprintln!("inner_main() returned with an Err:\n{}", payload);
         MessageDialog::new()
            .set_title("NetCanv - Error")
            .set_text(&message)
            .set_type(MessageType::Error)
            .show_alert()
            .unwrap();
      }
   }
}
