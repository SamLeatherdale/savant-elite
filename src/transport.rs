//! USB/HID transport: hidapi open policy, interface claim, and discovery helpers.
//!
//! Device-open policy is process-wide in hidapi. On macOS this crate must open
//! devices non-exclusively so Play-mode monitoring does not seize the pedal.

use hidapi::HidApi;
use rusb::{Device, DeviceHandle, Direction, GlobalContext, Recipient, RequestType};
use std::fmt;
use std::time::Duration;

use crate::protocol::{
    encode_program, Pedal, ProgramAction, KINESIS_VID, PROGRAMMING_PID, SAVANT_ELITE_PID,
};

/// Default USB operation timeout in milliseconds
pub const DEFAULT_USB_TIMEOUT_MS: u64 = 500;

/// Programming-mode vendor interface observed on PID 05F3:0232.
pub const PROGRAMMING_INTERFACE: u8 = 0;

pub mod usb_constants {
    /// Kept so tests can prove request-6 is not HID SET_REPORT (0x09).
    pub const HID_SET_REPORT: u8 = 0x09;
    /// Observed native request-6 OUT: Direction::Out + Vendor + Endpoint.
    pub const USB_REQUEST_TYPE_VENDOR_ENDPOINT_OUT: u8 = 0x42;
    /// Observed native request-7 IN: Direction::In + Vendor + Endpoint.
    pub const USB_REQUEST_TYPE_VENDOR_ENDPOINT_IN: u8 = 0xC2;
    pub const USB_VENDOR_REQUEST_6: u8 = 6;
    pub const USB_VENDOR_REQUEST_7: u8 = 7;
}

/// Verified Programming-mode request-6 control OUT (host-to-device, vendor, endpoint).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Request6Setup {
    pub bm_request_type: u8,
    pub b_request: u8,
    pub w_value: u16,
    pub w_index: u16,
}

/// Build the verified request-6 control OUT setup.
///
/// `wLength` is the encoder payload length and is supplied at write time.
#[must_use]
pub fn request6_setup() -> Request6Setup {
    Request6Setup {
        bm_request_type: rusb::request_type(
            Direction::Out,
            RequestType::Vendor,
            Recipient::Endpoint,
        ),
        b_request: usb_constants::USB_VENDOR_REQUEST_6,
        w_value: 0,
        w_index: 0,
    }
}

/// Encoder-validated request-6 transfer ready to preview or write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedProgram {
    pub pedal: Pedal,
    pub action: ProgramAction,
    pub setup: Request6Setup,
    pub payload: Vec<u8>,
}

/// Parse, validate, and encode one mapping. Does not open USB.
pub fn prepare_program(pedal: &str, action: &str) -> anyhow::Result<PreparedProgram> {
    let pedal = Pedal::from_string(pedal)?;
    let action = ProgramAction::from_string(action)?;
    let payload = encode_program(pedal, &action)?;
    Ok(PreparedProgram {
        pedal,
        action,
        setup: request6_setup(),
        payload,
    })
}

/// Observed native request-7 control IN (device-to-host, vendor, endpoint).
///
/// Returns raw completion bytes only. Field meanings are unknown; callers must
/// not treat this as a decoded status register.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Request7Setup {
    pub bm_request_type: u8,
    pub b_request: u8,
    pub w_value: u16,
    pub w_index: u16,
    pub length: usize,
}

/// Build the observed request-7 control IN setup.
pub fn request7_setup() -> Request7Setup {
    Request7Setup {
        bm_request_type: rusb::request_type(
            Direction::In,
            RequestType::Vendor,
            Recipient::Endpoint,
        ),
        b_request: usb_constants::USB_VENDOR_REQUEST_7,
        w_value: 0,
        w_index: 0,
        length: 7,
    }
}

/// Stage of the Programming-mode transport where a failure occurred.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgrammingStage {
    Enumerate,
    Open,
    Claim,
    Read,
    Write,
}

