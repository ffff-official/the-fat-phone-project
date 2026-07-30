use crate::{Error, Event, SmResult, engine, messaging, network};

use libqmi_glib_rs::nas_client::NasEventReport;
use libqmi_glib_rs::nas_client::signal_info::SignalInfo;
use libqmi_glib_rs::{Device, DmsClient, NasClient, UimClient, VoiceClient, WdsClient, WmsClient};

use futures::channel::mpsc;
use futures::{FutureExt, select};
use gio_sys::g_file_new_for_path;
use glib::MainLoop;
use glib_unix::signal_future;
use gobject_sys::g_object_unref;
use nix::libc::{self, SIGINT, SIGTERM};

use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::mpsc::SyncSender;

pub(crate) fn init_thread(
    qmi_device_path: PathBuf,
    status_sender: SyncSender<SmResult<mpsc::UnboundedSender<engine::Request>>>,
    sm_event_sender: mpsc::Sender<Event>,
) -> SmResult<()> {
    let file: *mut gio_sys::GFile = unsafe {
        g_file_new_for_path(
            CString::new(qmi_device_path.as_os_str().as_bytes())
                .expect("The given Pathbuf contains interior NULL byte")
                .as_ptr(),
        )
    };
    let (engine_sender, engine_receiver) = mpsc::unbounded::<engine::Request>();

    let ctx = glib::MainContext::default(); // Glib runtime
    let ctx_clone = ctx.clone();
    let main_loop = MainLoop::new(Some(&ctx), false);
    let main_loop_clone = main_loop.clone();

    ctx.spawn_local(async move {
        if let Err(e) = async {
            let tokio_runtime = tokio::runtime::Runtime::new().map_err(Error::TokioRuntimeError)?; // Tokio runtime
            let tokio_handle = tokio_runtime.handle().clone();

            let mut device = Device::new(file).await?;
            device.open().await?;
            let mut wms_client = WmsClient::new(&device).await?;
            let wds_client = WdsClient::new(&device).await?;
            let uim_client = UimClient::new(&device).await?;
            let mut voice_client = VoiceClient::new(&device).await?;
            let dms_client = DmsClient::new(&device).await?;
            let mut nas_client = NasClient::new(&device).await?;

            let result = async {
                network::prepare_network_interface(&device).await?;
                let th_msg = messaging::start_messaging(
                    &mut wms_client,
                    tokio_handle.clone(),
                    sm_event_sender.clone(),
                )
                .await?;
                let mut sm_event_sender_clone = sm_event_sender.clone();
                voice_client
                    .set_all_call_status(Box::new(move |call_info_vec| {
                        let ret = sm_event_sender_clone.start_send(match call_info_vec {
                            Ok(call_info_vec) => Event::VoiceCallEvent(call_info_vec),
                            Err(e) => Event::InternalError(Error::Qmi(e)),
                        });
                        if let Err(e) = ret {
                            tracing::error!("set_all_call_status sender : cannot send : {:?}", e);
                        }
                    }))
                    .await?;
                let mut sm_event_sender_clone = sm_event_sender.clone();
                nas_client
                    .set_event_report(
                        &dms_client,
                        Box::new(
                            move |infos_res: Result<NasEventReport, libqmi_glib_rs::QmiError>| {
                                let ret = sm_event_sender_clone.start_send(match infos_res {
                                    Ok(infos) => Event::NasEvent(infos),
                                    Err(e) => Event::InternalError(Error::Qmi(e)),
                                });
                                if let Err(e) = ret {
                                    tracing::error!(
                                        "set_event_report sender : cannot send : {:?}",
                                        e
                                    );
                                }
                            },
                        ),
                    )
                    .await?;
                let mut sm_event_sender_clone = sm_event_sender.clone();
                nas_client
                    .set_signal_info(
                        &dms_client,
                        Box::new(
                            move |infos_res: Result<SignalInfo, libqmi_glib_rs::QmiError>| {
                                let ret = sm_event_sender_clone.start_send(match infos_res {
                                    Ok(infos) => Event::NasSignalInfo(infos),
                                    Err(e) => Event::InternalError(Error::Qmi(e)),
                                });
                                if let Err(e) = ret {
                                    tracing::error!(
                                        "set_signal_info sender : cannot send : {:?}",
                                        e
                                    );
                                }
                            },
                        ),
                    )
                    .await?;

                let wds_client_clone = wds_client.clone();
                let wms_client_clone = wms_client.clone();
                let uim_client_clone = uim_client.clone();
                let voice_client_clone = voice_client.clone();
                let dms_client_clone = dms_client.clone();
                let nas_client_clone = nas_client.clone();
                let status_sender_clone = status_sender.clone();
                let glib_control_loop = ctx_clone.spawn_local(async move {
                    drop(status_sender_clone.send(Ok(engine_sender)));
                    engine::glib_control_loop(
                        wds_client_clone,
                        wms_client_clone,
                        uim_client_clone,
                        voice_client_clone,
                        dms_client_clone,
                        nas_client_clone,
                        tokio_handle,
                        engine_receiver,
                    )
                    .await
                    .inspect_err(|_| unsafe {
                        libc::raise(SIGINT);
                    })
                });

                select! {
                    _ = signal_future(SIGINT).fuse() => {},
                    _ = signal_future(SIGTERM).fuse()  => {},
                }
                engine::GLIB_CONTROL_LOOP_TIMER_COND.with(|v| {
                    v.store(false, Relaxed);
                });
                match glib_control_loop.await {
                    Err(e) => tracing::error!("glib_control_loop has panicked : {}", e),
                    Ok(Err(e)) => tracing::error!("glib_control_loop failure : {}", e),
                    Ok(Ok(v)) => tracing::debug!("glib_control_loop has returning {:?}", v),
                }
                Ok::<_, Error>(th_msg)
            }
            .await;

            if let Err(e) = nas_client.close().await {
                tracing::error!("Cannot close NAS client properly : {}", e);
            }
            if let Err(e) = dms_client.close().await {
                tracing::error!("Cannot close DMS client properly : {}", e);
            }
            if let Err(e) = voice_client.close().await {
                tracing::error!("Cannot close VOICE client properly : {}", e);
            }
            if let Err(e) = uim_client.close().await {
                tracing::error!("Cannot close UIM client properly : {}", e);
            }
            if let Err(e) = wds_client.close().await {
                tracing::error!("Cannot close WDS client properly : {}", e);
            }
            if let Err(e) = wms_client.close().await {
                tracing::error!("Cannot close WMS client properly : {}", e);
            }
            if let Err(e) = device.close().await {
                tracing::error!("Cannot close DEVICE properly : {}", e)
            }

            match result {
                Ok(th_msg) => {
                    if let Err(e) = th_msg.join() {
                        tracing::error!("MESSAGING Thread has panicked ! {:?}", e);
                    }
                }
                Err(e) => {
                    tracing::error!("Task failed: {e}");
                    drop(status_sender.send(Err(e)));
                }
            }
            Ok::<(), Error>(())
        }
        .await
        {
            tracing::error!("GLib task failed: {e}");
            drop(status_sender.send(Err(e)));
        }
        main_loop_clone.quit();
    });
    main_loop.run();
    unsafe { g_object_unref(file as _) };
    Ok(())
}
