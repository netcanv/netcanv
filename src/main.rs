//
// NetCanv - online collaborative paint canvas
// Copyright (C) 2021, liquidev and contributors
//
// Licensed under the MIT license. Check LICENSE.txt in the repository root for details.
//
// Welcome to main.rs! You've come this far, and I'm happy to see you here.
// Here are some points of interest within the codebase:
//
//  - main.rs - handles platform details, such as opening a window and setting up the renderer.
//  - paint_canvas.rs - the infinite paint canvas.
//  - assets.rs - asset loading and color schemes.
//  - assets/ - does not contain any code, but rather actual assets, such as fonts and icons.
//  - app/ - contains app states (the lobby and paint UI).
//  - net/ - contains networking-related code (communicating with the matchmaker and other clients).
//  - ui/ - contains NetCanv's bespoke UI framework, as well as all the widgets.
//
// This list may become out of date with time, as the app gets refactored, so feel free to explore,
// and maybe even send a PR if you think something here is wrong.
//
// I hope you enjoy hacking on NetCanv!
//    - liquidev
//

// Prevent opening a console on Windows if this is a release build.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::fmt::Write;

use backend::Backend;
use config::UserConfig;
use native_dialog::{MessageDialog, MessageType};
use netcanv_renderer::paws::{vector, Layout};
use winit::dpi::LogicalSize;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
#[cfg(target_family = "unix")]
use winit::platform::unix::*;
use winit::window::WindowBuilder;

#[cfg(feature = "renderer-opengl")]
use netcanv_renderer_opengl::UiRenderFrame;
#[cfg(feature = "renderer-skia")]
use netcanv_renderer_skia::UiRenderFrame;

#[macro_use]
mod common;
mod app;
mod assets;
mod backend;
mod config;
mod net;
mod paint_canvas;
mod token;
mod ui;
mod viewport;

use app::*;
use assets::*;
use ui::{Input, Ui};

fn inner_main() -> anyhow::Result<()> {
   // Set up the winit event loop and open the window.
   let event_loop = EventLoop::new();
   let window_builder = {
      let b = WindowBuilder::new()
         .with_inner_size(LogicalSize::new(1024, 600))
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
   let config = UserConfig::load_or_create()?;
   let color_scheme = match config.ui.color_scheme {
      config::ColorScheme::Light => ColorScheme::light(),
      config::ColorScheme::Dark => ColorScheme::dark(),
   };

   // Build the render backend.
   let renderer = Backend::new(window_builder, &event_loop)?;

   // On Wayland, winit draws its own set of decorations, which can be customized.
   // We customize them to fit our color scheme.
   #[cfg(target_family = "unix")]
   renderer.window().set_wayland_theme(color_scheme.clone());

   // Build the UI.
   let mut ui = Ui::new(renderer);

   // Load all the assets, and start the first app state.
   let assets = Assets::new(color_scheme);
   let mut app: Option<Box<dyn AppState>> = Some(Box::new(lobby::State::new(assets, config)) as _);
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
                  Layout::Vertical,
               );
               // `unwrap()` always succeeds here as app is never None.
               app.as_mut().unwrap().process(StateArgs {
                  ui,
                  input: &mut input,
               });
               // See? Told ya.
               app = Some(app.take().unwrap().next_state());
            }) {
               Err(error) => eprintln!("render error: {}", error),
               _ => (),
            }
            input.finish_frame();
         }

         _ => (),
      }
   });
}

fn main() {
   let default_panic_hook = std::panic::take_hook();
   std::panic::set_hook(Box::new(move |panic_info| {
      // Pretty error messages are only enabled in release mode, as they hinder debugging.
      //      #[cfg(not(debug_assertions))]
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
