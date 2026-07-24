use super::*;

use crate::sys::*;

use std::cell::RefCell;
use std::ptr::NonNull;
use std::rc::Rc;

/// A client for interacting with the Device Management Service (DMS) of a QMI device.
#[derive(Debug)]
pub struct DmsClient {
    client: NonNull<QmiClientDms>,
    device: Device,
    data: Rc<RefCell<Option<()>>>,
}

client_drop_impl!(DmsClient);

client_clone_impl!(DmsClient);

impl DmsClient {
    #[allow(dead_code)]
    pub(crate) fn get_inner_device(&self) -> Device {
        self.device.clone()
    }

    new_client_impl!(DmsClient, QmiClientDms, libqmi_glib_types_rs::Service::Dms);

    close_client_impl!("dms");
}

allocate_client_callback!(allocate_client_cb, QmiClientDms, "allocate DMS client");

release_client_callback!(release_client_cb, "release DMS client");
