use wayland_client::{Connection, Dispatch, QueueHandle};
use wayland_protocols_plasma::blur::client::org_kde_kwin_blur_manager;

use crate::{
    error::GlobalError,
    globals::GlobalData,
    registry::{GlobalProxy, ProvidesRegistryState, RegistryHandler},
};

#[derive(Debug)]
pub struct KdeBlurManagerState {
    org_kde_kwin_blur_manager: GlobalProxy<org_kde_kwin_blur_manager::OrgKdeKwinBlurManager>,
}

impl KdeBlurManagerState {
    pub fn new() -> Self {
        Self { org_kde_kwin_blur_manager: GlobalProxy::NotReady }
    }

    pub fn org_kde_kwin_blur_manager(
        &self,
    ) -> Result<&org_kde_kwin_blur_manager::OrgKdeKwinBlurManager, GlobalError> {
        self.org_kde_kwin_blur_manager.get()
    }
}

pub trait KdeBlurManagerHandler: Sized {
    fn kde_blur_state(&mut self) -> &mut KdeBlurManagerState;
}

impl<D> RegistryHandler<D> for KdeBlurManagerState
where
    D: Dispatch<org_kde_kwin_blur_manager::OrgKdeKwinBlurManager, GlobalData>
        + KdeBlurManagerHandler
        + ProvidesRegistryState
        + 'static,
{
    fn ready(state: &mut D, _conn: &Connection, qh: &QueueHandle<D>) {
        state.kde_blur_state().org_kde_kwin_blur_manager =
            state.registry().bind_one(qh, 1..=1, GlobalData).into();
    }
}

pub trait KdeBlurManagerExt {
    fn kde_blur_state(&mut self) -> &mut KdeBlurManagerState;
}

impl<D, U> Dispatch<org_kde_kwin_blur_manager::OrgKdeKwinBlurManager, U, D> for KdeBlurManagerState
where
    D: Dispatch<org_kde_kwin_blur_manager::OrgKdeKwinBlurManager, U>
        + KdeBlurManagerHandler
        + 'static,
    U: KdeBlurManagerExt + 'static,
{
    fn event(
        _state: &mut D,
        _surface: &org_kde_kwin_blur_manager::OrgKdeKwinBlurManager,
        _event: org_kde_kwin_blur_manager::Event,
        _data: &U,
        _conn: &Connection,
        _qh: &QueueHandle<D>,
    ) {
        unreachable!();
    }
}
