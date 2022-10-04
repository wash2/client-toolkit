pub mod data_device;
pub mod data_offer;
pub mod data_source;

use std::{
    fs, io,
    os::unix::prelude::{AsRawFd, FromRawFd, IntoRawFd, RawFd},
};

use wayland_client::{
    protocol::{
        wl_data_device,
        wl_data_device_manager::{self, DndAction, WlDataDeviceManager},
        wl_data_source::WlDataSource,
        wl_seat::WlSeat,
    },
    Connection, Dispatch, QueueHandle,
};

use crate::{
    error::GlobalError,
    globals::GlobalData,
    registry::{GlobalProxy, ProvidesRegistryState, RegistryHandler},
};

use self::{
    data_device::{DataDeviceData, DataDeviceDataExt},
    data_source::{CopyPasteSource, DataSourceData, DataSourceDataExt, DragSource},
};

#[derive(Debug)]
pub struct DataDeviceManagerState {
    manager: GlobalProxy<WlDataDeviceManager>,
}

impl DataDeviceManagerState {
    pub fn new() -> Self {
        Self { manager: GlobalProxy::new() }
    }

    pub fn data_device_manager(&self) -> Result<&WlDataDeviceManager, GlobalError> {
        self.manager.get()
    }

    pub fn create_copy_paste_source<D>(
        &self,
        qh: &QueueHandle<D>,
        mime_types: Vec<&str>,
    ) -> Result<CopyPasteSource, GlobalError>
    where
        D: Dispatch<WlDataSource, DataSourceData> + 'static,
    {
        self.create_data_source(qh, mime_types, None)
            .map(|src| CopyPasteSource { inner: src, serial: None })
    }

    pub fn create_drag_and_drop_source<D>(
        &self,
        qh: &QueueHandle<D>,
        mime_types: Vec<&str>,
        dnd_actions: DndAction,
    ) -> Result<DragSource, GlobalError>
    where
        D: Dispatch<WlDataSource, DataSourceData> + 'static,
    {
        self.create_data_source(qh, mime_types, Some(dnd_actions))
            .map(|src| DragSource { inner: src })
    }

    /// creates a data source
    /// WlDataDevice::set_selection to actually copy data to the selection,
    fn create_data_source<D>(
        &self,
        qh: &QueueHandle<D>,
        mime_types: Vec<&str>,
        dnd_actions: Option<DndAction>,
    ) -> Result<WlDataSource, GlobalError>
    where
        D: Dispatch<WlDataSource, DataSourceData> + 'static,
    {
        self.create_data_source_with_data(qh, Default::default()).map(|selection| {
            for mime in mime_types {
                selection.offer(mime.to_string());
            }
            if let Some(dnd_actions) = dnd_actions {
                selection.set_actions(dnd_actions);
            }
            selection
        })
    }

    pub fn create_data_source_with_data<D, U>(
        &self,
        qh: &QueueHandle<D>,
        data: U,
    ) -> Result<WlDataSource, GlobalError>
    where
        D: Dispatch<WlDataSource, U> + 'static,
        U: DataSourceDataExt + 'static,
    {
        let manager = self.manager.get()?;

        let data_source = manager.create_data_source(qh, data);

        Ok(data_source)
    }

    pub fn get_data_device<D>(
        &self,
        qh: &QueueHandle<D>,
        seat: &WlSeat,
    ) -> Result<wl_data_device::WlDataDevice, GlobalError>
    where
        D: Dispatch<wl_data_device::WlDataDevice, DataDeviceData> + 'static,
    {
        self.get_data_device_with_data(qh, seat, Default::default())
    }

    pub fn get_data_device_with_data<D, U>(
        &self,
        qh: &QueueHandle<D>,
        seat: &WlSeat,
        data: U,
    ) -> Result<wl_data_device::WlDataDevice, GlobalError>
    where
        D: Dispatch<wl_data_device::WlDataDevice, U> + 'static,
        U: DataDeviceDataExt + 'static,
    {
        let manager = self.manager.get()?;

        let data_device = manager.get_data_device(seat, qh, data);

        Ok(data_device)
    }
}

pub trait DataDeviceManagerHandler: Sized {
    fn data_device_manager_state(&mut self) -> &mut DataDeviceManagerState;
}

impl<D> RegistryHandler<D> for DataDeviceManagerState
where
    D: Dispatch<wl_data_device_manager::WlDataDeviceManager, GlobalData>
        + DataDeviceManagerHandler
        + ProvidesRegistryState
        + 'static,
{
    fn ready(state: &mut D, _conn: &Connection, qh: &QueueHandle<D>) {
        let manager = state.registry().bind_one(qh, 1..=3, GlobalData);

        state.data_device_manager_state().manager = manager.into();
    }
}

impl<D> Dispatch<wl_data_device_manager::WlDataDeviceManager, GlobalData, D>
    for DataDeviceManagerState
