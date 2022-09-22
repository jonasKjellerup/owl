use std::cell::{Cell, RefCell};
use std::marker::PhantomData;
use std::rc::Rc;
use piet_common::CairoRenderContext;
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
use smithay_client_toolkit::environment::Environment;
use smithay_client_toolkit::output::OutputStatusListener;
use smithay_client_toolkit::reexports::client::{Display, EventQueue};
use crate::error::{Result, Error, Kind};
use crate::LoopHandle;

default_environment!(Env,
    fields = [
        layer_shell: SimpleGlobal<zwlr_layer_shell_v1::ZwlrLayerShellV1>,
    ],
    singles = [
        zwlr_layer_shell_v1::ZwlrLayerShellV1 => layer_shell
    ],
);

/// A wrapper structure intended to facilitate drawing directly
/// to the wayland surface buffer.
struct PietWaylandSurface<'a> {
    image_surface: cairo::ImageSurface,
    context: cairo::Context,
    phantom: PhantomData<&'a ()>,
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

    fn get_context(&mut self) -> CairoRenderContext {
        CairoRenderContext::new(&self.context)
    }
}

#[derive(PartialEq, Copy, Clone)]
enum SurfaceEvent {
    Configure { width: u32, height: u32 },
    Closed,
}

pub enum SurfaceAction {
    Redraw,
    Drop,
    None,
}

pub struct Surface {
    surface: wl_surface::WlSurface,
    layer_surface: Main<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1>,
    next_event: Rc<Cell<Option<SurfaceEvent>>>,
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
            zwlr_layer_shell_v1::Layer::Top,
            "owl-rs".to_owned(),
        );

        let next_event = Rc::new(Cell::new(None::<SurfaceEvent>));
        let next_event_handle = Rc::clone(&next_event);

        layer_surface.quick_assign(move |layer_surface, event, _| {
            match (event, next_event_handle.get()) {
                (zwlr_layer_surface_v1::Event::Closed, _) => {
                    next_event_handle.set(Some(SurfaceEvent::Closed));
                }
                (zwlr_layer_surface_v1::Event::Configure { serial, width, height }, next)
                if next != Some(SurfaceEvent::Closed) =>
                    {
                        layer_surface.ack_configure(serial);
                        next_event_handle.set(Some(SurfaceEvent::Configure { width, height }));
                    }
                (_, _) => {}
            }
        });

        // Commit so that the server will send a configure event
        surface.commit();

        Self { surface, layer_surface, next_event, pool, dimensions: (0, 0) }
    }

    pub fn configure(&mut self) {
        todo!()
    }

    pub fn handle_events(&mut self) -> SurfaceAction {
        todo!();
        SurfaceAction::None
    }

}

impl Drop for Surface {
    fn drop(&mut self) {
        self.layer_surface.destroy();
        self.surface.destroy();
    }
}

/// Intermediary representation of a `WaylandContext` that has not yet had
/// its event queue inserted into an event loop.
pub struct UnhandledWaylandContext(WaylandContext, EventQueue);

impl UnhandledWaylandContext {
    pub fn insert_queue_in(self, handle: LoopHandle) -> Result<WaylandContext> {
        WaylandSource::new(self.1)
            .quick_insert(handle)
            .map(|_| self.0)
            .map_err(|err| Error::new(Kind::WaylandError)
                .with_msg(format!("Unable to insert WaylandSource into EventLoop. Error: {}", err)))
    }
}

pub struct WaylandContext {
    pub surfaces: Rc<RefCell<Vec<(u32, Surface)>>>,
    display: Display,
    env: Environment<Env>,
    output_listener_handle: OutputStatusListener,
}

impl WaylandContext {
    pub fn new() -> Result<UnhandledWaylandContext> {
        let (env, display, queue) =
            new_default_environment!(Env, fields = [layer_shell: SimpleGlobal::new(), ])
                .expect("Initial roundtrip failed!");

        let surfaces = Rc::new(RefCell::new(Vec::new()));
        let layer_shell = env.require_global::<zwlr_layer_shell_v1::ZwlrLayerShellV1>();

        // Create Rc handles for the output handler
        let env_handle = env.clone();
        let surfaces_handle = surfaces.clone();

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

        let output_listener_handle
            = env.listen_for_outputs(move |output, info, _| output_handler(output, info));

        Ok(UnhandledWaylandContext(
            WaylandContext {
                surfaces,
                env,
                output_listener_handle,
                display,
            },
            queue,
        ))
    }

    pub fn flush_display(&self) -> Result<()> {
        self.display.flush().map_err(|err| err.into())
    }
}