/// Classified failure for Programming-mode enumerate/open/claim/read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgrammingFailureClass {
    NotFound,
    Access,
    DriverBinding,
    Busy,
    KernelDriver,
    Other,
}

/// Result of querying whether a kernel driver owns an interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KernelDriverState {
    Active,
    Inactive,
    /// Platform cannot report kernel drivers (Windows / WinUSB).
    Unsupported,
}

/// Play vs Programming identity from a VID/PID scan (no open or transfer).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SavantIdentities {
    pub play: bool,
    pub programming: bool,
}

/// Read-only Programming-mode transport error with classified cause.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgrammingTransportError {
    pub stage: ProgrammingStage,
    pub class: ProgrammingFailureClass,
    pub source: String,
}

impl ProgrammingTransportError {
    pub fn not_found() -> Self {
        Self {
            stage: ProgrammingStage::Open,
            class: ProgrammingFailureClass::NotFound,
            source: String::new(),
        }
    }

    pub fn from_rusb(stage: ProgrammingStage, error: rusb::Error) -> Self {
        let class = match stage {
            ProgrammingStage::Enumerate => match error {
                rusb::Error::Access => ProgrammingFailureClass::Access,
                rusb::Error::NotSupported => ProgrammingFailureClass::DriverBinding,
                _ => ProgrammingFailureClass::Other,
            },
            ProgrammingStage::Open => classify_open_error(error),
            ProgrammingStage::Claim => classify_claim_error(error),
            ProgrammingStage::Read | ProgrammingStage::Write => ProgrammingFailureClass::Other,
        };
        Self {
            stage,
            class,
            source: error.to_string(),
        }
    }

    pub fn kernel_detach(error: rusb::Error) -> Self {
        Self {
            stage: ProgrammingStage::Claim,
            class: ProgrammingFailureClass::KernelDriver,
            source: error.to_string(),
        }
    }

    pub fn message(&self) -> String {
        match (self.stage, self.class) {
            (_, ProgrammingFailureClass::NotFound) => {
                "No Savant Elite Programming device found (VID 05F3 PID 0232)".to_string()
            }
            (ProgrammingStage::Enumerate, _) => {
                format!("Failed to enumerate USB devices: {}", self.source)
            }
            (ProgrammingStage::Open, ProgrammingFailureClass::Access) => {
                format!(
                    "Failed to open Programming device (access denied): {}",
                    self.source
                )
            }
            (ProgrammingStage::Open, ProgrammingFailureClass::DriverBinding) => {
                format!(
                    "Failed to open Programming device (driver binding): {}",
                    self.source
                )
            }
            (ProgrammingStage::Open, _) => {
                format!("Failed to open Programming device: {}", self.source)
            }
            (ProgrammingStage::Claim, ProgrammingFailureClass::Access) => {
                format!(
                    "Failed to claim Programming interface 0 (access denied): {}",
                    self.source
                )
            }
            (ProgrammingStage::Claim, ProgrammingFailureClass::Busy) => {
                format!(
                    "Failed to claim Programming interface 0 (busy): {}",
                    self.source
                )
            }
            (ProgrammingStage::Claim, ProgrammingFailureClass::DriverBinding) => {
                format!(
                    "Failed to claim Programming interface 0 (driver binding): {}",
                    self.source
                )
            }
            (ProgrammingStage::Claim, ProgrammingFailureClass::KernelDriver) => {
                format!(
                    "Failed to detach kernel driver from Programming interface 0: {}",
                    self.source
                )
            }
            (ProgrammingStage::Claim, _) => {
                format!("Failed to claim Programming interface 0: {}", self.source)
            }
            (ProgrammingStage::Read, _) => {
                format!("Request-7 control IN failed: {}", self.source)
            }
            (ProgrammingStage::Write, _) => {
                format!("Request-6 control OUT failed: {}", self.source)
            }
        }
    }

