//! # Flipper Text CLI
//!
//! A `Transport` for communicating with Flipper Zero devices over a serial port using the text-based cli.
//! Only use this for operations that cannot be done with RPC, as this is an innefecient and error-prone wrapper.
//!
//! ## Examples
//!
//! ```no_run
//! use flipper_rpc::{error::Result, transport::{Transport, serial::cli::SerialCliTransport}};
//!
//! # fn main() -> Result<()> {
//!
//! let mut cli = SerialCliTransport::new("/dev/ttyACM0".to_string())?;
//!
//! // Set the LED to green
//!
//! cli.send("led g 255".to_string())?;
//!
//! # Ok(())
//! # }
//! ```

use crate::error::Error;
use crate::transport::serial::{TIMEOUT, helpers::drain_until_str};
use crate::{error::Result, logging::debug};

use crate::logging::trace;
use serialport::SerialPort;

use crate::transport::{Transport, serial::FLIPPER_BAUD};

use super::{
    helpers::{drain_until, read_to_string_no_eof},
    rpc::SerialRpcTransport,
};

/// # Flipper Text CLI
///
/// A `Transport` for communicating with Flipper Zero devices over a serial port using the text-based cli.
///
/// ## Examples
///
/// ```no_run
/// use flipper_rpc::{transport::Transport, error::Result, transport::serial::cli::SerialCliTransport};
///
/// # fn main() -> Result<()> {
///
/// let port = "/dev/ttyACM0";
///
/// let mut cli = SerialCliTransport::new(port.to_string())?;
///
/// // Set the LED to green
///
/// cli.send("led g 255".to_string())?;
///
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct SerialCliTransport {
    port: Box<dyn SerialPort>,
}

impl SerialCliTransport {
    /// Creates a new SerialCliTransport from a port name
    ///
    /// # Errors
    ///
    /// Will error if serialport cannot connect to the port or if the flipper shell prompt does not
    /// appear
    ///
    /// The above errors occur after a 2 second timeout
    #[cfg_attr(feature = "tracing", tracing::instrument)]
    pub fn new<S: AsRef<str> + std::fmt::Debug>(port: S) -> Result<Self> {
        Self::with_timeout(port, TIMEOUT)
    }

    /// Same as `new`, but with an explicit read timeout instead of the
    /// crate's default — mirrors `SerialRpcTransport::with_timeout`, for
    /// the same reason: a caller polling for output between commands wants
    /// a much shorter timeout than the initial prompt-wait needs.
    #[cfg_attr(feature = "tracing", tracing::instrument)]
    pub fn with_timeout<S: AsRef<str> + std::fmt::Debug>(
        port: S,
        timeout: std::time::Duration,
    ) -> Result<Self> {
        let mut port = serialport::new(port.as_ref(), FLIPPER_BAUD)
            .timeout(timeout)
            .dtr_on_open(true)
            .open()?;
        // Same fix as SerialRpcTransport::with_timeout (rpc.rs) and for the
        // same reason: DTR alone reproduced the Flipper's CLI banner in
        // manual testing, but RTS is asserted too since the firmware
        // doesn't document which control line it actually checks, and
        // `dtr_on_open` has no RTS equivalent on the builder.
        port.write_request_to_send(true)?;

        debug!("Draining port until prompt");
        drain_until_str(&mut port, ">: ", timeout)?;

        Ok(Self { port })
    }

    /// Changes the port's read timeout after construction — used to drop
    /// from the handshake's generous timeout to a short poll interval once
    /// connected, same pattern as `SerialRpcTransport::set_timeout`.
    pub fn set_timeout(&mut self, timeout: std::time::Duration) -> Result<()> {
        self.port.set_timeout(timeout).map_err(Error::Serialport)
    }

    /// Sends a single ETX byte (0x03 / Ctrl+C), with no trailing `\r` —
    /// matches the firmware's own interrupt check
    /// (`cli_is_pipe_broken_or_is_etx_next_char`, confirmed in
    /// `applications/main/subghz/subghz_cli.c`, tag 1.4.3) for breaking out
    /// of a looping command like `subghz rx`.
    pub fn interrupt(&mut self) -> Result<()> {
        self.port.write_all(&[0x03])?;
        self.port.flush()?;
        Ok(())
    }

    /// Converts a SerialCliTransport into a SerialRpcTransport
    ///
    /// This function runs the start_rpc_session command, waits for the response, and returns
    /// a SerialRpcTransport made from the internal port
    ///
    /// # Errors
    ///
    /// Will error if the command could not be sent or if the command does not reply with a newline
    #[cfg_attr(feature = "tracing", tracing::instrument)]
    pub fn into_rpc(mut self) -> Result<SerialRpcTransport> {
        self.send("start_rpc_session".to_string())?;
        drain_until(&mut self.port, b'\n', TIMEOUT)?;

        SerialRpcTransport::from_port(self.port)
    }
}

impl Transport<String> for SerialCliTransport {
    type Err = Error;

    #[cfg_attr(feature = "tracing", tracing::instrument)]
    fn send(&mut self, cmd: String) -> std::result::Result<(), Self::Err> {
        trace!("running: {}", cmd);
        self.port.write_all(cmd.as_bytes())?;
        self.port.write_all(b"\r")?;
        self.port.flush()?;

        Ok(())
    }

    #[cfg_attr(feature = "tracing", tracing::instrument)]
    fn receive(&mut self) -> std::result::Result<String, Self::Err> {
        let string = read_to_string_no_eof(&mut self.port)?;

        Ok(string)
    }
}
