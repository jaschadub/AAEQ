use anyhow::Result;
use bytes::{BufMut, BytesMut};
use std::net::SocketAddr;
use tokio::net::UdpSocket;
use tracing::debug;

/// RTP packet for audio streaming
pub struct RtpStream {
    socket: UdpSocket,
    dest_addr: SocketAddr,
    sequence_number: u16,
    timestamp: u32,
    ssrc: u32,
}

impl RtpStream {
    /// Create a new RTP stream
    pub async fn new(local_port: u16, dest_addr: SocketAddr) -> Result<Self> {
        let socket = UdpSocket::bind(format!("0.0.0.0:{}", local_port)).await?;

        Ok(Self {
            socket,
            dest_addr,
            sequence_number: rand::random(),
            timestamp: rand::random(),
            ssrc: rand::random(),
        })
    }

    /// Send an RTP packet with audio payload
    pub async fn send_packet(&mut self, payload: &[u8], samples_in_payload: u32) -> Result<()> {
        let packet = self.create_rtp_packet(payload);
        self.socket.send_to(&packet, self.dest_addr).await?;

        // Update state
        self.sequence_number = self.sequence_number.wrapping_add(1);
        self.timestamp = self.timestamp.wrapping_add(samples_in_payload);

        debug!(
            "Sent RTP packet: seq={}, ts={}, size={}",
            self.sequence_number,
            self.timestamp,
            packet.len()
        );

        Ok(())
    }

    /// Create an RTP packet with the given payload
    fn create_rtp_packet(&self, payload: &[u8]) -> Vec<u8> {
        let mut packet = BytesMut::with_capacity(12 + payload.len());

        // RTP Header (12 bytes)
        // V=2, P=0, X=0, CC=0
        packet.put_u8(0x80);

        // M=0, PT=96 (dynamic payload type for ALAC)
        packet.put_u8(96);

        // Sequence number
        packet.put_u16(self.sequence_number);

        // Timestamp
        packet.put_u32(self.timestamp);

        // SSRC (synchronization source identifier)
        packet.put_u32(self.ssrc);

        // Payload
        packet.put_slice(payload);

        packet.to_vec()
    }

    /// Get current sequence number
    pub fn sequence_number(&self) -> u16 {
        self.sequence_number
    }

    /// Get current timestamp
    pub fn timestamp(&self) -> u32 {
        self.timestamp
    }

    /// Get SSRC
    pub fn ssrc(&self) -> u32 {
        self.ssrc
    }
}

/// RTP control stream (RTCP) for timing and feedback
pub struct RtcpStream {
    socket: UdpSocket,
    dest_addr: SocketAddr,
}

impl RtcpStream {
    /// Create a new RTCP stream
    pub async fn new(local_port: u16, dest_addr: SocketAddr) -> Result<Self> {
        let socket = UdpSocket::bind(format!("0.0.0.0:{}", local_port)).await?;

        Ok(Self { socket, dest_addr })
    }

    /// Send a sender report
    pub async fn send_sender_report(
        &self,
        ssrc: u32,
        ntp_timestamp: u64,
        rtp_timestamp: u32,
        packet_count: u32,
        octet_count: u32,
    ) -> Result<()> {
        let mut packet = BytesMut::with_capacity(28);

        // RTCP header
        packet.put_u8(0x80); // V=2, P=0, RC=0
        packet.put_u8(200); // PT=200 (SR)
        packet.put_u16(6); // Length in 32-bit words - 1

        // SSRC of sender
        packet.put_u32(ssrc);

        // NTP timestamp
        packet.put_u64(ntp_timestamp);

        // RTP timestamp
        packet.put_u32(rtp_timestamp);

        // Sender's packet count
        packet.put_u32(packet_count);

        // Sender's octet count
        packet.put_u32(octet_count);

        self.socket.send_to(&packet, self.dest_addr).await?;

        debug!("Sent RTCP sender report");

        Ok(())
    }
}

/// Get NTP timestamp (for RTCP)
pub fn get_ntp_timestamp() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap();

    // NTP timestamp: seconds since 1900-01-01
    // Unix timestamp: seconds since 1970-01-01
    // Difference: 70 years = 2208988800 seconds

    let ntp_seconds = duration.as_secs() + 2_208_988_800;
    let ntp_fraction = ((duration.subsec_nanos() as u64) << 32) / 1_000_000_000;

    (ntp_seconds << 32) | ntp_fraction
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rtp_stream_create() {
        let dest = "127.0.0.1:5000".parse().unwrap();
        let stream = RtpStream::new(6000, dest).await.unwrap();

        assert_eq!(stream.dest_addr, dest);
    }

    #[test]
    fn test_ntp_timestamp() {
        let ts = get_ntp_timestamp();
        assert!(ts > 0);

        // NTP timestamp should be reasonable (between 2020 and 2050)
        let ntp_seconds = ts >> 32;
        assert!(ntp_seconds > 3_786_825_600); // 2020-01-01 in NTP
        assert!(ntp_seconds < 4_733_654_400); // 2050-01-01 in NTP
    }
}