    pub fn suggestions(&self) -> Vec<String> {
        guidance_for(self.class, std::env::consts::OS)
    }
}

impl fmt::Display for ProgrammingTransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message())
    }
}

impl std::error::Error for ProgrammingTransportError {}

/// Classify an open-handle failure without talking to hardware.
pub fn classify_open_error(error: rusb::Error) -> ProgrammingFailureClass {
    match error {
        rusb::Error::NoDevice => ProgrammingFailureClass::NotFound,
        rusb::Error::Access => ProgrammingFailureClass::Access,
        rusb::Error::NotSupported | rusb::Error::NotFound => ProgrammingFailureClass::DriverBinding,
        _ => ProgrammingFailureClass::Other,
    }
}

/// Classify a claim-interface failure without talking to hardware.
pub fn classify_claim_error(error: rusb::Error) -> ProgrammingFailureClass {
    match error {
        rusb::Error::Access => ProgrammingFailureClass::Access,
        rusb::Error::Busy => ProgrammingFailureClass::Busy,
        rusb::Error::NotSupported | rusb::Error::NotFound => ProgrammingFailureClass::DriverBinding,
        rusb::Error::NoDevice => ProgrammingFailureClass::NotFound,
        _ => ProgrammingFailureClass::Other,
    }
}

/// Interpret `kernel_driver_active` so Windows `NotSupported` is not a detach error.
pub fn interpret_kernel_driver_active(result: Result<bool, rusb::Error>) -> KernelDriverState {
    match result {
        Ok(true) => KernelDriverState::Active,
        Ok(false) => KernelDriverState::Inactive,
        Err(rusb::Error::NotSupported) => KernelDriverState::Unsupported,
        Err(_) => KernelDriverState::Inactive,
    }
}

/// Actionable guidance for a classified Programming-mode failure.
pub fn guidance_for(class: ProgrammingFailureClass, os: &str) -> Vec<String> {
    match class {
        ProgrammingFailureClass::NotFound => vec![
            "Connect the device via USB".to_string(),
            "Try a different USB port".to_string(),
            "To enter Programming mode: flip the switch to Program and replug USB".to_string(),
        ],
        ProgrammingFailureClass::Access if os == "windows" => vec![
            "Close any other app that may have the Programming device open".to_string(),
            "If access still fails, bind only Programming PID 05F3:0232 to WinUSB".to_string(),
            "Never bind Play PID 05F3:030C to WinUSB; leave it on the HID driver".to_string(),
        ],
        ProgrammingFailureClass::Access => vec![
            "Try running with elevated permissions".to_string(),
            "Check that another app does not have the device open".to_string(),
        ],
        ProgrammingFailureClass::DriverBinding => vec![
            "Bind only Programming PID 05F3:0232 to WinUSB (for example with Zadig)".to_string(),
            "Never bind Play PID 05F3:030C to WinUSB; leave it on the HID driver".to_string(),
        ],
        ProgrammingFailureClass::Busy => {
            vec!["Close any other app using the Programming interface".to_string()]
        }
        ProgrammingFailureClass::KernelDriver => vec![
            "Failed to detach a kernel driver; try running with elevated permissions".to_string(),
        ],
        ProgrammingFailureClass::Other => {
            vec!["Reconnect the Programming device and retry".to_string()]
        }
    }
}

/// Initialize hidapi with the device-open mode this tool needs.
///
/// On macOS, hidapi seizes devices by default (`kIOHIDOptionsTypeSeizeDevice`).
/// Seizing a keyboard-class device requires root, so it fails with IOKit
/// `0xE00002C1` (privilege violation) even when the terminal already holds the
/// Input Monitoring grant. Every HID path in this tool only reads reports or
/// talks to the vendor interface; none of it needs to detach the pedal from
/// the system HID stack. Shared mode (`kIOHIDOptionsTypeNone`) delivers the
/// same reports and leaves the pedal's keystrokes flowing to macOS while we
/// watch them. The setting is process-wide in hidapi and applies to every
/// device opened after this call.
pub fn new_hid_api() -> hidapi::HidResult<HidApi> {
    let api = HidApi::new()?;
    #[cfg(target_os = "macos")]
    api.set_open_exclusive(false);
    Ok(api)
}

