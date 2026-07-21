use crate::{SmError, SmResult};

use libqmi_glib_rs::{Device, WdsClient};
use libqmi_glib_types_rs::{DeviceExpectedDataFormat, WdsConnectionStatus};

use futures::TryStreamExt;
use nix::libc::EEXIST;
use rtnetlink::packet_route::link::LinkFlags;
use rtnetlink::{LinkUnspec, new_connection};
use tokio::runtime::Handle;

use std::net::Ipv4Addr;
use std::sync::OnceLock;

pub(crate) static WWAN_INTERFACE: OnceLock<String> = OnceLock::new();

/// This function primarily configures the QMI network interface to operate in Data Mode.
pub(crate) async fn prepare_network_interface(device: &Device) -> SmResult<()> {
    tracing::debug!("prepare_network_interface :");
    let iface = device.get_wwan_iface();
    tracing::debug!("device : {}", iface);
    WWAN_INTERFACE.set(iface.clone()).expect("WTF");
    let data_format = device.get_expected_data_format()?;
    let wanted_format = DeviceExpectedDataFormat::RawIp;
    if data_format != wanted_format {
        device.set_expected_data_fornnat(wanted_format)?; // set raw ip
    }
    tracing::debug!("wwan = {} data format = {:?}", iface, wanted_format);
    Ok(())
}

/// Starts the network and assigns an IP address to the network interface.
pub(crate) async fn start_network(
    wds_client: &mut WdsClient,
    tokio_handle: Handle,
    apn_name: &str,
) -> SmResult<WdsConnectionStatus> {
    tracing::debug!("net : {:?}", wds_client.get_packet_service_status().await?);
    wds_client.start_network(apn_name).await?;
    let new_status = wds_client.get_packet_service_status().await?;
    tracing::debug!("net : {:?}", new_status);
    let ipv4_settings = wds_client.get_current_ipv4_settings().await?;
    tracing::debug!("{}", ipv4_settings);

    async move {
        let ret = tokio_handle
            .spawn(async move {
                let ipv4 = Ipv4Addr::from(ipv4_settings.ipv4.ok_or_else(|| "no ipv4".to_string())?);
                let mask = Ipv4Addr::from(ipv4_settings.mask.ok_or_else(|| "no mask".to_string())?);
                // let gw = Ipv4Addr::from(ipv4_settings.gw.ok_or_else(|| "no gateway".to_string())?);

                let (connection, handle, _) = new_connection()?;
                tokio::spawn(connection);

                let ifname = WWAN_INTERFACE.get().expect("WTF").to_owned();
                let mut links = handle.link().get().match_name(ifname).execute();
                let link = links.try_next().await?.ok_or("interface not found")?;
                let ifindex = link.header.index;

                let mut msg = LinkUnspec::new_with_index(ifindex).build();

                msg.header.flags.insert(LinkFlags::Up); // remove instead insert to DOWM the interface
                msg.header.change_mask.insert(LinkFlags::Up);

                // ip link set dev wwan0 up
                handle.link().set(msg).execute().await?;

                fn netmask_to_prefix(mask: Ipv4Addr) -> u8 {
                    u32::from(mask).count_ones() as u8
                }

                // ip addr add 10.x.x.x/yy dev wwan0
                let ret = handle
                    .address()
                    .add(ifindex, std::net::IpAddr::V4(ipv4), netmask_to_prefix(mask))
                    .execute()
                    .await;
                // Check if IP is already attribued / it is not a real error
                if let Err(rtnetlink::Error::NetlinkError(e)) = &ret
                    && e.raw_code() != -EEXIST
                {
                    ret?
                }

                // ip route add default via <gw> dev <ifname>
                // let route = RouteMessageBuilder::<Ipv4Addr>::new()
                //     .destination_prefix(Ipv4Addr::from([0, 0, 0, 0]), 1)
                //     .gateway(gw)
                //     .output_interface(ifindex)
                //     .build();
                // handle.route().add(route).execute().await?;
                Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
            })
            .await;
        ret?
    }
    .await
    .map_err(SmError::NetworkConfError)?;
    Ok(new_status)
}

/// Stops the network.
pub(crate) async fn stop_network(wds_client: &mut WdsClient) -> SmResult<WdsConnectionStatus> {
    tracing::debug!("net : {:?}", wds_client.get_packet_service_status().await?);
    wds_client.stop_network().await?;
    let new_status = wds_client.get_packet_service_status().await?;
    tracing::debug!("net : {:?}", new_status);
    Ok(new_status)
}
