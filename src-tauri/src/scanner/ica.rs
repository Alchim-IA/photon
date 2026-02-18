use crate::scanner::*;

#[cfg(target_os = "macos")]
use objc::runtime::{Class, Object, BOOL, YES};
#[cfg(target_os = "macos")]
use objc::{class, msg_send, sel, sel_impl};
#[cfg(target_os = "macos")]
use std::ffi::CStr;
#[cfg(target_os = "macos")]
use std::sync::{Arc, Mutex};
#[cfg(target_os = "macos")]
use std::time::Duration;

pub struct IcaBackend {
    #[cfg(target_os = "macos")]
    _initialized: bool,
}

impl IcaBackend {
    pub fn new() -> Self {
        #[cfg(target_os = "macos")]
        {
            Self { _initialized: true }
        }
        #[cfg(not(target_os = "macos"))]
        Self {}
    }
}

// ─── macOS ImageCaptureCore implementation ────────────────────────

#[cfg(target_os = "macos")]
fn nsstring_to_string(nsstring: *mut Object) -> String {
    if nsstring.is_null() {
        return String::new();
    }
    unsafe {
        let utf8: *const i8 = msg_send![nsstring, UTF8String];
        if utf8.is_null() {
            return String::new();
        }
        CStr::from_ptr(utf8).to_string_lossy().into_owned()
    }
}

/// Run the NSRunLoop for the given duration to process async events (ICA device discovery, etc.)
#[cfg(target_os = "macos")]
fn run_loop_for(seconds: f64) {
    unsafe {
        let ns_date: *mut Object =
            msg_send![class!(NSDate), dateWithTimeIntervalSinceNow: seconds];
        let ns_runloop: *mut Object = msg_send![class!(NSRunLoop), currentRunLoop];
        let _: () = msg_send![ns_runloop, runUntilDate: ns_date];
    }
}

/// Execute a closure on the main thread synchronously using dispatch_sync.
/// ICDeviceBrowser and most Cocoa APIs require the main thread.
#[cfg(target_os = "macos")]
fn on_main_thread_sync<F, R>(f: F) -> R
where
    F: FnOnce() -> R + Send,
    R: Send,
{
    extern "C" {
        fn dispatch_get_main_queue() -> *mut Object;
        fn dispatch_sync_f(queue: *mut Object, context: *mut std::ffi::c_void, work: extern "C" fn(*mut std::ffi::c_void));
    }

    // We need to check if we're already on the main thread to avoid deadlock
    extern "C" {
        fn pthread_main_np() -> i32;
    }

    if unsafe { pthread_main_np() } != 0 {
        // Already on main thread, just call directly
        return f();
    }

    struct Context<F, R> {
        func: Option<F>,
        result: Option<R>,
    }

    extern "C" fn trampoline<F, R>(ctx: *mut std::ffi::c_void)
    where
        F: FnOnce() -> R,
    {
        unsafe {
            let ctx = &mut *(ctx as *mut Context<F, R>);
            let func = ctx.func.take().unwrap();
            ctx.result = Some(func());
        }
    }

    let mut ctx = Context {
        func: Some(f),
        result: None,
    };

    unsafe {
        let queue = dispatch_get_main_queue();
        dispatch_sync_f(
            queue,
            &mut ctx as *mut Context<F, R> as *mut std::ffi::c_void,
            trampoline::<F, R>,
        );
    }

    ctx.result.unwrap()
}

#[cfg(target_os = "macos")]
fn discover_devices(timeout_secs: u64) -> Result<Vec<ScannerDevice>, ScannerError> {
    on_main_thread_sync(move || discover_devices_main_thread(timeout_secs))
}