pub struct UsbInterfaceGuard<'a> {
    pub handle: &'a rusb::DeviceHandle<GlobalContext>,
    pub interface_num: u8,
    pub detached_kernel_driver: bool,
    pub claimed: bool,
}

impl<'a> UsbInterfaceGuard<'a> {
    /// Claim a Programming-mode interface with platform-aware kernel-driver handling.
    ///
    /// Windows `NotSupported` from `kernel_driver_active` is treated as "no detach
    /// needed" and is never reported as a sudo/kernel-driver failure.
    pub fn claim(
        handle: &'a DeviceHandle<GlobalContext>,
        interface_num: u8,
    ) -> Result<Self, ProgrammingTransportError> {
        let detached_kernel_driver =
            match interpret_kernel_driver_active(handle.kernel_driver_active(interface_num)) {
                KernelDriverState::Active => {
                    handle
                        .detach_kernel_driver(interface_num)
                        .map_err(ProgrammingTransportError::kernel_detach)?;
                    true
                }
                KernelDriverState::Inactive | KernelDriverState::Unsupported => false,
            };

        let mut guard = Self {
            handle,
            interface_num,
            detached_kernel_driver,
            claimed: false,
        };

        handle.claim_interface(interface_num).map_err(|error| {
            ProgrammingTransportError::from_rusb(ProgrammingStage::Claim, error)
        })?;
        guard.claimed = true;
        Ok(guard)
    }
}

impl Drop for UsbInterfaceGuard<'_> {
    fn drop(&mut self) {
        if self.claimed {
            let _ = self.handle.release_interface(self.interface_num);
        }

        if self.detached_kernel_driver {
            // Best-effort: if we detached the kernel driver, try to restore it.
            let _ = self.handle.attach_kernel_driver(self.interface_num);
        }
    }
}

/// Enumerate Kinesis Play/Programming identities. Does not open a handle.
pub fn scan_savant_identities() -> Result<SavantIdentities, ProgrammingTransportError> {
    let devices = rusb::devices().map_err(|error| {
        ProgrammingTransportError::from_rusb(ProgrammingStage::Enumerate, error)
    })?;
    let mut identities = SavantIdentities::default();
    for device in devices.iter() {
        let Ok(desc) = device.device_descriptor() else {
            continue;
        };
        if desc.vendor_id() != KINESIS_VID {
            continue;
        }
        match desc.product_id() {
            SAVANT_ELITE_PID => identities.play = true,
            PROGRAMMING_PID => identities.programming = true,
            _ => {}
        }
    }
    Ok(identities)
}

/// Abort when tests request that no USB layer be touched.
fn fail_on_usb_if_requested() {
    if std::env::var_os("SAVANT_FAIL_ON_USB").is_some() {
        panic!("USB enumeration attempted while SAVANT_FAIL_ON_USB is set");
    }
}

/// Find the Programming-mode device (VID 05F3 PID 0232). Does not open it.
pub fn find_programming_device() -> Result<Device<GlobalContext>, ProgrammingTransportError> {
    fail_on_usb_if_requested();
    let devices = rusb::devices().map_err(|error| {
        ProgrammingTransportError::from_rusb(ProgrammingStage::Enumerate, error)
    })?;
    for device in devices.iter() {
        let Ok(desc) = device.device_descriptor() else {
            continue;
        };
        if desc.vendor_id() == KINESIS_VID && desc.product_id() == PROGRAMMING_PID {
            return Ok(device);
        }
    }
    Err(ProgrammingTransportError::not_found())
}

