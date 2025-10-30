/// SSDP Server for announcing AAEQ as a discoverable UPnP device
///
/// This module handles:
/// - Periodic NOTIFY alive messages (every 30 minutes)
/// - Responding to M-SEARCH discovery requests
/// - Sending NOTIFY byebye on shutdown
use anyhow::Result;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

const SSDP_MULTICAST_ADDR: &str = "239.255.255.250:1900";
const SSDP_ALIVE_INTERVAL_SECS: u64 = 1800; // 30 minutes (UPnP spec: max 1800)

/// SSDP Server that announces AAEQ as a UPnP device
pub struct SsdpServer {
    device_uuid: String,
    friendly_name: String,
    port: u16,
    running: Arc<AtomicBool>,
    notify_task: Option<JoinHandle<()>>,
    search_task: Option<JoinHandle<()>>,
}

impl SsdpServer {
    /// Create a new SSDP server
    pub fn new(device_uuid: String, friendly_name: String, port: u16) -> Self {
        Self {
            device_uuid,
            friendly_name,
            port,
            running: Arc::new(AtomicBool::new(false)),
            notify_task: None,
            search_task: None,
        }
    }

    /// Start the SSDP server
    ///
    /// This will:
    /// 1. Send initial NOTIFY alive messages
    /// 2. Start a background task to send periodic alive messages
    /// 3. Start a background task to respond to M-SEARCH requests
    pub async fn start(&mut self) -> Result<()> {
        if self.running.load(Ordering::Relaxed) {
            warn!("SSDP server already running");
            return Ok(());
        }

        info!("Starting SSDP server for {} (port {})", self.friendly_name, self.port);

        self.running.store(true, Ordering::Relaxed);

        // Send initial NOTIFY alive messages
        self.send_notify_alive().await?;

        // Start periodic NOTIFY alive task
        let running = self.running.clone();
        let device_uuid = self.device_uuid.clone();
        let port = self.port;
        self.notify_task = Some(tokio::spawn(async move {
            periodic_notify_task(device_uuid, port, running).await;
        }));

        // Start M-SEARCH response task
        let running = self.running.clone();
        let device_uuid = self.device_uuid.clone();
        let port = self.port;
        self.search_task = Some(tokio::spawn(async move {
            msearch_response_task(device_uuid, port, running).await;
        }));

        info!("SSDP server started successfully");
        Ok(())
    }

    /// Stop the SSDP server and send byebye messages
    pub async fn stop(&mut self) -> Result<()> {
        if !self.running.load(Ordering::Relaxed) {
            return Ok(());
        }

        info!("Stopping SSDP server...");
        self.running.store(false, Ordering::Relaxed);

        // Send NOTIFY byebye messages
        self.send_notify_byebye().await?;

        // Cancel background tasks
        if let Some(task) = self.notify_task.take() {
            task.abort();
        }
        if let Some(task) = self.search_task.take() {
            task.abort();
        }

        info!("SSDP server stopped");
        Ok(())
    }

    /// Send NOTIFY alive messages for all device types
    async fn send_notify_alive(&self) -> Result<()> {
        let local_ip = get_local_ip()?;

        info!("Sending SSDP NOTIFY alive messages from {}", local_ip);

        let notification_types = vec![
            "upnp:rootdevice",
            &self.device_uuid,
            "urn:schemas-upnp-org:device:MediaServer:1",
            "urn:schemas-upnp-org:device:MediaRenderer:1",
            "urn:schemas-upnp-org:service:ContentDirectory:1",
            "urn:schemas-upnp-org:service:ConnectionManager:1",
            "urn:schemas-upnp-org:service:AVTransport:1",
        ];

        for nt in &notification_types {
            self.send_notify_message(nt, "ssdp:alive", local_ip)?;
        }

        debug!("Sent {} NOTIFY alive messages", notification_types.len());
        Ok(())
    }

    /// Send NOTIFY byebye messages for all device types
    async fn send_notify_byebye(&self) -> Result<()> {
        let local_ip = get_local_ip()?;

        info!("Sending SSDP NOTIFY byebye messages");

        let notification_types = vec![
            "upnp:rootdevice",
            &self.device_uuid,
            "urn:schemas-upnp-org:device:MediaServer:1",
            "urn:schemas-upnp-org:device:MediaRenderer:1",
        ];

        for nt in &notification_types {
            self.send_notify_message(nt, "ssdp:byebye", local_ip)?;
        }

        debug!("Sent {} NOTIFY byebye messages", notification_types.len());
        Ok(())
    }

