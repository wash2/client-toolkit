use std::os::unix::prelude::{FromRawFd, RawFd};

use wayland_client::{
    protocol::{
        wl_data_device_manager::DndAction,
        wl_data_offer::{self, WlDataOffer},
    },
    Connection, Dispatch, QueueHandle, WEnum,
};

use super::{DataDeviceManagerState, ReadPipe};

#[derive(Debug, Default)]
pub struct DataOfferData {}

/// Handler trait for DataOffer events.
///
/// The functions defined in this trait are called as DataOffer events are received from the compositor.
pub trait DataOfferHandler: Sized {
    /// Offer mime type
    fn offer(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        offer: &WlDataOffer,
        mime_type: String,
    );

    /// Available Source Actions
    fn source_actions(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        offer: &WlDataOffer,
        actions: WEnum<DndAction>,
    );

    /// Action selected by the compositor after matching the source/destinatino side actions.
    fn actions(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        offer: &WlDataOffer,
        actions: WEnum<DndAction>,
    );
}

pub trait DataOfferDataExt: Send + Sync {
    fn data_offer_data(&self) -> &DataOfferData;
}

impl DataOfferDataExt for DataOfferData {
    fn data_offer_data(&self) -> &DataOfferData {
        &self
    }
}

impl<D> Dispatch<wl_data_offer::WlDataOffer, DataOfferData, D> for DataDeviceManagerState
where
    D: Dispatch<wl_data_offer::WlDataOffer, DataOfferData> + DataOfferHandler,
{
    fn event(
        state: &mut D,
        offer: &wl_data_offer::WlDataOffer,
        event: <wl_data_offer::WlDataOffer as wayland_client::Proxy>::Event,
        _data: &DataOfferData,
        conn: &wayland_client::Connection,
        qh: &wayland_client::QueueHandle<D>,
    ) {
        match event {
            wl_data_offer::Event::Offer { mime_type } => state.offer(conn, qh, offer, mime_type),
            wl_data_offer::Event::SourceActions { source_actions } => {
                state.source_actions(conn, qh, offer, source_actions);
            }
            wl_data_offer::Event::Action { dnd_action } => {
                state.actions(conn, qh, offer, dnd_action);
            }
            _ => unimplemented!(),
        };
    }
}

/// Request to receive the data of a given mime type
///
/// You can do this several times, as a reaction to motion of
/// the dnd cursor, or to inspect the data in order to choose your
/// response.
///
/// Note that you should *not* read the contents right away in a
/// blocking way, as you may deadlock your application doing so.
/// At least make sure you flush your events to the server before
/// doing so.
///
/// Fails if too many file descriptors were already open and a pipe
/// could not be created.
pub fn receive(offer: &WlDataOffer, mime_type: String) -> std::io::Result<ReadPipe> {
    use nix::fcntl::OFlag;
    use nix::unistd::{close, pipe2};
    // create a pipe
    let (readfd, writefd) = pipe2(OFlag::O_CLOEXEC)?;

    offer.receive(mime_type, writefd);

    if let Err(err) = close(writefd) {
        log::warn!("Failed to close write pipe: {}", err);
    }

    Ok(unsafe { FromRawFd::from_raw_fd(readfd) })
}

/// Receive data to the write end of a raw file descriptor. If you have the read end, you can read from it.
///
/// You can do this several times, as a reaction to motion of
/// the dnd cursor, or to inspect the data in order to choose your
/// response.
///
/// Note that you should *not* read the contents right away in a
/// blocking way, as you may deadlock your application doing so.
/// At least make sure you flush your events to the server before
/// doing so.
///
/// # Safety
///
/// The provided file destructor must be a valid FD for writing, and will be closed
/// once the contents are written.
pub unsafe fn receive_to_fd(offer: &WlDataOffer, mime_type: String, writefd: RawFd) {
    use nix::unistd::close;

    offer.receive(mime_type, writefd);

    if let Err(err) = close(writefd) {
        log::warn!("Failed to close write pipe: {}", err);
    }
}
