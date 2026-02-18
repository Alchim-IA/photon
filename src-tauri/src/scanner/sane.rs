use crate::scanner::*;
use std::ffi::{CStr, CString};

pub struct SaneBackend {
    #[cfg(target_os = "linux")]
    initialized: bool,
}

impl SaneBackend {
    pub fn new() -> Self {
        #[cfg(target_os = "linux")]
        {
            let initialized = unsafe {
                let mut version: i32 = 0;
                let status = sane_init(&mut version, std::ptr::null());
                status == SANE_STATUS_GOOD
            };
            Self { initialized }
        }
        #[cfg(not(target_os = "linux"))]
        Self {}
    }
}

#[cfg(target_os = "linux")]
impl Drop for SaneBackend {
    fn drop(&mut self) {
        if self.initialized {
            unsafe { sane_exit(); }
        }
    }
}

// ─── SANE FFI Bindings ───────────────────────────────────────────

#[cfg(target_os = "linux")]
const SANE_STATUS_GOOD: i32 = 0;
#[cfg(target_os = "linux")]
const SANE_STATUS_EOF: i32 = 5;

#[cfg(target_os = "linux")]
const SANE_ACTION_SET_VALUE: i32 = 1;

#[cfg(target_os = "linux")]
const SANE_TYPE_INT: i32 = 1;
#[cfg(target_os = "linux")]
const SANE_TYPE_STRING: i32 = 3;

#[cfg(target_os = "linux")]
const SANE_FRAME_GRAY: i32 = 0;
#[cfg(target_os = "linux")]
const SANE_FRAME_RGB: i32 = 1;

#[cfg(target_os = "linux")]
#[repr(C)]
struct SaneDevice {
    name: *const i8,
    vendor: *const i8,
    model: *const i8,
    type_: *const i8,
}

#[cfg(target_os = "linux")]
#[repr(C)]
struct SaneOptionDescriptor {
    name: *const i8,
    title: *const i8,
    desc: *const i8,
    type_: i32,
    unit: i32,
    size: i32,
    cap: i32,
    constraint_type: i32,
    // constraint union follows but we don't need it fully
}

#[cfg(target_os = "linux")]
#[repr(C)]
struct SaneParameters {
    format: i32,
    last_frame: i32,
    bytes_per_line: i32,
    pixels_per_line: i32,
    lines: i32,
    depth: i32,
}

#[cfg(target_os = "linux")]
type SaneHandle = *mut std::ffi::c_void;

#[cfg(target_os = "linux")]
extern "C" {
    fn sane_init(version_code: *mut i32, authorize: *const std::ffi::c_void) -> i32;
    fn sane_exit();
    fn sane_get_devices(device_list: *mut *mut *const SaneDevice, local_only: i32) -> i32;
    fn sane_open(devicename: *const i8, handle: *mut SaneHandle) -> i32;
    fn sane_close(handle: SaneHandle);
    fn sane_get_option_descriptor(handle: SaneHandle, option: i32) -> *const SaneOptionDescriptor;
    fn sane_control_option(
        handle: SaneHandle,
        option: i32,
        action: i32,
        value: *mut std::ffi::c_void,
        info: *mut i32,
    ) -> i32;
    fn sane_get_parameters(handle: SaneHandle, params: *mut SaneParameters) -> i32;
    fn sane_start(handle: SaneHandle) -> i32;
    fn sane_read(handle: SaneHandle, data: *mut u8, max_length: i32, length: *mut i32) -> i32;
    fn sane_cancel(handle: SaneHandle);
}

#[cfg(target_os = "linux")]
unsafe fn cstr_to_string(ptr: *const i8) -> String {
    if ptr.is_null() {
        return String::new();
    }
    CStr::from_ptr(ptr).to_string_lossy().into_owned()
}

#[cfg(target_os = "linux")]
unsafe fn find_option_index(handle: SaneHandle, name: &str) -> Option<i32> {
    let target = CString::new(name).ok()?;
    for i in 1..100 {
        let desc = sane_get_option_descriptor(handle, i);
        if desc.is_null() {
            break;
        }
        let desc_ref = &*desc;
        if !desc_ref.name.is_null() {
            let opt_name = CStr::from_ptr(desc_ref.name);
            if opt_name == target.as_c_str() {
                return Some(i);
            }
        }
    }
    None
}

