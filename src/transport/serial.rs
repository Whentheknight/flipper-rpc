//! Implementation for serial communication protocols

use std::time::Duration;

use crate::logging::debug;

pub mod cli;
pub mod helpers;
pub mod rpc;

/// Baud rate for the flipper
pub(crate) const FLIPPER_BAUD: u32 = 115_200;

/// Global timeout for serial operations. Kinda large as large files may take a LONG time to
/// process
pub(crate) const TIMEOUT: Duration = Duration::from_secs(10);

/// USB vendor ID Flipper Devices Inc. ships on the Flipper Zero's CDC-ACM
/// interface (an STMicroelectronics-block ID, since the Flipper uses an
/// STM32 MCU for USB).
const FLIPPER_USB_VID: u16 = 0x0483;
/// USB product ID for the Flipper Zero's CDC-ACM interface.
const FLIPPER_USB_PID: u16 = 0x5740;

/// A flipper device. Contains port and device name;
#[derive(Debug)]
pub struct FlipperDevice {
    /// Port name. /dev/ttyACMX on linux or COMX on windows.
    pub port_name: String,
    /// Device name: Flipper XXX
    pub device_name: String,
}

/// Lists all flippers connected to the current system
///
/// A port is a Flipper if its USB manufacturer string reads "Flipper
/// Devices Inc." or its vendor/product ID matches the Flipper Zero's. The
/// VID/PID check is required as a fallback: on Windows, a port using the
/// stock `usbser.sys` driver (the normal driver for this device — no
/// separate INF ships for it) reports the *driver's* registry-provided
/// manufacturer string instead of the device's own USB descriptor string,
/// so the manufacturer-string check alone never matches there. VID/PID is
/// read from the USB descriptor directly and isn't affected by this.
#[cfg_attr(feature = "tracing", tracing::instrument)]
pub fn list_flipper_ports() -> Result<Vec<FlipperDevice>, serialport::Error> {
    debug!("scanning ports");

    let ports = serialport::available_ports()?;

    let ports = ports
        .into_iter()
        .filter_map(|port| {
            debug!("{}", port.port_name);
            if let serialport::SerialPortType::UsbPort(usb_info) = port.port_type {
                let is_flipper = usb_info.manufacturer.as_deref() == Some("Flipper Devices Inc.")
                    || (usb_info.vid == FLIPPER_USB_VID && usb_info.pid == FLIPPER_USB_PID);
                if is_flipper {
                    let device_name = usb_info.product.unwrap_or_else(|| "Flipper".to_string());
                    debug!("└── is flipper");
                    return Some(FlipperDevice {
                        port_name: port.port_name,
                        device_name,
                    });
                }
            }
            debug!("└── is not flipper");
            None
        })
        .collect();

    Ok(ports)
}
