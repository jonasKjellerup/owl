use piet::RenderContext;
use piet_common::{Piet, Device};
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

use crate::config;

default_environment!(Env,
    fields = [
        layer_shell: SimpleGlobal<zwlr_layer_shell_v1::ZwlrLayerShellV1>,
    ],
    singles = [
        zwlr_layer_shell_v1::ZwlrLayerShellV1 => layer_shell
    ],
);

use std::cell::{Cell, RefCell};
use std::rc::Rc;

#[derive(PartialEq, Copy, Clone)]
enum RenderEvent {
    Configure { width: u32, height: u32 },
    Closed,
}

struct Bar {
    surface: wl_surface::WlSurface,
    layer_surface: Main<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1>,
    next_render_event: Rc<Cell<Option<RenderEvent>>>,
    pool: AutoMemPool,
    dimensions: (u32, u32),

    device: Device,
}

impl From<config::Bar> for Bar {
    fn from(cfg: config::Bar) -> Self {
        let layer_surface = layer_shell.get_layer_surface(
            &surface,
            Some(output),
            zwlr_layer_shell_v1::Layer::Bottom,
            cfg.name,
        );

        let config::Bar {height, width, .. } = cfg;

        layer_surface.set_size(width, height);

        layer_surface.set_exclusive_zone(match cfg.anchor {
            config::Anchor::Left | config::Anchor::Right => cfg.width,
            config::Anchor::Top | config::Anchor::Bottom => cfg.height,
        });

        layer_surface.set_anchor(cfg.anchor.into());

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

        surface.commit();

        Bar {
            surface,
            layer_surface,
            next_render_event,
            pool,
            dimensions: (width, height),
        }
    }
}

impl Bar {
    fn new(
        output: &wl_output::WlOutput,
        surface: wl_surface::WlSurface,
        layer_shell: &Attached<zwlr_layer_shell_v1::ZwlrLayerShellV1>,
        pool: AutoMemPool,
    ) -> Self {

        let next_render_event = Rc::new(Cell::new(None::<RenderEvent>));
        let next_render_event_handle = Rc::clone(&next_render_event);


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

        for dst_pixel in canvas.chunks_exact_mut(4) {
            let pixel = 0xff00ff00u32.to_ne_bytes();
            dst_pixel[0] = pixel[0];
            dst_pixel[1] = pixel[1];
            dst_pixel[2] = pixel[2];
            dst_pixel[3] = pixel[3];
        }

        // Attach the buffer to the surface and mark the entire surface as damaged
        self.surface.attach(Some(&buffer), 0, 0);
        self.surface.damage_buffer(0, 0, width as i32, height as i32);

        // Finally, commit the surface
        self.surface.commit();
    }
}

impl Drop for Bar {
    fn drop(&mut self) {
        self.layer_surface.destroy();
        self.surface.destroy();
    }
}