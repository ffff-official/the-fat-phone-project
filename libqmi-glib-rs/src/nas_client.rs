use super::*;

use crate::sys::*;

use std::cell::RefCell;
use std::ptr::NonNull;
use std::rc::Rc;

/// A client for interacting with the Network Access Service (NAS) of a QMI device.
#[derive(Debug)]
pub struct NasClient {
    client: NonNull<QmiClientNas>,
    device: Device,
    data: Rc<RefCell<Option<()>>>,
}

client_drop_impl!(NasClient);

client_clone_impl!(NasClient);

impl NasClient {
    #[allow(dead_code)]
    pub(crate) fn get_inner_device(&self) -> Device {
        self.device.clone()
    }

    new_client_impl!(NasClient, QmiClientNas, libqmi_glib_types_rs::Service::Nas);

    close_client_impl!("nas");
}

allocate_client_callback!(allocate_client_cb, QmiClientNas, "allocate NAS client");

release_client_callback!(release_client_cb, "release NAS client");
