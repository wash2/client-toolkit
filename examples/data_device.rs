use std::{
    convert::TryInto,
    fs::File,
    io::{Read, Write},
    time::Duration,
};

use calloop::{LoopHandle, RegistrationToken};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    data_device::{
        data_device::DataDeviceHandler,
        data_offer::{receive, DataOfferHandler},
        data_source::{CopyPasteSource, DataSourceHandler, DragSource},
        DataDeviceManagerHandler, DataDeviceManagerState,
    },
    delegate_compositor, delegate_data_device_manager, delegate_keyboard, delegate_output,
    delegate_pointer, delegate_registry, delegate_seat, delegate_shm, delegate_xdg_shell,
    delegate_xdg_window,
    event_loop::WaylandSource,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        keyboard::{KeyEvent, KeyboardHandler, Modifiers},
        pointer::{PointerEvent, PointerEventKind, PointerHandler, BTN_LEFT},
        Capability, SeatHandler, SeatState,
    },
    shell::xdg::{
        window::{Window, WindowConfigure, WindowHandler, XdgWindowState},
        XdgShellHandler, XdgShellState,
    },
    shm::{
        slot::{Buffer, SlotPool},
        ShmHandler, ShmState,
    },
};
use wayland_client::{
    protocol::{
        wl_data_device::WlDataDevice,
        wl_data_device_manager::DndAction,
        wl_data_offer::WlDataOffer,
        wl_keyboard::{self, WlKeyboard},
        wl_output,
        wl_pointer::{self, WlPointer},
        wl_seat::{self, WlSeat},
        wl_shm, wl_surface,
    },
    Connection, QueueHandle, WEnum,
};

fn main() {
    env_logger::init();

    let conn = Connection::connect_to_env().unwrap();
    let event_queue = conn.new_event_queue();
    let qh = event_queue.handle();
    let mut event_loop = calloop::EventLoop::try_new().unwrap();
    WaylandSource::new(event_queue).unwrap().insert(event_loop.handle()).unwrap();

    let mut simple_window = SimpleWindow {
        registry_state: RegistryState::new(&conn, &qh),
        seat_state: SeatState::new(),
        output_state: OutputState::new(),
        compositor_state: CompositorState::new(),
        shm_state: ShmState::new(),
        xdg_shell_state: XdgShellState::new(),
        xdg_window_state: XdgWindowState::new(),
        data_device_manager_state: DataDeviceManagerState::new(),

        exit: false,
        first_configure: true,
        pool: None,
        width: 256,
        height: 256,
        shift: None,
        buffer: None,
        window: None,
        keyboard: None,
        keyboard_focus: false,
        pointer: None,
        offers: Vec::new(),
        data_devices: Vec::new(),
        copy_paste_sources: Vec::new(),
        drag_sources: Vec::new(),
        loop_handle: event_loop.handle(),
    };

    while !simple_window.registry_state.ready() {
        event_loop.dispatch(Duration::from_millis(30), &mut simple_window).unwrap();
    }

    let pool = SlotPool::new(
        simple_window.width as usize * simple_window.height as usize * 4,
        &simple_window.shm_state,
    )
    .expect("Failed to create pool");
    simple_window.pool = Some(pool);

    let surface = simple_window.compositor_state.create_surface(&qh).unwrap();

    let window = Window::builder()
        .title("A wayland window")
        // GitHub does not let projects use the `org.github` domain but the `io.github` domain is fine.
        .app_id("io.github.smithay.client-toolkit.SimpleWindow")
        .min_size((256, 256))
        .map(&qh, &simple_window.xdg_shell_state, &mut simple_window.xdg_window_state, surface)
        .expect("window creation");

    simple_window.window = Some(window);

    // We don't draw immediately, the configure will notify us when to first draw.

    loop {
        event_loop.dispatch(Duration::from_millis(30), &mut simple_window).unwrap();

        if simple_window.exit {
            println!("exiting example");
            break;
        }
    }
}

struct SimpleWindow {
    registry_state: RegistryState,
    seat_state: SeatState,
    output_state: OutputState,
    compositor_state: CompositorState,
    shm_state: ShmState,
    xdg_shell_state: XdgShellState,
    xdg_window_state: XdgWindowState,
    data_device_manager_state: DataDeviceManagerState,

