//! [TCG] (Trusted Computing Group) protocol for [TPM] (Trusted Platform
//! Module) 2.0.
//!
//! This protocol is defined in the [TCG EFI Protocol Specification _TPM
//! Family 2.0_][spec]. It is generally implemented only for TPM 2.0
//! devices, but the spec indicates it can also be used for older TPM
//! devices.
//!
//! [spec]: https://trustedcomputinggroup.org/resource/tcg-efi-protocol-specification/
//! [TCG]: https://trustedcomputinggroup.org/
//! [TPM]: https://en.wikipedia.org/wiki/Trusted_Platform_Module

use super::HashAlgorithm;
use crate::data_types::PhysicalAddress;
use crate::proto::unsafe_protocol;
use crate::{Result, Status};
use bitflags::bitflags;
use core::mem;

/// Version information.
///
/// Layout compatible with the C type `EFI_TG2_VERSION`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd)]
pub struct Version {
    /// Major version.
    pub major: u8,
    /// Minor version.
    pub minor: u8,
}

bitflags! {
    /// Event log formats supported by the firmware.
    ///
    /// Corresponds to the C typedef `EFI_TCG2_EVENT_ALGORITHM_BITMAP`.
    #[derive(Default)]
    #[repr(transparent)]
    pub struct EventLogFormat: u32 {
        /// Firmware supports the SHA-1 log format.
        const TCG_1_2 = 0x0000_0001;

        /// Firmware supports the crypto-agile log format.
        const TCG_2 = 0x0000_0002;
    }
}

/// Information about the protocol and the TPM device.
///
/// Layout compatible with the C type `EFI_TCG2_BOOT_SERVICE_CAPABILITY`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct BootServiceCapability {
    size: u8,

    /// Version of the EFI TCG2 protocol.
    pub structure_version: Version,

    /// Version of the EFI TCG2 protocol.
    pub protocol_version: Version,

    /// Bitmap of supported hash algorithms.
    pub hash_algorithm_bitmap: HashAlgorithm,

    /// Event log formats supported by the firmware.
    pub supported_event_logs: EventLogFormat,

    present_flag: u8,

    /// Maximum size (in bytes) of a command that can be sent to the TPM.
    pub max_command_size: u16,

    /// Maximum size (in bytes) of a response that can be provided by the TPM.
    pub max_response_size: u16,

    /// Manufacturer ID.
    ///
    /// See the [TCG Vendor ID registry].
    ///
    /// [TCG Vendor ID registry]: https://trustedcomputinggroup.org/resource/vendor-id-registry/
    pub manufacturer_id: u32,

    /// Maximum number of supported PCR banks (hashing algorithms).
    pub number_of_pcr_banks: u32,

    /// Bitmap of currently-active PCR banks (hashing algorithms). This
    /// is a subset of the supported algorithms in [`hash_algorithm_bitmap`].
    ///
    /// [`hash_algorithm_bitmap`]: Self::hash_algorithm_bitmap
    pub active_pcr_banks: HashAlgorithm,
}

impl Default for BootServiceCapability {
    fn default() -> Self {
        // OK to unwrap, the size is less than u8.
        let struct_size = u8::try_from(mem::size_of::<BootServiceCapability>()).unwrap();

        Self {
            size: struct_size,
            structure_version: Version::default(),
            protocol_version: Version::default(),
            hash_algorithm_bitmap: HashAlgorithm::default(),
            supported_event_logs: EventLogFormat::default(),
            present_flag: 0,
            max_command_size: 0,
            max_response_size: 0,
            manufacturer_id: 0,
            number_of_pcr_banks: 0,
            active_pcr_banks: HashAlgorithm::default(),
        }
    }
}

impl BootServiceCapability {
    /// Whether the TPM device is present.
    #[must_use]
    pub fn tpm_present(&self) -> bool {
        self.present_flag != 0
    }
}

bitflags! {
    /// Flags for the [`Tcg::hash_log_extend_event`] function.
    #[derive(Default)]
    #[repr(transparent)]
    pub struct HashLogExtendEventFlags: u64 {
        /// Extend an event but don't log it.
        const EFI_TCG2_EXTEND_ONLY = 0x0000_0000_0000_0001;

        /// Use when measuring a PE/COFF image.
        const PE_COFF_IMAGE = 0x0000_0000_0000_0010;
    }
}

