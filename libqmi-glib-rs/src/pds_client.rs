use libqmi_glib_types_rs::Service;

use super::*;

use crate::sys::*;

use std::cell::RefCell;
use std::ptr::NonNull;
use std::rc::Rc;

/// A client for interacting with the Position Determination Service (PDS) of a QMI device.
#[derive(Debug)]
pub struct PdsClient {
    client: NonNull<QmiClientPds>,
    device: Device,
    data: Rc<RefCell<Option<()>>>,
}

client_drop_impl!(PdsClient);

client_clone_impl!(PdsClient);

impl PdsClient {
    #[allow(dead_code)]
    pub(crate) fn get_inner_device(&self) -> Device {
        self.device.clone()
    }

    new_client_impl!(PdsClient, QmiClientPds, Service::Pds);

    close_client_impl!("pds");
}

allocate_client_callback!(allocate_client_cb, QmiClientPds, "allocate PDS client");

release_client_callback!(release_client_cb, "release PDS client");