#[cfg(target_os = "linux")]
unsafe fn set_option_int(handle: SaneHandle, name: &str, value: i32) -> bool {
    if let Some(idx) = find_option_index(handle, name) {
        let mut val = value;
        let mut info: i32 = 0;
        let status = sane_control_option(
            handle,
            idx,
            SANE_ACTION_SET_VALUE,
            &mut val as *mut i32 as *mut std::ffi::c_void,
            &mut info,
        );
        return status == SANE_STATUS_GOOD;
    }
    false
}

#[cfg(target_os = "linux")]
unsafe fn set_option_string(handle: SaneHandle, name: &str, value: &str) -> bool {
    if let Some(idx) = find_option_index(handle, name) {
        let desc = sane_get_option_descriptor(handle, idx);
        if desc.is_null() {
            return false;
        }
        let desc_ref = &*desc;
        let buf_size = desc_ref.size as usize;

        let c_value = match CString::new(value) {
            Ok(v) => v,
            Err(_) => return false,
        };
        let bytes = c_value.as_bytes_with_nul();

        let mut buffer = vec![0u8; buf_size.max(bytes.len())];
        buffer[..bytes.len()].copy_from_slice(bytes);

        let mut info: i32 = 0;
        let status = sane_control_option(
            handle,
            idx,
            SANE_ACTION_SET_VALUE,
            buffer.as_mut_ptr() as *mut std::ffi::c_void,
            &mut info,
        );
        return status == SANE_STATUS_GOOD;
    }
    false
}