/// Protocol for interacting with TPM devices.
///
/// This protocol can be used for interacting with older TPM 1.1/1.2
/// devices, but most firmware only uses it for TPM 2.0.
///
/// The corresponding C type is `EFI_TCG2_PROTOCOL`.
#[repr(C)]
#[unsafe_protocol("607f766c-7455-42be-930b-e4d76db2720f")]
pub struct Tcg {
    get_capability: unsafe extern "efiapi" fn(
        this: *mut Tcg,
        protocol_capability: *mut BootServiceCapability,
    ) -> Status,

    get_event_log: unsafe extern "efiapi" fn(
        this: *mut Tcg,
        event_log_format: EventLogFormat,
        event_log_location: *mut PhysicalAddress,
        event_log_last_entry: *mut PhysicalAddress,
        event_log_truncated: *mut u8,
    ) -> Status,

    hash_log_extend_event: unsafe extern "efiapi" fn(
        this: *mut Tcg,
        flags: HashLogExtendEventFlags,
        data_to_hash: PhysicalAddress,
        data_to_hash_len: u64,
        // Use `()` here rather than `PcrEventInputs` so that it's a
        // thin pointer.
        event: *const (),
    ) -> Status,

    submit_command: unsafe extern "efiapi" fn(
        this: *mut Tcg,
        input_parameter_block_size: u32,
        input_parameter_block: *const u8,
        output_parameter_block_size: u32,
        output_parameter_block: *mut u8,
    ) -> Status,

    get_active_pcr_banks:
        unsafe extern "efiapi" fn(this: *mut Tcg, active_pcr_banks: *mut HashAlgorithm) -> Status,

    set_active_pcr_banks:
        unsafe extern "efiapi" fn(this: *mut Tcg, active_pcr_banks: HashAlgorithm) -> Status,

    get_result_of_set_active_pcr_banks: unsafe extern "efiapi" fn(
        this: *mut Tcg,
        operation_present: *mut u32,
        response: *mut u32,
    ) -> Status,
}

impl Tcg {
    /// Get information about the protocol and TPM device.
    pub fn get_capability(&mut self) -> Result<BootServiceCapability> {
        let mut capability = BootServiceCapability::default();
        unsafe { (self.get_capability)(self, &mut capability).into_with_val(|| capability) }
    }

    /// Get a bitmap of the active PCR banks. Each bank corresponds to a hash
    /// algorithm.
    pub fn get_active_pcr_banks(&mut self) -> Result<HashAlgorithm> {
        let mut active_pcr_banks = HashAlgorithm::empty();

        let status = unsafe { (self.get_active_pcr_banks)(self, &mut active_pcr_banks) };

        status.into_with_val(|| active_pcr_banks)
    }

    /// Set the active PCR banks. Each bank corresponds to a hash
    /// algorithm. This change will not take effect until the system is
    /// rebooted twice.
    pub fn set_active_pcr_banks(&mut self, active_pcr_banks: HashAlgorithm) -> Result {
        unsafe { (self.set_active_pcr_banks)(self, active_pcr_banks) }.into()
    }

    /// Get the stored result of calling [`Tcg::set_active_pcr_banks`] in a
    /// previous boot.
    ///
    /// If there was no attempt to set the active PCR banks in a previous boot,
    /// this returns `None`. Otherwise, it returns a numeric response code:
    /// * `0x00000000`: Success
    /// * `0x00000001..=0x00000FFF`: TPM error code
    /// * `0xfffffff0`: The operation was canceled by the user or timed out
    /// * `0xfffffff1`: Firmware error
    pub fn get_result_of_set_active_pcr_banks(&mut self) -> Result<Option<u32>> {
        let mut operation_present = 0;
        let mut response = 0;

        let status = unsafe {
            (self.get_result_of_set_active_pcr_banks)(self, &mut operation_present, &mut response)
        };

        status.into_with_val(|| {
            if operation_present == 0 {
                None
            } else {
                Some(response)
            }
        })
    }
}