    exit: bool,
    first_configure: bool,
    pool: Option<SlotPool>,
    width: u32,
    height: u32,
    shift: Option<u32>,
    buffer: Option<Buffer>,
    window: Option<Window>,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    keyboard_focus: bool,
    pointer: Option<wl_pointer::WlPointer>,
    offers: Vec<(WlDataOffer, u32, Vec<String>, String, Option<RegistrationToken>)>,
    data_devices: Vec<(WlSeat, Option<WlKeyboard>, Option<WlPointer>, WlDataDevice)>,
    copy_paste_sources: Vec<CopyPasteSource>,
    drag_sources: Vec<(DragSource, bool)>,
    loop_handle: LoopHandle<'static, SimpleWindow>,
}

impl CompositorHandler for SimpleWindow {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.compositor_state
    }

    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_factor: i32,
    ) {
        // Not needed for this example.
    }

    fn frame(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
        self.draw(conn, qh);
    }
}

impl OutputHandler for SimpleWindow {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn update_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }
}

impl XdgShellHandler for SimpleWindow {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }
}

impl WindowHandler for SimpleWindow {
    fn xdg_window_state(&mut self) -> &mut XdgWindowState {
        &mut self.xdg_window_state
    }

    fn request_close(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &Window) {
        self.exit = true;
    }

    fn configure(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        _window: &Window,
        configure: WindowConfigure,
        _serial: u32,
    ) {
        match configure.new_size {
            Some(size) => {
                self.width = size.0;
                self.height = size.1;
                self.buffer = None;
            }
            None => {
                self.width = 256;
                self.height = 256;
                self.buffer = None;
            }
        }

        // Initiate the first draw.
        if self.first_configure {
            self.first_configure = false;
            self.draw(conn, qh);
        }
    }
}

impl SeatHandler for SimpleWindow {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }

    fn new_seat(&mut self, _: &Connection, _qh: &QueueHandle<Self>, _seat: wl_seat::WlSeat) {}

    fn new_capability(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        let data_device =
            if let Some(data_device) = self.data_devices.iter_mut().find(|(s, ..)| s == &seat) {
                data_device
            } else {
                // create the data device here for this seat
                let data_device_manager = &self.data_device_manager_state;
                let data_device = data_device_manager.get_data_device(qh, &seat).unwrap();
                self.data_devices.push((seat.clone(), None, None, data_device));
                self.data_devices.last_mut().unwrap()
            };
        if capability == Capability::Keyboard && self.keyboard.is_none() {
            println!("Set keyboard capability");
            let keyboard =
                self.seat_state.get_keyboard(qh, &seat, None).expect("Failed to create keyboard");
            self.keyboard = Some(keyboard.clone());
            data_device.1.replace(keyboard);
        }

        if capability == Capability::Pointer && self.pointer.is_none() {
            println!("Set pointer capability");
            let pointer = self.seat_state.get_pointer(qh, &seat).expect("Failed to create pointer");
            self.pointer = Some(pointer.clone());
            data_device.2.replace(pointer);
        }
    }

    fn remove_capability(
        &mut self,
        _conn: &Connection,
        _: &QueueHandle<Self>,
        _: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Keyboard && self.keyboard.is_some() {
            println!("Unset keyboard capability");
            self.keyboard.take().unwrap().release();
        }

        if capability == Capability::Pointer && self.pointer.is_some() {
            println!("Unset pointer capability");
            self.pointer.take().unwrap().release();
        }
    }

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl KeyboardHandler for SimpleWindow {
    fn enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        surface: &wl_surface::WlSurface,
        _: u32,
        _: &[u32],
        _keysyms: &[u32],
    ) {
        if self.window.as_ref().map(Window::wl_surface) == Some(surface) {
            self.keyboard_focus = true;
        }
    }

    fn leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        surface: &wl_surface::WlSurface,
        _: u32,
    ) {
        if self.window.as_ref().map(Window::wl_surface) == Some(surface) {
            self.keyboard_focus = false;
        }
    }

    fn press_key(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        kbd: &wl_keyboard::WlKeyboard,
        serial: u32,
        event: KeyEvent,
    ) {
        match event.utf8 {
            Some(s) if s.to_lowercase() == "c" => {
                println!("Creating copy paste source and setting selection...");
                if let Some(data_device) =
                    self.data_devices.iter().find(|(_, d_kbd, ..)| d_kbd.as_ref() == Some(&kbd))
                {
                    let source = self
                        .data_device_manager_state
                        .create_copy_paste_source(qh, vec!["text/plain"])
                        .unwrap();
                    source.set_selection(&data_device.3, serial);
                    self.copy_paste_sources.push(source);
                }
            }
            Some(s) => {
                dbg!(s);
            }
            _ => {}
        };
    }

    fn release_key(
        &mut self,
        _: &Connection,
        _qh: &QueueHandle<Self>,
        _kbd: &wl_keyboard::WlKeyboard,
        _serial: u32,
        _event: KeyEvent,
    ) {
    }

    fn update_modifiers(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _serial: u32,
        _modifiers: Modifiers,
    ) {
    }
}