#[cfg(target_os = "macos")]
fn discover_devices_main_thread(timeout_secs: u64) -> Result<Vec<ScannerDevice>, ScannerError> {
    let mut result_devices = Vec::new();

    unsafe {
        // Create ICDeviceBrowser
        let browser_class = Class::get("ICDeviceBrowser")
            .ok_or_else(|| ScannerError::SystemError("ICDeviceBrowser non disponible".into()))?;

        let browser: *mut Object = msg_send![browser_class, new];
        if browser.is_null() {
            return Err(ScannerError::SystemError(
                "Impossible de créer ICDeviceBrowser".into(),
            ));
        }

        let mask: u64 = 0x00000200; // ICDeviceTypeMaskScanner
        let _: () = msg_send![browser, setBrowsedDeviceTypeMask: mask];
        let _: () = msg_send![browser, start];

        // Run the run loop to allow device discovery events to be processed.
        // Poll in shorter intervals to return as soon as devices are found.
        let deadline = std::time::Instant::now() + Duration::from_secs(timeout_secs.min(5));
        while std::time::Instant::now() < deadline {
            run_loop_for(0.5);

            let devices_array: *mut Object = msg_send![browser, devices];
            if !devices_array.is_null() {
                let count: usize = msg_send![devices_array, count];
                if count > 0 {
                    break;
                }
            }
        }

        // Get the devices array
        let devices_array: *mut Object = msg_send![browser, devices];
        if !devices_array.is_null() {
            let count: usize = msg_send![devices_array, count];

            for i in 0..count {
                let device: *mut Object = msg_send![devices_array, objectAtIndex: i];
                if device.is_null() {
                    continue;
                }

                let name_ns: *mut Object = msg_send![device, name];
                let name = nsstring_to_string(name_ns);

                let manufacturer_ns: *mut Object = msg_send![device, manufacturer];
                let vendor = nsstring_to_string(manufacturer_ns);

                let uuid_ns: *mut Object = msg_send![device, UUIDString];
                let id = nsstring_to_string(uuid_ns);

                // Check if device is a scanner
                let device_type: u64 = msg_send![device, type];
                let is_scanner = (device_type & 0x00000200) != 0;

                if !is_scanner {
                    continue;
                }

                // Get capabilities
                let mut caps = ScannerCapabilities::default();

                // Check for document feeder (ADF)
                let has_adf: BOOL = msg_send![device, documentLoaded];
                caps.supports_adf = has_adf == YES;

                // Check for duplex
                let supports_duplex: BOOL = msg_send![device, supportsDuplexScanning];
                caps.supports_duplex = supports_duplex == YES;

                // Available resolutions from the functional unit
                let fu: *mut Object = msg_send![device, selectedFunctionalUnit];
                if !fu.is_null() {
                    let supported_res: *mut Object = msg_send![fu, supportedResolutions];
                    if !supported_res.is_null() {
                        let res_count: usize = msg_send![supported_res, count];
                        let mut resolutions = Vec::new();
                        for j in 0..res_count {
                            let res_num: *mut Object =
                                msg_send![supported_res, objectAtIndex: j];
                            let res_val: u32 = msg_send![res_num, unsignedIntValue];
                            resolutions.push(res_val);
                        }
                        if !resolutions.is_empty() {
                            caps.resolutions = resolutions;
                        }
                    }

                    // Max scan area — physicalSize returns NSSize (width, height) in inches
                    let phys_size: (f64, f64) = msg_send![fu, physicalSize];
                    caps.max_width_mm = phys_size.0 * 25.4;
                    caps.max_height_mm = phys_size.1 * 25.4;
                }

                result_devices.push(ScannerDevice {
                    id: if id.is_empty() {
                        format!("ica-{}", i)
                    } else {
                        id
                    },
                    name,
                    vendor,
                    capabilities: caps,
                });
            }
        }

        let _: () = msg_send![browser, stop];
        let _: () = msg_send![browser, release];
    }

    Ok(result_devices)
}

