/// DLNA/UPnP streaming implementation
///
/// This module provides:
/// - HTTP streaming server (pull mode) - in dlna_sink.rs
/// - UPnP device discovery via SSDP
/// - SSDP server for device announcement
/// - Device description XML generation
/// - AVTransport control (push mode)
/// - DIDL-Lite metadata generation
/// - Proper XML parsing with quick-xml
pub mod avtransport;
pub mod device_description;
pub mod device_profiles;
pub mod didl;
pub mod discovery;
pub mod ssdp_server;
pub mod xml_parser;

pub use avtransport::{AVTransport, PositionInfo, TransportInfo};
pub use device_description::{generate_device_uuid, generate_device_description};
pub use device_profiles::{DeviceProfile, DeviceQuirks, OptimalConfig};
pub use didl::{generate_didl_lite, generate_simple_didl_lite, MediaMetadata};
pub use discovery::{create_device_from_ip, discover_devices, find_device_by_name, DlnaDevice, DlnaService};
pub use ssdp_server::SsdpServer;
pub use xml_parser::parse_device_xml_proper;