impl PointerHandler for SimpleWindow {
    fn pointer_frame(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        pointer: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        use PointerEventKind::*;
        for event in events {
            // Ignore events for other surfaces
            if Some(&event.surface) != self.window.as_ref().map(Window::wl_surface) {
                continue;
            }
            let surface = event.surface.clone();
            match event.kind {
                Press { button, serial, .. } if button == BTN_LEFT => {
                    if let Some(data_device) = self
                        .data_devices
                        .iter()
                        .find(|(_, _, d_pointer, ..)| d_pointer.as_ref() == Some(&pointer))
                    {
                        self.shift = self.shift.xor(Some(0));
                        let source = self
                            .data_device_manager_state
                            .create_drag_and_drop_source(qh, vec!["text/plain"], DndAction::Copy)
                            .unwrap();

                        source.start_drag(&data_device.3, &surface, None, serial);
                        self.drag_sources.push((source, false));
                    }
                }
                _ => {}
            }
        }
    }
}

impl ShmHandler for SimpleWindow {
    fn shm_state(&mut self) -> &mut ShmState {
        &mut self.shm_state
    }
}

impl SimpleWindow {
    pub fn draw(&mut self, _conn: &Connection, qh: &QueueHandle<Self>) {
        if let Some(window) = self.window.as_ref() {
            let width = self.width;
            let height = self.height;
            let stride = self.width as i32 * 4;
            let pool = self.pool.as_mut().unwrap();

            let buffer = self.buffer.get_or_insert_with(|| {
                pool.create_buffer(width as i32, height as i32, stride, wl_shm::Format::Argb8888)
                    .expect("create buffer")
                    .0
            });

            let canvas = match pool.canvas(buffer) {
                Some(canvas) => canvas,
                None => {
                    // This should be rare, but if the compositor has not released the previous
                    // buffer, we need double-buffering.
                    let (second_buffer, canvas) = pool
                        .create_buffer(
                            self.width as i32,
                            self.height as i32,
                            stride,
                            wl_shm::Format::Argb8888,
                        )
                        .expect("create buffer");
                    *buffer = second_buffer;
                    canvas
                }
            };

            // Draw to the window:
            {
                let shift = self.shift.unwrap_or(0);
                canvas.chunks_exact_mut(4).enumerate().for_each(|(index, chunk)| {
                    let x = ((index + shift as usize) % width as usize) as u32;
                    let y = (index / width as usize) as u32;

                    let a = 0xFF;
                    let r = u32::min(((width - x) * 0xFF) / width, ((height - y) * 0xFF) / height);
                    let g = u32::min((x * 0xFF) / width, ((height - y) * 0xFF) / height);
                    let b = u32::min(((width - x) * 0xFF) / width, (y * 0xFF) / height);
                    let color = (a << 24) + (r << 16) + (g << 8) + b;

                    let array: &mut [u8; 4] = chunk.try_into().unwrap();
                    *array = color.to_le_bytes();
                });

                if let Some(shift) = &mut self.shift {
                    *shift = (*shift + 1) % width;
                }
            }

            // Damage the entire window
            window.wl_surface().damage_buffer(0, 0, self.width as i32, self.height as i32);

            // Request our next frame
            window.wl_surface().frame(qh, window.wl_surface().clone());

            // Attach and commit to present.
            buffer.attach_to(window.wl_surface()).expect("buffer attach");
            window.wl_surface().commit();
        }
    }
}

