use crate::atat::{AtRequest, NoUrcParser, TokioStream};
use crate::{Error, SmResult};

use atat::asynch::SimpleClient;
use atat::{AtDigester, Config};
use futures::channel::mpsc::{self, TryRecvError};
use tokio::runtime::Runtime;
use tokio_serial::SerialPortBuilderExt;

use std::path::PathBuf;
use std::sync::mpsc::SyncSender;
use std::time::Duration;

const ENGINE_TIC_MS: Duration = Duration::from_millis(10);
const UNREACHABLE: &str = "CRITICAL : Status Sender Unexpected Bahavior!";

pub(crate) fn at_init_thread(
    tokio_runtime: Runtime,
    at_tty_path: PathBuf,
    baud_rate: u32,
    at_status_sender: SyncSender<SmResult<mpsc::UnboundedSender<super::AtRequest>>>,
) {
    let at_status_sender_clone = at_status_sender.clone();
    if let Err(e) = tokio_runtime.block_on(async move {
        let s: &str = at_tty_path
            .to_str()
            .ok_or_else(|| Error::Other("Invalid UTF-8 path".into()))?;

        let stream = tokio_serial::new(s, baud_rate)
            .open_native_async()
            .map_err(Error::TokioSerialError)?;

        let stream = TokioStream { stream };

        let digester = AtDigester::<NoUrcParser>::new();
        let mut buf = vec![0_u8; 4096];
        let mut simple_client: SimpleClient<'_, TokioStream, AtDigester<NoUrcParser>> =
            SimpleClient::new(stream, digester, &mut buf, Config::new());

        let (at_operation_sender, mut at_operation_receiver) = mpsc::unbounded();
        at_status_sender
            .send(Ok(at_operation_sender))
            .expect(UNREACHABLE);
        loop {
            match at_operation_receiver.try_recv() {
                Ok(AtRequest::Operation(operation)) => operation(&mut simple_client).await,
                Err(TryRecvError::Empty) => tokio::time::sleep(ENGINE_TIC_MS).await,
                Ok(AtRequest::ShutDown) | Err(TryRecvError::Closed) => break,
            };
        }
        Ok(())
    }) {
        at_status_sender_clone.send(Err(e)).expect(UNREACHABLE)
    }
}