    /// Send a single NOTIFY message
    fn send_notify_message(&self, nt: &str, nts: &str, local_ip: IpAddr) -> Result<()> {
        let location = format!("http://{}:{}/device.xml", local_ip, self.port);

        let message = format!(
            "NOTIFY * HTTP/1.1\r\n\
             HOST: {}\r\n\
             CACHE-CONTROL: max-age={}\r\n\
             LOCATION: {}\r\n\
             NT: {}\r\n\
             NTS: {}\r\n\
             SERVER: Linux/5.0 UPnP/1.0 AAEQ/1.0\r\n\
             USN: {}::{}\r\n\
             \r\n",
            SSDP_MULTICAST_ADDR,
            SSDP_ALIVE_INTERVAL_SECS,
            location,
            nt,
            nts,
            self.device_uuid,
            nt
        );

        send_multicast_message(&message)?;
        debug!("Sent NOTIFY {} for {}", nts, nt);
        Ok(())
    }
}

impl Drop for SsdpServer {
    fn drop(&mut self) {
        // Send byebye on drop (best effort)
        let rt = tokio::runtime::Handle::try_current();
        if let Ok(rt) = rt {
            let device_uuid = self.device_uuid.clone();
            rt.spawn(async move {
                if let Ok(_local_ip) = get_local_ip() {
                    let notification_types = vec![
                        "upnp:rootdevice",
                        &device_uuid,
                        "urn:schemas-upnp-org:device:MediaServer:1",
                    ];

                    for nt in notification_types {
                        let message = format!(
                            "NOTIFY * HTTP/1.1\r\n\
                             HOST: {}\r\n\
                             NT: {}\r\n\
                             NTS: ssdp:byebye\r\n\
                             USN: {}::{}\r\n\
                             \r\n",
                            SSDP_MULTICAST_ADDR, nt, device_uuid, nt
                        );
                        let _ = send_multicast_message(&message);
                    }
                }
            });
        }
    }
}

/// Background task to send periodic NOTIFY alive messages
async fn periodic_notify_task(device_uuid: String, port: u16, running: Arc<AtomicBool>) {
    info!("Starting periodic NOTIFY alive task");

    while running.load(Ordering::Relaxed) {
        // Wait for the interval
        tokio::time::sleep(Duration::from_secs(SSDP_ALIVE_INTERVAL_SECS)).await;

        if !running.load(Ordering::Relaxed) {
            break;
        }

        // Send NOTIFY alive messages
        if let Ok(local_ip) = get_local_ip() {
            let notification_types = vec![
                "upnp:rootdevice",
                &device_uuid,
                "urn:schemas-upnp-org:device:MediaServer:1",
                "urn:schemas-upnp-org:device:MediaRenderer:1",
            ];

            for nt in &notification_types {
                let location = format!("http://{}:{}/device.xml", local_ip, port);
                let message = format!(
                    "NOTIFY * HTTP/1.1\r\n\
                     HOST: {}\r\n\
                     CACHE-CONTROL: max-age={}\r\n\
                     LOCATION: {}\r\n\
                     NT: {}\r\n\
                     NTS: ssdp:alive\r\n\
                     SERVER: Linux/5.0 UPnP/1.0 AAEQ/1.0\r\n\
                     USN: {}::{}\r\n\
                     \r\n",
                    SSDP_MULTICAST_ADDR, SSDP_ALIVE_INTERVAL_SECS, location, nt, device_uuid, nt
                );

                if let Err(e) = send_multicast_message(&message) {
                    warn!("Failed to send periodic NOTIFY for {}: {}", nt, e);
                }
            }

            debug!("Sent periodic NOTIFY alive messages");
        }
    }

    info!("Periodic NOTIFY alive task stopped");
}

