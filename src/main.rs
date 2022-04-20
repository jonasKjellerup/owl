// TODO investigate fullscreen mode rules. Currently overlaps fullscreen elements.

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

use argh::FromArgs;
use std::cell::{Cell, RefCell};
use std::io::Read;
use std::marker::PhantomData;
use std::rc::Rc;
use piet::{ImageFormat, RenderContext, TextLayoutBuilder};
use piet::{kurbo::Rect, Color};
use piet::kurbo::Point;
use piet_common::{BitmapTarget, CairoRenderContext, CairoTextLayout, CairoTextLayoutBuilder, Device, Text};
use crate::modules::Module;

mod config;
//mod udev;
mod modules;
//mod bar;

const DEFAULT_CONFIG_PATH: &str = "~/.config/.rs-bar";

#[derive(FromArgs)]
#[argh(description = "Wayland status bar (greeeeeeeen)")]
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

default_environment!(Env,
    fields = [
        layer_shell: SimpleGlobal<zwlr_layer_shell_v1::ZwlrLayerShellV1>,
    ],
    singles = [
        zwlr_layer_shell_v1::ZwlrLayerShellV1 => layer_shell
    ],
);

#[derive(PartialEq, Copy, Clone)]
enum RenderEvent {
    Configure { width: u32, height: u32 },
    Closed,
}

struct PietWaylandSurface<'a> {
    image_surface: cairo::ImageSurface,
    context: cairo::Context,
    phantom: std::marker::PhantomData<&'a ()>,
}

impl<'a> PietWaylandSurface<'a> {
    fn new(canvas: &'a mut [u8], width: i32, height: i32, stride: i32) -> Self {
        let image_surface = unsafe {
            cairo::ImageSurface::create_for_data_unsafe(
                canvas.as_mut_ptr(),
                cairo::Format::ARgb32,
                width,
                height,
                stride,
            ).expect("Unable to create ImageSurface for wayland canvas,")
        };

        let context = cairo::Context::new(&image_surface)
            .expect("Unable to create Context from ImageSurface");

        PietWaylandSurface {
            image_surface,
            context,
            phantom: PhantomData::default(),
        }
    }

    fn get_context(&mut self) -> piet_common::CairoRenderContext {
        CairoRenderContext::new(&self.context)
    }
}

struct Surface {
    surface: wl_surface::WlSurface,
    layer_surface: Main<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1>,
    next_render_event: Rc<Cell<Option<RenderEvent>>>,
    pool: AutoMemPool,
    dimensions: (u32, u32),
}

impl Surface {
    fn new(
        output: &wl_output::WlOutput,
        surface: wl_surface::WlSurface,
        layer_shell: &Attached<zwlr_layer_shell_v1::ZwlrLayerShellV1>,
        pool: AutoMemPool,
    ) -> Self {
        let layer_surface = layer_shell.get_layer_surface(
            &surface,
            Some(output),
            zwlr_layer_shell_v1::Layer::Overlay,
            "example".to_owned(),
        );

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

        Self { surface, layer_surface, next_render_event, pool, dimensions: (0, 0) }
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

        let mut text = piet_common::CairoText::new();
        let mut layout_builder = text.new_text_layout("Text test")
            .text_color(Color::rgba8(0xFF,0,0,0xFF))
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
}

impl Drop for Surface {
    fn drop(&mut self) {
        self.layer_surface.destroy();
        self.surface.destroy();
    }
}

fn main() {
    let args: Args = argh::from_env();
    let config = args.load_config();

    let config = match config {
        Ok(bar) => bar,
        Err(err) => {
            println!("{:?}", err);
            let path = args.config.as_ref().map(String::as_str).unwrap_or(DEFAULT_CONFIG_PATH);
            println!("Unable to load config: {}", path); // TODO print to error pipe instead.
            return;
        }
    };

    let mut bat_mod = modules::battery::BatteryModule::init();

    let (env, display, queue) =
        new_default_environment!(Env, fields = [layer_shell: SimpleGlobal::new(),])
            .expect("Initial roundtrip failed!");

    let surfaces = Rc::new(RefCell::new(Vec::new()));

    let layer_shell = env.require_global::<zwlr_layer_shell_v1::ZwlrLayerShellV1>();

    let env_handle = env.clone();
    let surfaces_handle = Rc::clone(&surfaces);
    let output_handler = move |output: wl_output::WlOutput, info: &OutputInfo| {
        if info.obsolete {
            // an output has been removed, release it
            surfaces_handle.borrow_mut().retain(|(i, _)| *i != info.id);
            output.release();
        } else {
            // an output has been created, construct a surface for it
            let surface = env_handle.create_surface().detach();
            let pool = env_handle.create_auto_pool().expect("Failed to create a memory pool!");
            (*surfaces_handle.borrow_mut())
                .push((info.id, Surface::new(&output, surface, &layer_shell.clone(), pool)));
        }
    };

    // Process currently existing outputs
    for output in env.get_all_outputs() {
        if let Some(info) = with_output_info(&output, Clone::clone) {
            output_handler(output, &info);
        }
    }

    // Setup a listener for changes
    // The listener will live for as long as we keep this handle alive
    let _listener_handle =
        env.listen_for_outputs(move |output, info, _| output_handler(output, info));

    let mut event_loop = calloop::EventLoop::<()>::try_new().unwrap();

    WaylandSource::new(queue).quick_insert(event_loop.handle()).unwrap();

    loop {
        // This is ugly, let's hope that some version of drain_filter() gets stabilized soon
        // https://github.com/rust-lang/rust/issues/43244
        {
            let mut surfaces = surfaces.borrow_mut();
            let mut i = 0;
            while i != surfaces.len() {
                if surfaces[i].1.handle_events() {
                    surfaces.remove(i);
                } else {
                    i += 1;
                }
            }
        }

        display.flush().unwrap();
        event_loop.dispatch(None, &mut ()).unwrap();
    }
}