impl DataDeviceHandler for SimpleWindow {
    fn data_offer(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _data_device: &wayland_client::protocol::wl_data_device::WlDataDevice,
        offer: wayland_client::protocol::wl_data_offer::WlDataOffer,
        serial: u32,
    ) {
        dbg!(&offer);
        self.offers.push((offer, serial, Vec::new(), String::new(), None));
    }

    fn enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _data_device: &wayland_client::protocol::wl_data_device::WlDataDevice,
        serial: u32,
        _surface: wl_surface::WlSurface,
        _x: f64,
        _y: f64,
        mut offer: Option<wayland_client::protocol::wl_data_offer::WlDataOffer>,
    ) {
        dbg!(_x, _y);
        if let Some((offer, offer_serial, mime_types, ..)) =
            self.offers.iter_mut().find(|(o, ..)| Some(o) == offer.as_ref())
        {
            for mime in mime_types {
                offer.accept(serial, Some(mime.clone()));
            }
            *offer_serial = serial;
        } else if let Some(offer) = offer.take() {
            self.offers.push((offer, serial, Vec::new(), String::new(), None));
        }
    }

    fn leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _data_device: &wayland_client::protocol::wl_data_device::WlDataDevice,
    ) {
        println!("data offer left");
    }

    fn motion(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _data_device: &wayland_client::protocol::wl_data_device::WlDataDevice,
        _time: u32,
        _x: f64,
        _y: f64,
        _offer: &WlDataOffer,
    ) {
        dbg!((_time, _x, _y));
    }

    fn selection(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _data_device: &wayland_client::protocol::wl_data_device::WlDataDevice,
        offer: Option<wayland_client::protocol::wl_data_offer::WlDataOffer>,
    ) {
        if let Some((offer, mime_type, tracked_token)) = self
            .offers
            .iter_mut()
            .find(|(o, ..)| Some(o) == offer.as_ref())
            .and_then(|o| (o.2.get(0).map(|mime| (&o.0, mime, &mut o.4))))
        {
            if let Ok(read_pipe) = receive(offer, mime_type.clone()) {
                let offer_clone = offer.clone();
                match self.loop_handle.insert_source(read_pipe, move |_, f, state| {
                    let (_, _, _, mut contents, token) = state
                        .offers
                        .iter()
                        .position(|o| o.0 == offer_clone)
                        .map(|p| state.offers.remove(p))
                        .unwrap();

                    f.read_to_string(&mut contents).unwrap();
                    println!("TEXT FROM Selection: {contents}");
                    state.loop_handle.remove(token.unwrap());
                    offer_clone.finish();
                    offer_clone.destroy();
                }) {
                    Ok(token) => {
                        tracked_token.replace(token);
                    }
                    Err(err) => {
                        eprintln!("{:?}", err);
                    }
                }
            }
        }
    }

    fn drop_performed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _data_device: &wayland_client::protocol::wl_data_device::WlDataDevice,
        offer: &wayland_client::protocol::wl_data_offer::WlDataOffer,
        _serial: u32,
        _surface: &wl_surface::WlSurface,
        _x: f64,
        _y: f64,
        _time: Option<u32>,
    ) {
        dbg!((&offer, _serial, _surface, _x, _y, _time));
        if let Some((mime_type, tracked_token)) = self
            .offers
            .iter_mut()
            .find(|(o, ..)| o == offer)
            .and_then(|o| o.2.get(0).map(|mime| (mime, &mut o.4)))
        {
            if let Ok(read_pipe) = receive(offer, mime_type.clone()) {
                let offer_clone = offer.clone();
                match self.loop_handle.insert_source(read_pipe, move |_, f, state| {
                    let (_, _, _, mut contents, token) = state
                        .offers
                        .iter()
                        .position(|o| o.0 == offer_clone)
                        .map(|p| state.offers.remove(p))
                        .unwrap();

                    f.read_to_string(&mut contents).unwrap();
                    println!("TEXT FROM DROP: {contents}");
                    state.loop_handle.remove(token.unwrap());
                    offer_clone.finish();
                    offer_clone.destroy();
                }) {
                    Ok(token) => {
                        tracked_token.replace(token);
                    }
                    Err(err) => {
                        eprintln!("{:?}", err);
                    }
                }
            }
        }
    }
}