/// Background task to respond to M-SEARCH requests
async fn msearch_response_task(device_uuid: String, port: u16, running: Arc<AtomicBool>) {
    info!("Starting M-SEARCH response task");

    // Create socket for listening to M-SEARCH requests
    let socket = match create_msearch_listener() {
        Ok(s) => s,
        Err(e) => {
            error!("Failed to create M-SEARCH listener: {}", e);
            return;
        }
    };

    socket.set_nonblocking(true).ok();

    while running.load(Ordering::Relaxed) {
        let mut buf = [0u8; 2048];

        match socket.recv_from(&mut buf) {
            Ok((len, addr)) => {
                let request = String::from_utf8_lossy(&buf[..len]);

                // Check if this is an M-SEARCH request
                if request.starts_with("M-SEARCH") {
                    debug!("Received M-SEARCH from {}", addr);

                    // Extract the ST (Search Target) header
                    let st = extract_search_target(&request);

                    // Check if we should respond to this search target
                    if should_respond_to_st(&st, &device_uuid) {
                        if let Err(e) = send_msearch_response(&socket, addr, &device_uuid, &st, port)
                        {
                            warn!("Failed to send M-SEARCH response: {}", e);
                        } else {
                            debug!("Sent M-SEARCH response to {} for ST: {}", addr, st);
                        }
                    }
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // No data available, sleep briefly
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            Err(e) => {
                warn!("Error receiving M-SEARCH: {}", e);
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    }

    info!("M-SEARCH response task stopped");
}

/// Create a socket for listening to M-SEARCH requests
fn create_msearch_listener() -> Result<UdpSocket> {
    let socket = socket2::Socket::new(
        socket2::Domain::IPV4,
        socket2::Type::DGRAM,
        Some(socket2::Protocol::UDP),
    )?;

    socket.set_reuse_address(true)?;
    #[cfg(unix)]
    socket.set_reuse_port(true)?;

    // Bind to the SSDP multicast port
    let addr: SocketAddr = "0.0.0.0:1900".parse().unwrap();
    socket.bind(&addr.into())?;

    let socket: UdpSocket = socket.into();

    // Join the SSDP multicast group
    let multicast_addr = Ipv4Addr::new(239, 255, 255, 250);
    socket.join_multicast_v4(&multicast_addr, &Ipv4Addr::UNSPECIFIED)?;

    info!("M-SEARCH listener created on port 1900");
    Ok(socket)
}

/// Send an M-SEARCH response
fn send_msearch_response(
    socket: &UdpSocket,
    dest: SocketAddr,
    device_uuid: &str,
    st: &str,
    port: u16,
) -> Result<()> {
    let local_ip = get_local_ip()?;
    let location = format!("http://{}:{}/device.xml", local_ip, port);

    let usn = if st == "upnp:rootdevice" {
        format!("{}::upnp:rootdevice", device_uuid)
    } else if st.starts_with("uuid:") {
        device_uuid.to_string()
    } else {
        format!("{}::{}", device_uuid, st)
    };

    let response = format!(
        "HTTP/1.1 200 OK\r\n\
         CACHE-CONTROL: max-age={}\r\n\
         EXT:\r\n\
         LOCATION: {}\r\n\
         SERVER: Linux/5.0 UPnP/1.0 AAEQ/1.0\r\n\
         ST: {}\r\n\
         USN: {}\r\n\
         \r\n",
        SSDP_ALIVE_INTERVAL_SECS, location, st, usn
    );

    socket.send_to(response.as_bytes(), dest)?;
    Ok(())
}

/// Extract the ST (Search Target) header from an M-SEARCH request
fn extract_search_target(request: &str) -> String {
    for line in request.lines() {
        if line.to_uppercase().starts_with("ST:") {
            // Split only on the first colon to separate header from value
            return line
                .split_once(':')
                .map(|(_, value)| value.trim().to_string())
                .unwrap_or_default();
        }
    }
    String::new()
}

/// Check if we should respond to a given search target
fn should_respond_to_st(st: &str, device_uuid: &str) -> bool {
    match st {
        "ssdp:all" => true,
        "upnp:rootdevice" => true,
        st if st == device_uuid => true,
        "urn:schemas-upnp-org:device:MediaServer:1" => true,
        "urn:schemas-upnp-org:device:MediaRenderer:1" => true,
        "urn:schemas-upnp-org:service:ContentDirectory:1" => true,
        "urn:schemas-upnp-org:service:ConnectionManager:1" => true,
        "urn:schemas-upnp-org:service:AVTransport:1" => true,
        _ => false,
    }
}

/// Send a multicast message to the SSDP group
fn send_multicast_message(message: &str) -> Result<()> {
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.set_multicast_ttl_v4(2)?;

    let multicast_addr: SocketAddr = SSDP_MULTICAST_ADDR.parse()?;
    socket.send_to(message.as_bytes(), multicast_addr)?;

    Ok(())
}

/// Get the local IP address
fn get_local_ip() -> Result<IpAddr> {
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.connect("8.8.8.8:80")?;
    let addr = socket.local_addr()?;
    Ok(addr.ip())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_respond_to_st() {
        let uuid = "uuid:12345678-1234-1234-1234-123456789012";

        assert!(should_respond_to_st("ssdp:all", uuid));
        assert!(should_respond_to_st("upnp:rootdevice", uuid));
        assert!(should_respond_to_st(uuid, uuid));
        assert!(should_respond_to_st(
            "urn:schemas-upnp-org:device:MediaServer:1",
            uuid
        ));
        assert!(!should_respond_to_st("some:other:device", uuid));
    }

    #[test]
    fn test_extract_search_target() {
        let request = "M-SEARCH * HTTP/1.1\r\nHOST: 239.255.255.250:1900\r\nST: ssdp:all\r\n\r\n";
        assert_eq!(extract_search_target(request), "ssdp:all");

        let request2 =
            "M-SEARCH * HTTP/1.1\r\nHOST: 239.255.255.250:1900\r\nst: upnp:rootdevice\r\n\r\n";
        assert_eq!(extract_search_target(request2), "upnp:rootdevice");
    }
}
