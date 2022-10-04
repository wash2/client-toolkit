use wayland_backend::io_lifetimes::OwnedFd;
use wayland_client::{
    protocol::{
        wl_data_device::WlDataDevice,
        wl_data_device_manager::DndAction,
        wl_data_source::{self, WlDataSource},
        wl_surface::WlSurface,
    },
    Connection, Dispatch, QueueHandle, WEnum,
};

use super::DataDeviceManagerState;

#[derive(Debug, Default)]
pub struct DataSourceData {}

pub trait DataSourceDataExt: Send + Sync {
    fn data_source_data(&self) -> &DataSourceData;
}

impl DataSourceDataExt for DataSourceData {
    fn data_source_data(&self) -> &DataSourceData {
        &self
    }
}

/// Handler trait for DataSource events.
///
/// The functions defined in this trait are called as DataSource events are received from the compositor.
pub trait DataSourceHandler: Sized {
    /// The accepted mime type from the destination, if any
    fn accept_mime(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        source: &WlDataSource,
        mime: Option<String>,
    );

    /// Request to send data from the client.
    /// Send the data, then close the fd.
    fn send(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        source: &WlDataSource,
        mime: String,
        fd: OwnedFd,
    );

    /// The data source is no longer valid
    fn cancelled(&mut self, conn: &Connection, qh: &QueueHandle<Self>, source: &WlDataSource);

    /// A drop was performed
    /// the data source will be used and should not be destroyed yet
    fn drop_performed(&mut self, conn: &Connection, qh: &QueueHandle<Self>, source: &WlDataSource);

    /// the dnd finished
    /// the data source may be sestroyed
    fn dnd_finished(&mut self, conn: &Connection, qh: &QueueHandle<Self>, source: &WlDataSource);

    /// an action was selected by the compositor
    fn action(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        source: &WlDataSource,
        action: DndAction,
    );
}

impl<D> Dispatch<wl_data_source::WlDataSource, DataSourceData, D> for DataDeviceManagerState
where
    D: Dispatch<wl_data_source::WlDataSource, DataSourceData> + DataSourceHandler,
{
    fn event(
        state: &mut D,
        source: &wl_data_source::WlDataSource,
        event: <wl_data_source::WlDataSource as wayland_client::Proxy>::Event,
        _data: &DataSourceData,
        conn: &wayland_client::Connection,
        qh: &wayland_client::QueueHandle<D>,
    ) {
        match event {
            wl_data_source::Event::Target { mime_type } => {
                state.accept_mime(conn, qh, source, mime_type)
            }
            wl_data_source::Event::Send { mime_type, fd } => {
                state.send(conn, qh, source, mime_type, fd);
            }
            wl_data_source::Event::Cancelled => {
                source.destroy();
                state.cancelled(conn, qh, source);
            }
            wl_data_source::Event::DndDropPerformed => {
                state.drop_performed(conn, qh, source);
            }
            wl_data_source::Event::DndFinished => {
                state.dnd_finished(conn, qh, source);
            }
            wl_data_source::Event::Action { dnd_action } => match dnd_action {
                WEnum::Value(dnd_action) => {
                    state.action(conn, qh, source, dnd_action);
                }
                WEnum::Unknown(_) => {}
            },
            _ => unimplemented!(),
        };
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct CopyPasteSource {
    pub(crate) inner: WlDataSource,
    pub(crate) serial: Option<u32>,
}

impl CopyPasteSource {
    /// set the selection
    /// internally tracks the serial for when unset selection may be called
    pub fn set_selection(&self, device: &WlDataDevice, serial: u32) {
        device.set_selection(Some(&self.inner), serial);
    }

    /// unset the selection
    pub fn unset_selection(&self, device: &WlDataDevice) {
        if let Some(serial) = self.serial {
            device.set_selection(None, serial);
        }
    }

    pub fn inner(&self) -> &WlDataSource {
        &self.inner
    }
}

impl Drop for CopyPasteSource {
    fn drop(&mut self) {
        self.inner.destroy();
    }
}

#[derive(Debug)]
pub struct DragSource {
    pub(crate) inner: WlDataSource,
}

impl DragSource {
    /// start a drag and drop operation
    /// the drag is cancelled when the DragSource is dropped
    pub fn start_drag(
        &self,
        device: &WlDataDevice,
        origin: &WlSurface,
        icon: Option<&WlSurface>,
        serial: u32,
    ) {
        device.start_drag(Some(&self.inner), origin, icon, serial);
    }

    /// start an internal draf and drop operation
    /// This will pass a NULL source, and the client is expected to handle data passing internally.
    /// Only Enter, Leave, & Motion events will be sent to the client
    pub fn start_internal_drag(
        device: &WlDataDevice,
        origin: &WlSurface,
        icon: Option<&WlSurface>,
        serial: u32,
    ) {
        device.start_drag(None, origin, icon, serial);
    }

    pub fn inner(&self) -> &WlDataSource {
        &self.inner
    }
}

impl Drop for DragSource {
    fn drop(&mut self) {
        self.inner.destroy();
    }
}