where
    D: Dispatch<wl_data_device_manager::WlDataDeviceManager, GlobalData> + DataDeviceManagerHandler,
{
    fn event(
        _state: &mut D,
        _proxy: &wl_data_device_manager::WlDataDeviceManager,
        event: <wl_data_device_manager::WlDataDeviceManager as wayland_client::Proxy>::Event,
        _data: &GlobalData,
        _conn: &Connection,
        _qhandle: &QueueHandle<D>,
    ) {
        match event {
            _ => unimplemented!(),
        }
    }
}

#[macro_export]
macro_rules! delegate_data_device_manager {
    ($(@<$( $lt:tt $( : $clt:tt $(+ $dlt:tt )* )? ),+>)? $ty: ty) => {
        $crate::reexports::client::delegate_dispatch!($(@< $( $lt $( : $clt $(+ $dlt )* )? ),+ >)? $ty:
            [
                $crate::reexports::client::protocol::wl_data_device_manager::WlDataDeviceManager: $crate::globals::GlobalData
            ] => $crate::data_device::DataDeviceManagerState
        );
        $crate::reexports::client::delegate_dispatch!($(@< $( $lt $( : $clt $(+ $dlt )* )? ),+ >)? $ty:
            [
                $crate::reexports::client::protocol::wl_data_source::WlDataSource: $crate::data_device::data_source::DataSourceData
            ] => $crate::data_device::DataDeviceManagerState
        );
        $crate::reexports::client::delegate_dispatch!($(@< $( $lt $( : $clt $(+ $dlt )* )? ),+ >)? $ty:
            [
                $crate::reexports::client::protocol::wl_data_offer::WlDataOffer: $crate::data_device::data_offer::DataOfferData
            ] => $crate::data_device::DataDeviceManagerState
        );
        $crate::reexports::client::delegate_dispatch!($(@< $( $lt $( : $clt $(+ $dlt )* )? ),+ >)? $ty:
            [
                $crate::reexports::client::protocol::wl_data_device::WlDataDevice: $crate::data_device::data_device::DataDeviceData
            ] => $crate::data_device::DataDeviceManagerState
        );
    };
}

/// A file descriptor that can only be read from
///
/// If the `calloop` cargo feature is enabled, this can be used
/// as an `EventSource` in a calloop event loop.
#[derive(Debug)]
pub struct ReadPipe {
    #[cfg(feature = "calloop")]
    file: calloop::generic::Generic<fs::File>,
    #[cfg(not(feature = "calloop"))]
    file: fs::File,
}

#[cfg(feature = "calloop")]
impl io::Read for ReadPipe {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.file.file.read(buf)
    }
}

#[cfg(not(feature = "calloop"))]
impl io::Read for ReadPipe {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.file.read(buf)
    }
}

#[cfg(feature = "calloop")]
impl FromRawFd for ReadPipe {
    unsafe fn from_raw_fd(fd: RawFd) -> ReadPipe {
        ReadPipe {
            file: calloop::generic::Generic::new(
                unsafe { FromRawFd::from_raw_fd(fd) },
                calloop::Interest::READ,
                calloop::Mode::Level,
            ),
        }
    }
}

#[cfg(not(feature = "calloop"))]
impl FromRawFd for ReadPipe {
    unsafe fn from_raw_fd(fd: RawFd) -> ReadPipe {
        ReadPipe { file: FromRawFd::from_raw_fd(fd) }
    }
}

#[cfg(feature = "calloop")]
impl AsRawFd for ReadPipe {
    fn as_raw_fd(&self) -> RawFd {
        self.file.file.as_raw_fd()
    }
}

#[cfg(not(feature = "calloop"))]
impl AsRawFd for ReadPipe {
    fn as_raw_fd(&self) -> RawFd {
        self.file.as_raw_fd()
    }
}

#[cfg(feature = "calloop")]
impl IntoRawFd for ReadPipe {
    fn into_raw_fd(self) -> RawFd {
        self.file.file.into_raw_fd()
    }
}

#[cfg(not(feature = "calloop"))]
impl IntoRawFd for ReadPipe {
    fn into_raw_fd(self) -> RawFd {
        self.file.into_raw_fd()
    }
}

#[cfg(feature = "calloop")]
impl calloop::EventSource for ReadPipe {
    type Event = ();
    type Error = std::io::Error;
    type Metadata = fs::File;
    type Ret = ();

    fn process_events<F>(
        &mut self,
        readiness: calloop::Readiness,
        token: calloop::Token,
        mut callback: F,
    ) -> std::io::Result<calloop::PostAction>
    where
        F: FnMut((), &mut fs::File),
    {
        self.file.process_events(readiness, token, |_, file| {
            callback((), file);
            Ok(calloop::PostAction::Continue)
        })
    }

    fn register(
        &mut self,
        poll: &mut calloop::Poll,
        token_factory: &mut calloop::TokenFactory,
    ) -> calloop::Result<()> {
        self.file.register(poll, token_factory)
    }

    fn reregister(
        &mut self,
        poll: &mut calloop::Poll,
        token_factory: &mut calloop::TokenFactory,
    ) -> calloop::Result<()> {
        self.file.reregister(poll, token_factory)
    }

    fn unregister(&mut self, poll: &mut calloop::Poll) -> calloop::Result<()> {
        self.file.unregister(poll)
    }
}