/// Require `write_control` to send every payload byte.
pub fn require_full_write(
    written: usize,
    expected: usize,
) -> Result<usize, ProgrammingTransportError> {
    if written == expected {
        Ok(written)
    } else {
        Err(ProgrammingTransportError {
            stage: ProgrammingStage::Write,
            class: ProgrammingFailureClass::Other,
            source: format!("short write: wrote {written} bytes, expected {expected}"),
        })
    }
}

/// Issue one verified request-6 control OUT on an already-claimed handle.
///
/// Calls `write_control` exactly once. Does not issue request 2/3, SET_REPORT,
/// or a save/readback transfer.
pub fn write_request6(
    handle: &DeviceHandle<GlobalContext>,
    payload: &[u8],
    timeout_ms: u64,
) -> Result<usize, ProgrammingTransportError> {
    fail_on_usb_if_requested();
    let setup = request6_setup();
    let written = handle
        .write_control(
            setup.bm_request_type,
            setup.b_request,
            setup.w_value,
            setup.w_index,
            payload,
            Duration::from_millis(timeout_ms),
        )
        .map_err(|error| ProgrammingTransportError::from_rusb(ProgrammingStage::Write, error))?;
    require_full_write(written, payload.len())
}

/// Enumerate, open PID 0232, claim interface 0, and write one request-6 OUT.
///
/// Releases the interface on return via [`UsbInterfaceGuard`].
pub fn write_programming_request6(
    payload: &[u8],
    timeout_ms: u64,
) -> Result<usize, ProgrammingTransportError> {
    fail_on_usb_if_requested();
    let device = find_programming_device()?;
    let handle = device
        .open()
        .map_err(|error| ProgrammingTransportError::from_rusb(ProgrammingStage::Open, error))?;
    let _interface = UsbInterfaceGuard::claim(&handle, PROGRAMMING_INTERFACE)?;
    write_request6(&handle, payload, timeout_ms)
}

/// Issue the observed request-7 control IN on an already-claimed handle.
///
/// Returns raw bytes only. Does not decode fields.
pub fn read_request7(
    handle: &DeviceHandle<GlobalContext>,
    timeout_ms: u64,
) -> Result<Vec<u8>, ProgrammingTransportError> {
    let setup = request7_setup();
    let mut buf = vec![0u8; setup.length];
    let n = handle
        .read_control(
            setup.bm_request_type,
            setup.b_request,
            setup.w_value,
            setup.w_index,
            &mut buf,
            Duration::from_millis(timeout_ms),
        )
        .map_err(|error| ProgrammingTransportError::from_rusb(ProgrammingStage::Read, error))?;
    buf.truncate(n);
    Ok(buf)
}