impl ScannerBackend for SaneBackend {
    fn list_devices(&self) -> Result<Vec<ScannerDevice>, ScannerError> {
        #[cfg(target_os = "linux")]
        {
            if !self.initialized {
                return Err(ScannerError::NoDriver);
            }

            unsafe {
                let mut device_list: *mut *const SaneDevice = std::ptr::null_mut();
                let status = sane_get_devices(&mut device_list, 1);

                if status != SANE_STATUS_GOOD {
                    return Err(ScannerError::SystemError(
                        format!("sane_get_devices a échoué avec le code {}", status),
                    ));
                }

                if device_list.is_null() {
                    return Ok(Vec::new());
                }

                let mut devices = Vec::new();
                let mut i = 0;

                loop {
                    let dev_ptr = *device_list.add(i);
                    if dev_ptr.is_null() {
                        break;
                    }
                    let dev = &*dev_ptr;

                    let name = cstr_to_string(dev.model);
                    let vendor = cstr_to_string(dev.vendor);
                    let id = cstr_to_string(dev.name);
                    let dev_type = cstr_to_string(dev.type_);

                    // Probe capabilities by opening the device
                    let mut caps = ScannerCapabilities::default();
                    let c_name = CString::new(id.as_str()).unwrap_or_default();
                    let mut handle: SaneHandle = std::ptr::null_mut();

                    if sane_open(c_name.as_ptr(), &mut handle) == SANE_STATUS_GOOD {
                        // Check for ADF source option
                        if let Some(source_idx) = find_option_index(handle, "source") {
                            let desc = sane_get_option_descriptor(handle, source_idx);
                            if !desc.is_null() {
                                // If source option exists, device likely has ADF
                                caps.supports_adf = true;
                            }
                        }

                        // Check for duplex option
                        caps.supports_duplex = find_option_index(handle, "duplex").is_some();

                        sane_close(handle);
                    }

                    devices.push(ScannerDevice {
                        id,
                        name: if name.is_empty() { format!("Scanner {}", dev_type) } else { name },
                        vendor,
                        capabilities: caps,
                    });

                    i += 1;
                }

                Ok(devices)
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            Err(ScannerError::SystemError("SANE n'est disponible que sur Linux".to_string()))
        }
    }

    fn scan(&self, options: ScanOptions) -> Result<ScanResult, ScannerError> {
        #[cfg(target_os = "linux")]
        {
            if !self.initialized {
                return Err(ScannerError::NoDriver);
            }

            unsafe {
                let c_device_id = CString::new(options.device_id.as_str())
                    .map_err(|_| ScannerError::SystemError("ID périphérique invalide".into()))?;

                let mut handle: SaneHandle = std::ptr::null_mut();
                let status = sane_open(c_device_id.as_ptr(), &mut handle);
                if status != SANE_STATUS_GOOD {
                    return Err(ScannerError::SystemError(
                        format!("Impossible d'ouvrir le scanner (code {})", status),
                    ));
                }

                // Set resolution
                set_option_int(handle, "resolution", options.dpi as i32);

                // Set color mode
                let mode_str = match color_mode_id(&options.color_mode) {
                    1 => "Color",
                    2 => "Gray",
                    4 => "Lineart",
                    _ => "Color",
                };
                set_option_string(handle, "mode", mode_str);

                // Set scan area based on paper format
                let (paper_w_mm, paper_h_mm) = paper_dimensions(&options.paper_format);
                // SANE uses mm for geometry (typically as fixed-point)
                set_option_int(handle, "br-x", (paper_w_mm * 65536.0) as i32);
                set_option_int(handle, "br-y", (paper_h_mm * 65536.0) as i32);
                set_option_int(handle, "tl-x", 0);
                set_option_int(handle, "tl-y", 0);

                // Set source for ADF/duplex
                if options.duplex {
                    set_option_string(handle, "source", "Duplex");
                }

                // Start scanning
                let status = sane_start(handle);
                if status != SANE_STATUS_GOOD {
                    sane_close(handle);
                    return Err(ScannerError::SystemError(
                        format!("Échec du démarrage de la numérisation (code {})", status),
                    ));
                }

                // Get parameters to know image dimensions
                let mut params = SaneParameters {
                    format: 0,
                    last_frame: 0,
                    bytes_per_line: 0,
                    pixels_per_line: 0,
                    lines: 0,
                    depth: 0,
                };
                let status = sane_get_parameters(handle, &mut params);
                if status != SANE_STATUS_GOOD {
                    sane_cancel(handle);
                    sane_close(handle);
                    return Err(ScannerError::SystemError(
                        format!("Impossible de lire les paramètres de scan (code {})", status),
                    ));
                }

                let width = params.pixels_per_line as u32;
                let height = if params.lines > 0 { params.lines as u32 } else { 4000 }; // -1 means unknown
                let bytes_per_line = params.bytes_per_line as usize;

                // Read scan data
                let mut raw_data: Vec<u8> = Vec::with_capacity(bytes_per_line * height as usize);
                let mut buf = vec![0u8; 65536];

                loop {
                    let mut length: i32 = 0;
                    let status = sane_read(
                        handle,
                        buf.as_mut_ptr(),
                        buf.len() as i32,
                        &mut length,
                    );

                    if length > 0 {
                        raw_data.extend_from_slice(&buf[..length as usize]);
                    }

                    if status == SANE_STATUS_EOF {
                        break;
                    }
                    if status != SANE_STATUS_GOOD {
                        sane_cancel(handle);
                        sane_close(handle);
                        return Err(ScannerError::SystemError(
                            format!("Erreur de lecture (code {})", status),
                        ));
                    }
                }

                sane_cancel(handle);
                sane_close(handle);

                // Convert raw data to image
                let actual_height = (raw_data.len() / bytes_per_line) as u32;
                let height = if params.lines > 0 { height } else { actual_height };

                let img = match params.format {
                    f if f == SANE_FRAME_RGB => {
                        ::image::RgbImage::from_raw(width, height, raw_data)
                            .map(::image::DynamicImage::ImageRgb8)
                            .ok_or_else(|| ScannerError::SystemError("Conversion image RGB échouée".into()))?
                    }
                    f if f == SANE_FRAME_GRAY => {
                        if params.depth == 1 {
                            // 1-bit BW: expand to 8-bit gray
                            let mut gray_data = Vec::with_capacity((width * height) as usize);
                            for byte in &raw_data {
                                for bit in (0..8).rev() {
                                    if gray_data.len() >= (width * height) as usize {
                                        break;
                                    }
                                    gray_data.push(if (byte >> bit) & 1 == 0 { 255 } else { 0 });
                                }
                            }
                            ::image::GrayImage::from_raw(width, height, gray_data)
                                .map(::image::DynamicImage::ImageLuma8)
                                .ok_or_else(|| ScannerError::SystemError("Conversion image N&B échouée".into()))?
                        } else {
                            ::image::GrayImage::from_raw(width, height, raw_data)
                                .map(::image::DynamicImage::ImageLuma8)
                                .ok_or_else(|| ScannerError::SystemError("Conversion image gris échouée".into()))?
                        }
                    }
                    _ => {
                        return Err(ScannerError::UnsupportedFormat(
                            format!("Format SANE {} non supporté", params.format),
                        ));
                    }
                };

                // Encode to PNG
                let mut png_bytes = Vec::new();
                let mut cursor = std::io::Cursor::new(&mut png_bytes);
                img.write_to(&mut cursor, ::image::ImageFormat::Png)
                    .map_err(|e| ScannerError::SystemError(format!("Encodage PNG: {}", e)))?;

                Ok(ScanResult {
                    image_data: png_bytes,
                    width,
                    height,
                })
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = options;
            Err(ScannerError::SystemError("SANE n'est disponible que sur Linux".to_string()))
        }
    }
}
