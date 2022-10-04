use std::sync::Mutex;

use wayland_client::{
    event_created_child,
    protocol::{
        wl_data_device::{self, WlDataDevice},
        wl_data_offer::{self, WlDataOffer},
        wl_surface::WlSurface,
    },
    Connection, Dispatch, QueueHandle,
};

use super::{
    data_offer::{DataOfferData, DataOfferHandler},
    DataDeviceManagerState,
};

#[derive(Debug, Default)]
pub struct DataDeviceInner {
    pub(super) dnd_data_offer: Option<DnDDataOffer>,
    pub(super) selection: Option<WlDataOffer>,
    serial: u32,
}

#[derive(Debug)]
pub struct DnDDataOffer {
    data_offer: Option<WlDataOffer>,
    serial: u32,
    surface: WlSurface,
    x: f64,
    y: f64,
    time: Option<u32>,
}

impl Drop for DnDDataOffer {
    fn drop(&mut self) {
        self.data_offer.as_mut().map(|offer| offer.destroy());
    }
}

#[derive(Debug, Default)]
pub struct DataDeviceData {
    pub(super) inner: Mutex<DataDeviceInner>,
}

pub trait DataDeviceDataExt: Send + Sync {
    fn data_device_data(&self) -> &DataDeviceData;
}

impl DataDeviceDataExt for DataDeviceData {
    fn data_device_data(&self) -> &DataDeviceData {
        &self
    }
}

/// Handler trait for DataDevice events.
///
/// The functions defined in this trait are called as DataDevice events are received from the compositor.
pub trait DataDeviceHandler: Sized {
    /// Introduces a new data offer
    fn data_offer(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        data_device: &WlDataDevice,
        offer: WlDataOffer,
        serial: u32,
    );

    /// The data device pointer has entered a surface at the provided location
    #[allow(clippy::too_many_arguments)]
    fn enter(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        data_device: &WlDataDevice,
        serial: u32,
        surface: WlSurface,
        x: f64,
        y: f64,
        id: Option<WlDataOffer>,
    );

    /// The drag and drop pointer has left the surface and the session ends
    /// The offer will be destroyed
    fn leave(&mut self, conn: &Connection, qh: &QueueHandle<Self>, data_device: &WlDataDevice);

    /// Drag and Drop motion
    fn motion(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        data_device: &WlDataDevice,
        time: u32,
        x: f64,
        y: f64,
        offer: &WlDataOffer,
    );

    /// Advertises a new selection
    fn selection(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        data_device: &WlDataDevice,
        offer: Option<WlDataOffer>,
    );

    /// drop performed
    /// after the next data offer action event, data may be able to be received, unless the action is "ask"
    fn drop_performed(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        data_device: &WlDataDevice,
        offer: &WlDataOffer,
        serial: u32,
        surface: &WlSurface,
        x: f64,
        y: f64,
        time: Option<u32>,
    );
}

impl<D> Dispatch<wl_data_device::WlDataDevice, DataDeviceData, D> for DataDeviceManagerState
where
    D: Dispatch<wl_data_device::WlDataDevice, DataDeviceData>
        + Dispatch<wl_data_offer::WlDataOffer, DataOfferData>
        + DataDeviceHandler
        + DataOfferHandler
        + 'static,
{
    event_created_child!(D, WlDataDevice, [
        0 => (WlDataOffer, DataOfferData::default())
    ]);

    fn event(
        state: &mut D,
        data_device: &wl_data_device::WlDataDevice,
        event: wl_data_device::Event,
        data: &DataDeviceData,
        conn: &Connection,
        qh: &QueueHandle<D>,
    ) {
        let data = data.data_device_data();
        let mut inner = data.inner.lock().unwrap();

        match event {
            wayland_client::protocol::wl_data_device::Event::DataOffer { id } => {
                let serial = inner.serial;
                inner.serial += 1;
                state.data_offer(conn, qh, data_device, id, serial);
            }
            wayland_client::protocol::wl_data_device::Event::Enter {
                serial,
                surface,
                x,
                y,
                id,
            } => {
                inner.dnd_data_offer.replace(DnDDataOffer {
                    data_offer: id.clone(),
                    serial,
                    surface: surface.clone(),
                    x,
                    y,
                    time: None,
                });
                state.enter(conn, qh, data_device, serial, surface, x, y, id);
            }
            wayland_client::protocol::wl_data_device::Event::Leave => {
                inner.dnd_data_offer.take();
                state.leave(conn, qh, data_device);
            }
            wayland_client::protocol::wl_data_device::Event::Motion {
                time: new_time,
                x: new_x,
                y: new_y,
            } => {
                match inner.dnd_data_offer.as_mut() {
                    Some(DnDDataOffer { x, y, time, data_offer, .. }) => {
                        *x = new_x;
                        *y = new_y;
                        time.replace(new_time);
                        if let Some(offer) = data_offer.as_ref() {
                            state.motion(conn, qh, data_device, new_time, new_x, new_y, offer);
                        }
                    }
                    _ => {} // ignored
                }
            }
            wayland_client::protocol::wl_data_device::Event::Drop => {
                dbg!(&inner.dnd_data_offer);
                match inner.dnd_data_offer.as_ref() {
                    Some(DnDDataOffer { data_offer, serial, surface, x, y, time }) => {
                        let data_offer = match data_offer {
                            Some(data_offer) => data_offer,
                            None => return, // ignored
                        };
                        state.drop_performed(
                            conn,
                            qh,
                            data_device,
                            data_offer,
                            *serial,
                            surface,
                            *x,
                            *y,
                            *time,
                        );
                    }
                    _ => {} // ignored
                }
            }
            wayland_client::protocol::wl_data_device::Event::Selection { id } => {
                match id.clone() {
                    Some(id) => {
                        let old = inner.selection.replace(id.clone());

                        if let Some(old) = old {
                            old.destroy();
                        }
                    }
                    None => {
                        if let Some(old) = inner.selection.take() {
                            old.destroy();
                        }
                    }
                }
                state.selection(conn, qh, data_device, id);
            }
            _ => unreachable!(),
        }
    }
}