impl DataOfferHandler for SimpleWindow {
    fn offer(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        offer: &wayland_client::protocol::wl_data_offer::WlDataOffer,
        mime_type: String,
    ) {
        if let Some((_, serial, mime_types, ..)) = self.offers.iter_mut().find(|(o, ..)| o == offer)
        {
            if mime_type == "text/plain" {
                offer.accept(*serial, Some(mime_type.clone()));
                mime_types.push(mime_type);
            }
        } else {
            self.offers.push((offer.clone(), 0, vec![mime_type], String::new(), None));
        }
    }

    fn source_actions(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _offer: &wayland_client::protocol::wl_data_offer::WlDataOffer,
        actions: wayland_client::WEnum<wayland_client::protocol::wl_data_device_manager::DndAction>,
    ) {
        dbg!(actions);
        match actions {
            WEnum::Value(actions) => {
                _offer.set_actions(actions, actions.intersection(DndAction::Copy))
            }
            WEnum::Unknown(_) => {}
        }
    }

    fn actions(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _offer: &wayland_client::protocol::wl_data_offer::WlDataOffer,
        actions: wayland_client::WEnum<wayland_client::protocol::wl_data_device_manager::DndAction>,
    ) {
        dbg!(actions);
    }
}

impl DataSourceHandler for SimpleWindow {
    fn accept_mime(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _source: &wayland_client::protocol::wl_data_source::WlDataSource,
        _mime: Option<String>,
    ) {
    }

    fn send(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        source: &wayland_client::protocol::wl_data_source::WlDataSource,
        mime: String,
        fd: wayland_backend::io_lifetimes::OwnedFd,
    ) {
        dbg!(&self.drag_sources);

        if let Some(_) = self
            .copy_paste_sources
            .iter_mut()
            .find(|s| s.inner() == source && mime == "text/plain".to_string())
        {
            let mut f = File::from(fd);
            writeln!(f, "Copied from selection via sctk").unwrap();
        } else if let Some(_) = self
            .drag_sources
            .iter_mut()
            .find(|s| s.0.inner() == source && mime == "text/plain".to_string() && s.1)
        {
            let mut f = File::from(fd);
            writeln!(f, "Dropped via sctk").unwrap();
        }
    }

    fn cancelled(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        source: &wayland_client::protocol::wl_data_source::WlDataSource,
    ) {
        self.copy_paste_sources
            .iter()
            .position(|s| s.inner() == source)
            .map(|pos| self.copy_paste_sources.remove(pos));
        source.destroy();
    }

    fn drop_performed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _source: &wayland_client::protocol::wl_data_source::WlDataSource,
    ) {
        println!("DROP PERFORMED");
    }

    fn dnd_finished(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        source: &wayland_client::protocol::wl_data_source::WlDataSource,
    ) {
        self.copy_paste_sources.iter().position(|s| s.inner() == source).map(|pos| {
            self.copy_paste_sources.remove(pos);
        });
        source.destroy();
    }

    fn action(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        source: &wayland_client::protocol::wl_data_source::WlDataSource,
        action: wayland_client::protocol::wl_data_device_manager::DndAction,
    ) {
        if let Some(source) = self.drag_sources.iter_mut().find(|s| s.0.inner() == source) {
            source.1 = action.contains(DndAction::Copy);
        }
    }
}

impl DataDeviceManagerHandler for SimpleWindow {
    fn data_device_manager_state(&mut self) -> &mut DataDeviceManagerState {
        &mut self.data_device_manager_state
    }
}

delegate_compositor!(SimpleWindow);
delegate_output!(SimpleWindow);
delegate_shm!(SimpleWindow);

delegate_seat!(SimpleWindow);
delegate_keyboard!(SimpleWindow);
delegate_pointer!(SimpleWindow);

delegate_xdg_shell!(SimpleWindow);
delegate_xdg_window!(SimpleWindow);

delegate_data_device_manager!(SimpleWindow);

delegate_registry!(SimpleWindow);

impl ProvidesRegistryState for SimpleWindow {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![
        CompositorState,
        OutputState,
        ShmState,
        SeatState,
        XdgShellState,
        XdgWindowState,
        DataDeviceManagerState,
    ];
}
