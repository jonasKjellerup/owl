use smithay_client_toolkit::{
    default_environment,
    environment::SimpleGlobal,
    new_default_environment,
    output::{with_output_info, OutputInfo},
    reexports::{
        calloop,
        client::protocol::{wl_output, wl_shm, wl_surface},
        client::{Attached, Main},
        protocols::wlr::unstable::layer_shell::v1::client as wlr_client,
        protocols::wlr::unstable::layer_shell::v1::client::{
            zwlr_layer_shell_v1, zwlr_layer_surface_v1,
        },
    },
    shm::AutoMemPool,
    WaylandSource,
};
use owl::{SharedLoopData, UpdateHandle, EventLoop};

use argh::FromArgs;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::io::Read;
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::RwLock;
use log::error;
use piet::{ImageFormat, RenderContext, TextLayoutBuilder};
use piet::{kurbo::Rect, Color};
use piet::kurbo::Point;
use piet_common::{BitmapTarget, CairoRenderContext, CairoTextLayout, CairoTextLayoutBuilder, Device, Text};
use owl::wayland::{SurfaceAction, WaylandContext};

mod config;
mod udev;
//mod bar;

const DEFAULT_CONFIG_PATH: &str = "~/.config/.rs-bar";

#[derive(FromArgs)]
#[argh(description = "Wayland status bar")]
struct Args {
    /// path to configuration file
    #[argh(option, short = 'c')]
    config: Option<String>,
}

impl Args {
    fn load_config(&self) -> Result<config::Bar, impl std::error::Error> {
        let path = self.config.as_ref()
            .map(|s| s.as_str())
            .unwrap_or(DEFAULT_CONFIG_PATH);

        let src = std::fs::File::open(path)
            .and_then(|mut f| {
                let mut s = String::new();
                f.read_to_string(&mut s)
                    .map(|_| s)
            })?;

        toml::from_str(src.as_str())
            .map_err(|err| <toml::de::Error as Into<std::io::Error>>::into(err))
    }
}

/*impl Surface {
    fn new(
        output: &wl_output::WlOutput,
        surface: wl_surface::WlSurface,
        layer_shell: &Attached<zwlr_layer_shell_v1::ZwlrLayerShellV1>,
        pool: AutoMemPool,
        modules: owl::Modules, // TODO merge modules and config to one
        config: Rc<config::Bar>,
    ) -> Self {


        layer_surface.set_size(1920, 30);
        // Anchor to the top left corner of the output
        layer_surface.set_anchor(config::Anchor::Top.into());
        layer_surface.set_exclusive_zone(30);

        let next_render_event = Rc::new(Cell::new(None::<RenderEvent>));
        let next_render_event_handle = Rc::clone(&next_render_event);
        layer_surface.quick_assign(move |layer_surface, event, _| {
            match (event, next_render_event_handle.get()) {
                (zwlr_layer_surface_v1::Event::Closed, _) => {
                    next_render_event_handle.set(Some(RenderEvent::Closed));
                }
                (zwlr_layer_surface_v1::Event::Configure { serial, width, height }, next)
                if next != Some(RenderEvent::Closed) =>
                    {
                        layer_surface.ack_configure(serial);
                        next_render_event_handle.set(Some(RenderEvent::Configure { width, height }));
                    }
                (_, _) => {}
            }
        });

        // Commit so that the server will send a configure event
        surface.commit();

        Self { surface, layer_surface, next_render_event, pool, dimensions: (0, 0), modules, config }
    }

    /// Handles any events that have occurred since the last call, redrawing if needed.
    /// Returns true if the surface should be dropped.
    fn handle_events(&mut self) -> bool {
        match self.next_render_event.take() {
            Some(RenderEvent::Closed) => true,
            Some(RenderEvent::Configure { width, height }) => {
                if self.dimensions != (width, height) {
                    self.dimensions = (width, height);
                    self.draw();
                }
                false
            }
            None => false,
        }
    }

    fn draw(&mut self) {
        let stride = 4 * self.dimensions.0 as i32;
        let width = self.dimensions.0 as i32;
        let height = self.dimensions.1 as i32;

        // Note: unwrap() is only used here in the interest of simplicity of the example.
        // A "real" application should handle the case where both pools are still in use by the
        // compositor.
        let (canvas, buffer) =
            self.pool.buffer(width, height, stride, wl_shm::Format::Argb8888).unwrap();


        let mut surface = PietWaylandSurface::new(canvas, width, height, stride);
        let mut rc = surface.get_context();

        rc.fill(
            Rect::new(0.0, 0.0, 1920.0, 30.0),
            &Color::rgba8(0, 255, 0, 255));


        let mut text_source = String::new();
        if let Ok(modules) = self.modules.read() {
            let _ = modules.get("battery")
                .map(|m| m.write("charge", &mut text_source));
        }

        let mut text = piet_common::CairoText::new();
        let mut layout_builder = text.new_text_layout(text_source)
            .text_color(Color::rgba8(0xFF, 0, 0, 0xFF))
            .alignment(piet_common::TextAlignment::Center)
            .build()
            .expect("Unable build text layout");

        rc.draw_text(&layout_builder, Point::new(600.0, 10.0));

        rc.finish().unwrap();


        // Attach the buffer to the surface and mark the entire surface as damaged
        self.surface.attach(Some(&buffer), 0, 0);
        self.surface.damage_buffer(0, 0, width as i32, height as i32);

        // Finally, commit the surface
        self.surface.commit();
    }
}*/





// TODO remove/reduce unwrap usage in main
fn main() {
    let args: Args = argh::from_env();
    let config = args.load_config();

    let config = Rc::new(match config {
        Ok(bar) => bar,
        Err(err) => {
            error!("{:?}", err);
            let path = args.config.as_ref().map(String::as_str).unwrap_or(DEFAULT_CONFIG_PATH);
            error!("Unable to load config: {}", path);
            return;
        }
    });

    let modules: owl::Modules = Rc::new(RwLock::new(HashMap::new()));
    let modules_ref = modules.clone();
    {
        let mut modules = modules.write().unwrap();
        let mut bat_mod = owl::modules::battery::BatteryModule::init()
            .expect("Unable to initialise battery module");
        modules.insert("battery", Box::new(bat_mod));
    }

    let wayland_context = WaylandContext::new().unwrap();
    let mut event_loop = EventLoop::try_new().unwrap();
    let wayland_context = wayland_context.insert_queue_in(event_loop.handle()).unwrap();

    let mut loop_data = SharedLoopData {
        update_handle: UpdateHandle::new(),
        modules: modules_ref,
    };

    loop {
        // This is ugly, let's hope that some version of drain_filter() gets stabilized soon
        // https://github.com/rust-lang/rust/issues/43244
        {
            let mut surfaces = wayland_context.surfaces.borrow_mut();
            let mut i = 0;
            while i != surfaces.len() {
                if let SurfaceAction::Drop = surfaces[i].1.handle_events() {
                    surfaces.remove(i);
                } else {
                    i += 1;
                }
            }
        }

        wayland_context.flush_display().unwrap();
        event_loop.dispatch(None, &mut loop_data).unwrap();
    }
}