/// Enumerate, open, claim interface 0, and perform a request-7 control IN.
///
/// Does not change the device configuration and never issues an OUT/write.
/// Absence of the Programming device is a [`ProgrammingFailureClass::NotFound`]
/// error; callers must not require this path when no device is present.
pub fn read_programming_request7(timeout_ms: u64) -> Result<Vec<u8>, ProgrammingTransportError> {
    let device = find_programming_device()?;
    let handle = device
        .open()
        .map_err(|error| ProgrammingTransportError::from_rusb(ProgrammingStage::Open, error))?;
    let _interface = UsbInterfaceGuard::claim(&handle, PROGRAMMING_INTERFACE)?;
    read_request7(&handle, timeout_ms)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request6_setup_matches_verified_write_envelope() {
        let setup = request6_setup();
        assert_eq!(setup.bm_request_type, 0x42);
        assert_eq!(
            setup.bm_request_type,
            rusb::request_type(Direction::Out, RequestType::Vendor, Recipient::Endpoint)
        );
        assert_eq!(
            setup.bm_request_type,
            usb_constants::USB_REQUEST_TYPE_VENDOR_ENDPOINT_OUT
        );
        assert_eq!(setup.b_request, 6);
        assert_eq!(setup.b_request, usb_constants::USB_VENDOR_REQUEST_6);
        assert_eq!(setup.w_value, 0);
        assert_eq!(setup.w_index, 0);
    }

    #[test]
    fn request6_setup_is_not_request7_or_set_report() {
        let setup = request6_setup();
        assert_ne!(setup.bm_request_type, 0xC2);
        assert_ne!(setup.bm_request_type, 0x21);
        assert_ne!(setup.b_request, 7);
        assert_ne!(setup.b_request, usb_constants::HID_SET_REPORT);
        assert_ne!(setup.b_request, 2);
        assert_ne!(setup.b_request, 3);
    }

    #[test]
    fn prepare_program_returns_verified_payload_without_usb() {
        let planned = prepare_program("a", "a").expect("verified mapping must encode");
        assert_eq!(planned.pedal, Pedal::A);
        assert_eq!(planned.action.to_string(), "a");
        assert_eq!(planned.setup, request6_setup());
        assert_eq!(
            planned.payload,
            [0x01, 0x00, 0x00, 0x01, 0x02, 0x04, 0xFE, 0x04]
        );
    }

    #[test]
    fn prepare_program_accepts_pedal_c_and_rejects_media_without_usb() {
        let planned = prepare_program("c", "a").expect("Pedal C→a must encode");
        assert_eq!(planned.pedal, Pedal::C);
        assert_eq!(
            planned.payload,
            [0x03, 0x00, 0x00, 0x01, 0x02, 0x04, 0xFE, 0x04]
        );

        let err = prepare_program("a", "play").unwrap_err();
        assert!(
            err.to_string().to_lowercase().contains("consumer")
                || err.to_string().to_lowercase().contains("media"),
            "consumer/media must fail before USB: {err}"
        );
    }

    #[test]
    fn require_full_write_rejects_short_count() {
        let err = require_full_write(4, 8).unwrap_err();
        assert_eq!(err.stage, ProgrammingStage::Write);
        assert!(err.message().contains("short write"));
        assert_eq!(require_full_write(8, 8).unwrap(), 8);
    }

    #[test]
    fn request7_setup_matches_observed_native_transfer() {
        let setup = request7_setup();
        assert_eq!(setup.bm_request_type, 0xC2);
        assert_eq!(
            setup.bm_request_type,
            rusb::request_type(Direction::In, RequestType::Vendor, Recipient::Endpoint)
        );
        assert_eq!(
            setup.bm_request_type,
            usb_constants::USB_REQUEST_TYPE_VENDOR_ENDPOINT_IN
        );
        assert_eq!(setup.b_request, 7);
        assert_eq!(setup.w_value, 0);
        assert_eq!(setup.w_index, 0);
        assert_eq!(setup.length, 7);
    }

    #[test]
    fn request7_setup_is_not_the_verified_write_envelope() {
        let setup = request7_setup();
        assert_ne!(setup.bm_request_type, 0x42);
        assert_ne!(setup.b_request, 6);
    }

    #[test]
    fn classify_open_error_distinguishes_access_from_driver_binding() {
        assert_eq!(
            classify_open_error(rusb::Error::Access),
            ProgrammingFailureClass::Access
        );
        assert_eq!(
            classify_open_error(rusb::Error::NotSupported),
            ProgrammingFailureClass::DriverBinding
        );
        assert_eq!(
            classify_open_error(rusb::Error::NotFound),
            ProgrammingFailureClass::DriverBinding
        );
        assert_eq!(
            classify_open_error(rusb::Error::NoDevice),
            ProgrammingFailureClass::NotFound
        );
    }

    #[test]
    fn classify_claim_error_distinguishes_busy_access_and_driver_binding() {
        assert_eq!(
            classify_claim_error(rusb::Error::Busy),
            ProgrammingFailureClass::Busy
        );
        assert_eq!(
            classify_claim_error(rusb::Error::Access),
            ProgrammingFailureClass::Access
        );
        assert_eq!(
            classify_claim_error(rusb::Error::NotSupported),
            ProgrammingFailureClass::DriverBinding
        );
    }

    #[test]
    fn windows_not_supported_kernel_query_is_not_a_detach_failure() {
        assert_eq!(
            interpret_kernel_driver_active(Err(rusb::Error::NotSupported)),
            KernelDriverState::Unsupported
        );
        assert_eq!(
            interpret_kernel_driver_active(Ok(true)),
            KernelDriverState::Active
        );
        assert_eq!(
            interpret_kernel_driver_active(Ok(false)),
            KernelDriverState::Inactive
        );
    }

    #[test]
    fn error_messages_distinguish_no_device_from_open_access_and_driver_binding() {
        let missing = ProgrammingTransportError::not_found();
        let access =
            ProgrammingTransportError::from_rusb(ProgrammingStage::Open, rusb::Error::Access);
        let driver =
            ProgrammingTransportError::from_rusb(ProgrammingStage::Open, rusb::Error::NotSupported);
        let claim =
            ProgrammingTransportError::from_rusb(ProgrammingStage::Claim, rusb::Error::Busy);

        assert!(missing
            .message()
            .contains("No Savant Elite Programming device found"));
        assert!(access.message().to_lowercase().contains("access denied"));
        assert!(driver.message().to_lowercase().contains("driver binding"));
        assert!(claim.message().to_lowercase().contains("busy"));
        assert_ne!(missing.message(), access.message());
        assert_ne!(access.message(), driver.message());
        assert_ne!(driver.message(), claim.message());
        assert!(!missing.message().to_lowercase().contains("status register"));
        assert!(!access.message().to_lowercase().contains("status register"));
    }

    #[test]
    fn driver_binding_guidance_binds_only_programming_pid() {
        let tips = guidance_for(ProgrammingFailureClass::DriverBinding, "windows");
        let joined = tips.join(" ");
        assert!(joined.contains("05F3:0232"));
        assert!(joined.contains("05F3:030C"));
        assert!(joined.to_lowercase().contains("never"));
        assert!(joined.contains("WinUSB"));
        assert!(!joined.to_lowercase().contains("sudo"));
    }

    #[test]
    fn windows_access_guidance_never_rebrinds_play_pid() {
        let tips = guidance_for(ProgrammingFailureClass::Access, "windows");
        let joined = tips.join(" ");
        assert!(joined.contains("05F3:0232"));
        assert!(joined.contains("Never bind Play PID 05F3:030C"));
        assert!(!joined.to_lowercase().contains("sudo"));
    }

    #[test]
    fn kernel_driver_guidance_is_not_used_for_windows_not_supported() {
        let unsupported = interpret_kernel_driver_active(Err(rusb::Error::NotSupported));
        assert_eq!(unsupported, KernelDriverState::Unsupported);
        let tips = guidance_for(ProgrammingFailureClass::DriverBinding, "windows");
        assert!(tips
            .iter()
            .all(|tip| !tip.to_lowercase().contains("kernel driver")));
        assert!(tips.iter().all(|tip| !tip.to_lowercase().contains("sudo")));
    }

    /// Seizing a keyboard-class device on macOS needs root and fails with
    /// IOKit 0xE00002C1 even with Input Monitoring granted, so every HID
    /// handle this tool opens must be a shared (non-exclusive) one.
    #[cfg(target_os = "macos")]
    #[test]
    fn hid_api_opens_devices_in_shared_mode() {
        let api = new_hid_api().expect("hidapi should initialize without a pedal attached");
        assert!(
            !api.get_open_exclusive(),
            "hidapi must not seize devices; monitor would fail with 0xE00002C1"
        );
    }
}