#[cfg(target_os = "macos")]
fn perform_scan(device_id: &str, options: &ScanOptions) -> Result<ScanResult, ScannerError> {
    let device_id = device_id.to_string();
    let options = options.clone();
    on_main_thread_sync(move || perform_scan_main_thread(&device_id, &options))
}

#[cfg(target_os = "macos")]
fn perform_scan_main_thread(
    device_id: &str,
    options: &ScanOptions,
) -> Result<ScanResult, ScannerError> {
    unsafe {
        // Rediscover to get the device object
        let browser_class = Class::get("ICDeviceBrowser")
            .ok_or_else(|| ScannerError::SystemError("ICDeviceBrowser non disponible".into()))?;

        let browser: *mut Object = msg_send![browser_class, new];
        let mask: u64 = 0x00000200;
        let _: () = msg_send![browser, setBrowsedDeviceTypeMask: mask];
        let _: () = msg_send![browser, start];

        // Run the run loop for device discovery
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        while std::time::Instant::now() < deadline {
            run_loop_for(0.5);
            let devices_array: *mut Object = msg_send![browser, devices];
            if !devices_array.is_null() {
                let count: usize = msg_send![devices_array, count];
                if count > 0 {
                    break;
                }
            }
        }

        let devices_array: *mut Object = msg_send![browser, devices];
        if devices_array.is_null() {
            let _: () = msg_send![browser, stop];
            let _: () = msg_send![browser, release];
            return Err(ScannerError::NoDeviceFound);
        }

        let count: usize = msg_send![devices_array, count];
        let mut target_device: *mut Object = std::ptr::null_mut();

        for i in 0..count {
            let device: *mut Object = msg_send![devices_array, objectAtIndex: i];
            let uuid_ns: *mut Object = msg_send![device, UUIDString];
            let uid = nsstring_to_string(uuid_ns);
            if uid == device_id || format!("ica-{}", i) == device_id {
                target_device = device;
                break;
            }
        }

        if target_device.is_null() {
            let _: () = msg_send![browser, stop];
            let _: () = msg_send![browser, release];
            return Err(ScannerError::NoDeviceFound);
        }

        // Open the device
        let _: () = msg_send![target_device, requestOpenSession];
        run_loop_for(1.0);

        // Get the scanner functional unit
        let fu: *mut Object = msg_send![target_device, selectedFunctionalUnit];
        if fu.is_null() {
            let _: () = msg_send![target_device, requestCloseSession];
            let _: () = msg_send![browser, stop];
            let _: () = msg_send![browser, release];
            return Err(ScannerError::SystemError(
                "Unité fonctionnelle non disponible".into(),
            ));
        }

        // Configure scan parameters
        let dpi = options.dpi as f64;
        let _: () = msg_send![fu, setResolution: dpi];

        // Set scan area based on paper format
        let (paper_w, paper_h) = paper_dimensions(&options.paper_format);
        let width_inches = paper_w / 25.4;
        let height_inches = paper_h / 25.4;

        // scanArea is an NSRect: {origin: {x, y}, size: {width, height}}
        let scan_area: ((f64, f64), (f64, f64)) = ((0.0, 0.0), (width_inches, height_inches));
        let _: () = msg_send![fu, setScanArea: scan_area];

        // Color mode
        let pixel_data_type: i64 = match color_mode_id(&options.color_mode) {
            1 => 0, // ICScannerPixelDataTypeRGB
            2 => 1, // ICScannerPixelDataTypeGray
            4 => 2, // ICScannerPixelDataTypeBW
            _ => 0,
        };
        let _: () = msg_send![fu, setPixelDataType: pixel_data_type];

        // Set bit depth (8 for color/gray, 1 for BW)
        let bit_depth: i64 = if pixel_data_type == 2 { 1 } else { 8 };
        let _: () = msg_send![fu, setBitDepth: bit_depth];

        // Document type (ADF vs flatbed)
        if options.duplex {
            let duplex_type: u64 = 1; // ICScannerDocumentTypeDuplexBothSides
            let _: () = msg_send![fu, setDocumentType: duplex_type];
        }

        // Create a temporary directory for scan output
        let tmp_dir = std::env::temp_dir().join("document-scanner-scans");
        std::fs::create_dir_all(&tmp_dir)
            .map_err(|e| ScannerError::SystemError(format!("Dossier temp: {}", e)))?;

        // Set download directory
        let tmp_path = tmp_dir.to_string_lossy().to_string();
        let tmp_cstr = std::ffi::CString::new(tmp_path.clone()).unwrap();
        let ns_tmp: *mut Object =
            msg_send![class!(NSString), stringWithUTF8String: tmp_cstr.as_ptr()];
        let ns_url: *mut Object = msg_send![class!(NSURL), fileURLWithPath: ns_tmp];
        let _: () = msg_send![target_device, setDownloadsDirectory: ns_url];

        // Request scan
        let _: () = msg_send![target_device, requestScan];

        // Wait for scan to complete, running the run loop to process events
        let scan_timeout = Duration::from_secs(60);
        let start = std::time::Instant::now();
        let mut scan_complete = false;

        while start.elapsed() < scan_timeout {
            run_loop_for(0.5);

            // Check for new files in tmp_dir
            if let Ok(entries) = std::fs::read_dir(&tmp_dir) {
                let files: Vec<_> = entries
                    .filter_map(|e| e.ok())
                    .filter(|e| {
                        e.path().extension().map_or(false, |ext| {
                            ext == "tiff"
                                || ext == "tif"
                                || ext == "jpeg"
                                || ext == "jpg"
                                || ext == "png"
                        })
                    })
                    .collect();

                if !files.is_empty() {
                    // Read the most recent file
                    let file_path = files.last().unwrap().path();
                    if let Ok(data) = std::fs::read(&file_path) {
                        let img = ::image::load_from_memory(&data).map_err(|e| {
                            ScannerError::SystemError(format!("Décodage: {}", e))
                        })?;

                        let width = img.width();
                        let height = img.height();

                        let mut png_bytes = Vec::new();
                        let mut cursor = std::io::Cursor::new(&mut png_bytes);
                        img.write_to(&mut cursor, ::image::ImageFormat::Png)
                            .map_err(|e| {
                                ScannerError::SystemError(format!("Encodage PNG: {}", e))
                            })?;

                        // Cleanup
                        let _ = std::fs::remove_file(&file_path);
                        let _: () = msg_send![target_device, requestCloseSession];
                        let _: () = msg_send![browser, stop];
                        let _: () = msg_send![browser, release];

                        return Ok(ScanResult {
                            image_data: png_bytes,
                            width,
                            height,
                        });
                    }
                    scan_complete = true;
                    break;
                }
            }
        }

        let _: () = msg_send![target_device, requestCloseSession];
        let _: () = msg_send![browser, stop];
        let _: () = msg_send![browser, release];

        if !scan_complete {
            return Err(ScannerError::SystemError(
                "Délai de numérisation dépassé".into(),
            ));
        }

        Err(ScannerError::SystemError(
            "Échec de la lecture des données numérisées".into(),
        ))
    }
}

impl ScannerBackend for IcaBackend {
    fn list_devices(&self) -> Result<Vec<ScannerDevice>, ScannerError> {
        #[cfg(target_os = "macos")]
        {
            discover_devices(3)
        }
        #[cfg(not(target_os = "macos"))]
        {
            Err(ScannerError::SystemError(
                "ICA n'est disponible que sur macOS".to_string(),
            ))
        }
    }

    fn scan(&self, options: ScanOptions) -> Result<ScanResult, ScannerError> {
        #[cfg(target_os = "macos")]
        {
            perform_scan(&options.device_id, &options)
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = options;
            Err(ScannerError::SystemError(
                "ICA n'est disponible que sur macOS".to_string(),
            ))
        }
    }
}
