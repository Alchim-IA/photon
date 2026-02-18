use crate::scanner::*;

#[cfg(target_os = "macos")]
use objc::declare::ClassDecl;
#[cfg(target_os = "macos")]
use objc::runtime::{Class, Object, Sel, BOOL, YES};
#[cfg(target_os = "macos")]
use objc::{class, msg_send, sel, sel_impl};
#[cfg(target_os = "macos")]
use std::ffi::CStr;
#[cfg(target_os = "macos")]
use std::sync::Once;
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

#[cfg(target_os = "macos")]
fn run_loop_for(seconds: f64) {
    unsafe {
        let ns_date: *mut Object =
            msg_send![class!(NSDate), dateWithTimeIntervalSinceNow: seconds];
        let ns_runloop: *mut Object = msg_send![class!(NSRunLoop), currentRunLoop];
        let _: () = msg_send![ns_runloop, runUntilDate: ns_date];
    }
}

#[cfg(target_os = "macos")]
fn on_main_thread_sync<F, R>(f: F) -> R
where
    F: FnOnce() -> R + Send,
    R: Send,
{
    extern "C" {
        static _dispatch_main_q: std::ffi::c_void;
        fn dispatch_sync_f(
            queue: *const std::ffi::c_void,
            context: *mut std::ffi::c_void,
            work: extern "C" fn(*mut std::ffi::c_void),
        );
        fn pthread_main_np() -> i32;
    }

    if unsafe { pthread_main_np() } != 0 {
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
        let queue = &_dispatch_main_q as *const std::ffi::c_void;
        dispatch_sync_f(
            queue,
            &mut ctx as *mut Context<F, R> as *mut std::ffi::c_void,
            trampoline::<F, R>,
        );
    }

    ctx.result.unwrap()
}

// ─── ICDeviceBrowserDelegate ─────────────────────────────────────

#[cfg(target_os = "macos")]
static REGISTER_BROWSER_DELEGATE: Once = Once::new();

#[cfg(target_os = "macos")]
fn browser_delegate_class() -> &'static Class {
    REGISTER_BROWSER_DELEGATE.call_once(|| {
        let superclass = Class::get("NSObject").unwrap();
        let mut decl = ClassDecl::new("PhotonICBrowserDelegate", superclass).unwrap();

        decl.add_ivar::<*mut std::ffi::c_void>("_devices_ptr");

        unsafe {
            decl.add_method(
                sel!(deviceBrowser:didAddDevice:moreComing:),
                browser_did_add_device
                    as extern "C" fn(&mut Object, Sel, *mut Object, *mut Object, BOOL),
            );
            decl.add_method(
                sel!(deviceBrowser:didRemoveDevice:moreGoing:),
                browser_did_remove_device
                    as extern "C" fn(&mut Object, Sel, *mut Object, *mut Object, BOOL),
            );
        }

        decl.register();
    });
    Class::get("PhotonICBrowserDelegate").unwrap()
}

#[cfg(target_os = "macos")]
extern "C" fn browser_did_add_device(
    this: &mut Object,
    _sel: Sel,
    _browser: *mut Object,
    device: *mut Object,
    _more_coming: BOOL,
) {
    unsafe {
        let ptr: *mut std::ffi::c_void = *this.get_ivar("_devices_ptr");
        if !ptr.is_null() && !device.is_null() {
            let vec = &mut *(ptr as *mut Vec<*mut Object>);
            let _: () = msg_send![device, retain];
            vec.push(device);
        }
    }
}

#[cfg(target_os = "macos")]
extern "C" fn browser_did_remove_device(
    _this: &mut Object,
    _sel: Sel,
    _browser: *mut Object,
    _device: *mut Object,
    _more_going: BOOL,
) {
}

// ─── ICDeviceDelegate (for scan operations) ──────────────────────
//
// The scanner device needs a delegate that implements ICDeviceDelegate
// and ICScannerDeviceDelegate to handle session open/close and scan
// completion callbacks.

#[cfg(target_os = "macos")]
static REGISTER_DEVICE_DELEGATE: Once = Once::new();

/// State flags stored as ivars on the device delegate.
/// _session_open: bool (as u8)
/// _scan_done: bool (as u8)
/// _scan_error: *mut Object (NSError or nil)
#[cfg(target_os = "macos")]
fn device_delegate_class() -> &'static Class {
    REGISTER_DEVICE_DELEGATE.call_once(|| {
        let superclass = Class::get("NSObject").unwrap();
        let mut decl = ClassDecl::new("PhotonICDeviceDelegate", superclass).unwrap();

        decl.add_ivar::<u8>("_session_open");
        decl.add_ivar::<u8>("_scan_done");
        decl.add_ivar::<*mut Object>("_scan_error");

        unsafe {
            // ICDeviceDelegate
            decl.add_method(
                sel!(device:didOpenSessionWithError:),
                device_did_open_session
                    as extern "C" fn(&mut Object, Sel, *mut Object, *mut Object),
            );
            decl.add_method(
                sel!(device:didCloseSessionWithError:),
                device_did_close_session
                    as extern "C" fn(&mut Object, Sel, *mut Object, *mut Object),
            );
            decl.add_method(
                sel!(didRemoveDevice:),
                device_did_remove
                    as extern "C" fn(&mut Object, Sel, *mut Object),
            );
            decl.add_method(
                sel!(device:didReceiveStatusInformation:),
                device_did_receive_status
                    as extern "C" fn(&mut Object, Sel, *mut Object, *mut Object),
            );

            // ICScannerDeviceDelegate
            decl.add_method(
                sel!(scannerDevice:didScanToURL:),
                scanner_did_scan_to_url
                    as extern "C" fn(&mut Object, Sel, *mut Object, *mut Object),
            );
            decl.add_method(
                sel!(scannerDevice:didCompleteScanWithError:),
                scanner_did_complete_scan
                    as extern "C" fn(&mut Object, Sel, *mut Object, *mut Object),
            );
            decl.add_method(
                sel!(scannerDevice:didScanToBandData:),
                scanner_did_scan_to_band
                    as extern "C" fn(&mut Object, Sel, *mut Object, *mut Object),
            );
        }

        decl.register();
    });
    Class::get("PhotonICDeviceDelegate").unwrap()
}

#[cfg(target_os = "macos")]
extern "C" fn device_did_open_session(
    this: &mut Object,
    _sel: Sel,
    _device: *mut Object,
    error: *mut Object,
) {
    unsafe {
        if error.is_null() {
            this.set_ivar::<u8>("_session_open", 1);
            log::info!("[ICA] Session opened successfully");
        } else {
            let desc: *mut Object = msg_send![error, localizedDescription];
            log::error!("[ICA] Session open error: {}", nsstring_to_string(desc));
        }
    }
}

#[cfg(target_os = "macos")]
extern "C" fn device_did_close_session(
    _this: &mut Object,
    _sel: Sel,
    _device: *mut Object,
    _error: *mut Object,
) {
    log::info!("[ICA] Session closed");
}

#[cfg(target_os = "macos")]
extern "C" fn device_did_remove(
    _this: &mut Object,
    _sel: Sel,
    _device: *mut Object,
) {
}

#[cfg(target_os = "macos")]
extern "C" fn device_did_receive_status(
    _this: &mut Object,
    _sel: Sel,
    _device: *mut Object,
    _status: *mut Object,
) {
}

#[cfg(target_os = "macos")]
extern "C" fn scanner_did_scan_to_url(
    this: &mut Object,
    _sel: Sel,
    _device: *mut Object,
    url: *mut Object,
) {
    unsafe {
        let path: *mut Object = msg_send![url, path];
        log::info!("[ICA] Scanned to: {}", nsstring_to_string(path));
        this.set_ivar::<u8>("_scan_done", 1);
    }
}

#[cfg(target_os = "macos")]
extern "C" fn scanner_did_complete_scan(
    this: &mut Object,
    _sel: Sel,
    _device: *mut Object,
    error: *mut Object,
) {
    unsafe {
        this.set_ivar::<u8>("_scan_done", 1);
        if !error.is_null() {
            let desc: *mut Object = msg_send![error, localizedDescription];
            log::error!("[ICA] Scan error: {}", nsstring_to_string(desc));
            this.set_ivar::<*mut Object>("_scan_error", error);
        } else {
            log::info!("[ICA] Scan completed successfully");
        }
    }
}

#[cfg(target_os = "macos")]
extern "C" fn scanner_did_scan_to_band(
    _this: &mut Object,
    _sel: Sel,
    _device: *mut Object,
    _data: *mut Object,
) {
}

// ─── Discovery ───────────────────────────────────────────────────

#[cfg(target_os = "macos")]
fn discover_devices(timeout_secs: u64) -> Result<Vec<ScannerDevice>, ScannerError> {
    on_main_thread_sync(move || discover_devices_main_thread(timeout_secs))
}

#[cfg(target_os = "macos")]
fn discover_devices_main_thread(timeout_secs: u64) -> Result<Vec<ScannerDevice>, ScannerError> {
    let mut result_devices = Vec::new();

    unsafe {
        let browser_class = Class::get("ICDeviceBrowser")
            .ok_or_else(|| ScannerError::SystemError("ICDeviceBrowser non disponible".into()))?;

        let browser: *mut Object = msg_send![browser_class, alloc];
        let browser: *mut Object = msg_send![browser, init];
        if browser.is_null() {
            return Err(ScannerError::SystemError(
                "Impossible de créer ICDeviceBrowser".into(),
            ));
        }

        let mut devices_vec: Box<Vec<*mut Object>> = Box::new(Vec::new());
        let devices_ptr = &mut *devices_vec as *mut Vec<*mut Object> as *mut std::ffi::c_void;

        let delegate_cls = browser_delegate_class();
        let delegate: *mut Object = msg_send![delegate_cls, alloc];
        let delegate: *mut Object = msg_send![delegate, init];
        (*delegate).set_ivar("_devices_ptr", devices_ptr);

        let _: () = msg_send![browser, setDelegate: delegate];

        let mask: u64 = 0x00000002 | 0x00000100 | 0x00000200 | 0x00000400 | 0x00000800;
        let _: () = msg_send![browser, setBrowsedDeviceTypeMask: mask];
        let _: () = msg_send![browser, start];

        let deadline =
            std::time::Instant::now() + Duration::from_secs(timeout_secs.max(3).min(8));
        while std::time::Instant::now() < deadline {
            run_loop_for(0.5);
            if !devices_vec.is_empty() {
                run_loop_for(1.0);
                break;
            }
        }

        let mut seen_ids = std::collections::HashSet::new();

        for (i, &device) in devices_vec.iter().enumerate() {
            if device.is_null() {
                continue;
            }

            let name: *mut Object = msg_send![device, name];
            let name = nsstring_to_string(name);

            let uuid_sel = objc::runtime::Sel::register("UUIDString");
            let responds_uuid: BOOL = msg_send![device, respondsToSelector: uuid_sel];
            let id = if responds_uuid == YES {
                let uuid_ns: *mut Object = msg_send![device, UUIDString];
                nsstring_to_string(uuid_ns)
            } else {
                String::new()
            };

            let device_id = if id.is_empty() {
                format!("ica-{}", i)
            } else {
                id
            };

            if seen_ids.contains(&device_id) {
                let _: () = msg_send![device, release];
                continue;
            }
            seen_ids.insert(device_id.clone());

            result_devices.push(ScannerDevice {
                id: device_id,
                name,
                vendor: String::new(),
                capabilities: ScannerCapabilities::default(),
            });

            let _: () = msg_send![device, release];
        }

        (*delegate).set_ivar::<*mut std::ffi::c_void>("_devices_ptr", std::ptr::null_mut());
        let _: () = msg_send![browser, stop];
        let _: () = msg_send![delegate, release];
        let _: () = msg_send![browser, release];
        drop(devices_vec);
    }

    Ok(result_devices)
}

// ─── Scanning ────────────────────────────────────────────────────

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
        // ── Discover device ──
        let browser_class = Class::get("ICDeviceBrowser")
            .ok_or_else(|| ScannerError::SystemError("ICDeviceBrowser non disponible".into()))?;

        let browser: *mut Object = msg_send![browser_class, alloc];
        let browser: *mut Object = msg_send![browser, init];

        let mut devices_vec: Box<Vec<*mut Object>> = Box::new(Vec::new());
        let devices_ptr = &mut *devices_vec as *mut Vec<*mut Object> as *mut std::ffi::c_void;

        let browser_del_cls = browser_delegate_class();
        let browser_del: *mut Object = msg_send![browser_del_cls, alloc];
        let browser_del: *mut Object = msg_send![browser_del, init];
        (*browser_del).set_ivar("_devices_ptr", devices_ptr);
        let _: () = msg_send![browser, setDelegate: browser_del];

        let mask: u64 = 0x00000002 | 0x00000100 | 0x00000200 | 0x00000400 | 0x00000800;
        let _: () = msg_send![browser, setBrowsedDeviceTypeMask: mask];
        let _: () = msg_send![browser, start];

        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while std::time::Instant::now() < deadline {
            run_loop_for(0.5);
            if !devices_vec.is_empty() {
                run_loop_for(0.5);
                break;
            }
        }

        // Find target device
        let mut target_device: *mut Object = std::ptr::null_mut();
        for (i, &device) in devices_vec.iter().enumerate() {
            let uuid_sel = objc::runtime::Sel::register("UUIDString");
            let responds: BOOL = msg_send![device, respondsToSelector: uuid_sel];
            let uid = if responds == YES {
                let uuid_ns: *mut Object = msg_send![device, UUIDString];
                nsstring_to_string(uuid_ns)
            } else {
                String::new()
            };
            if uid == device_id || format!("ica-{}", i) == device_id {
                target_device = device;
                break;
            }
        }

        if target_device.is_null() {
            (*browser_del).set_ivar::<*mut std::ffi::c_void>("_devices_ptr", std::ptr::null_mut());
            for &d in devices_vec.iter() { let _: () = msg_send![d, release]; }
            let _: () = msg_send![browser, stop];
            let _: () = msg_send![browser_del, release];
            let _: () = msg_send![browser, release];
            drop(devices_vec);
            return Err(ScannerError::NoDeviceFound);
        }

        // ── Set device delegate for scan callbacks ──
        let dev_del_cls = device_delegate_class();
        let dev_del: *mut Object = msg_send![dev_del_cls, alloc];
        let dev_del: *mut Object = msg_send![dev_del, init];
        (*dev_del).set_ivar::<u8>("_session_open", 0);
        (*dev_del).set_ivar::<u8>("_scan_done", 0);
        (*dev_del).set_ivar::<*mut Object>("_scan_error", std::ptr::null_mut());
        let _: () = msg_send![target_device, setDelegate: dev_del];

        // ── Set downloads directory ──
        let tmp_dir = std::env::temp_dir().join("document-scanner-scans");
        std::fs::create_dir_all(&tmp_dir)
            .map_err(|e| ScannerError::SystemError(format!("Dossier temp: {}", e)))?;
        // Clean any leftover files
        if let Ok(entries) = std::fs::read_dir(&tmp_dir) {
            for entry in entries.flatten() {
                let _ = std::fs::remove_file(entry.path());
            }
        }

        let tmp_path = tmp_dir.to_string_lossy().to_string();
        let tmp_cstr = std::ffi::CString::new(tmp_path.clone()).unwrap();
        let ns_tmp: *mut Object =
            msg_send![class!(NSString), stringWithUTF8String: tmp_cstr.as_ptr()];
        let ns_url: *mut Object = msg_send![class!(NSURL), fileURLWithPath: ns_tmp];
        let _: () = msg_send![target_device, setDownloadsDirectory: ns_url];

        // ── Open session ──
        log::info!("[ICA] Opening session...");
        let _: () = msg_send![target_device, requestOpenSession];

        // Wait for session to open (delegate sets _session_open = 1)
        let session_deadline = std::time::Instant::now() + Duration::from_secs(10);
        while std::time::Instant::now() < session_deadline {
            run_loop_for(0.5);
            let open: u8 = *(*dev_del).get_ivar("_session_open");
            if open != 0 {
                break;
            }
        }

        let session_open: u8 = *(*dev_del).get_ivar("_session_open");
        if session_open == 0 {
            log::error!("[ICA] Session did not open within timeout");
            let _: () = msg_send![target_device, requestCloseSession];
            (*browser_del).set_ivar::<*mut std::ffi::c_void>("_devices_ptr", std::ptr::null_mut());
            for &d in devices_vec.iter() { let _: () = msg_send![d, release]; }
            let _: () = msg_send![browser, stop];
            let _: () = msg_send![dev_del, release];
            let _: () = msg_send![browser_del, release];
            let _: () = msg_send![browser, release];
            drop(devices_vec);
            return Err(ScannerError::SystemError(
                "Impossible d'ouvrir la session avec le scanner".into(),
            ));
        }

        // ── Configure scan parameters ──
        let fu: *mut Object = msg_send![target_device, selectedFunctionalUnit];
        if !fu.is_null() {
            let dpi = options.dpi as f64;
            let _: () = msg_send![fu, setResolution: dpi];

            let (paper_w, paper_h) = paper_dimensions(&options.paper_format);
            let width_inches = paper_w / 25.4;
            let height_inches = paper_h / 25.4;
            let scan_area: ((f64, f64), (f64, f64)) = ((0.0, 0.0), (width_inches, height_inches));
            let _: () = msg_send![fu, setScanArea: scan_area];

            let pixel_data_type: i64 = match color_mode_id(&options.color_mode) {
                1 => 0,
                2 => 1,
                4 => 2,
                _ => 0,
            };
            let _: () = msg_send![fu, setPixelDataType: pixel_data_type];

            let bit_depth: i64 = if pixel_data_type == 2 { 1 } else { 8 };
            let _: () = msg_send![fu, setBitDepth: bit_depth];

            if options.duplex {
                let duplex_type: u64 = 1;
                let _: () = msg_send![fu, setDocumentType: duplex_type];
            }

            log::info!("[ICA] Scan configured: {}dpi, area={}x{} in", dpi, width_inches, height_inches);
        } else {
            log::warn!("[ICA] No functional unit available, scanning with defaults");
        }

        // ── Start scan ──
        log::info!("[ICA] Requesting scan...");
        let _: () = msg_send![target_device, requestScan];

        // ── Wait for scan completion ──
        let scan_timeout = Duration::from_secs(120);
        let start = std::time::Instant::now();

        let result = loop {
            if start.elapsed() >= scan_timeout {
                log::error!("[ICA] Scan timed out after 120s");
                break Err(ScannerError::SystemError(
                    "Délai de numérisation dépassé".into(),
                ));
            }

            run_loop_for(1.0);

            // Check if delegate received scan completion
            let scan_done: u8 = *(*dev_del).get_ivar("_scan_done");
            let scan_error: *mut Object = *(*dev_del).get_ivar("_scan_error");

            if scan_done != 0 && !scan_error.is_null() {
                let desc: *mut Object = msg_send![scan_error, localizedDescription];
                let err_str = nsstring_to_string(desc);
                log::error!("[ICA] Scan failed: {}", err_str);
                break Err(ScannerError::SystemError(format!("Erreur scan: {}", err_str)));
            }

            // Check for output files
            if let Ok(entries) = std::fs::read_dir(&tmp_dir) {
                let files: Vec<_> = entries
                    .filter_map(|e| e.ok())
                    .filter(|e| {
                        let p = e.path();
                        p.extension().map_or(false, |ext| {
                            let ext = ext.to_string_lossy().to_lowercase();
                            ext == "tiff" || ext == "tif" || ext == "jpeg"
                                || ext == "jpg" || ext == "png" || ext == "pdf"
                        })
                    })
                    .collect();

                if let Some(entry) = files.last() {
                    let file_path = entry.path();
                    log::info!("[ICA] Found scan output: {:?}", file_path);

                    // Wait a moment for the file to finish writing
                    run_loop_for(0.5);

                    if let Ok(data) = std::fs::read(&file_path) {
                        if data.is_empty() {
                            continue; // File still being written
                        }

                        let img = ::image::load_from_memory(&data).map_err(|e| {
                            ScannerError::SystemError(format!("Décodage image: {}", e))
                        })?;

                        let width = img.width();
                        let height = img.height();

                        let mut png_bytes = Vec::new();
                        let mut cursor = std::io::Cursor::new(&mut png_bytes);
                        img.write_to(&mut cursor, ::image::ImageFormat::Png)
                            .map_err(|e| {
                                ScannerError::SystemError(format!("Encodage PNG: {}", e))
                            })?;

                        let _ = std::fs::remove_file(&file_path);

                        break Ok(ScanResult {
                            image_data: png_bytes,
                            width,
                            height,
                        });
                    }
                }
            }

            // If scan is done but no file found, report error
            if scan_done != 0 {
                log::error!("[ICA] Scan completed but no output file found");
                break Err(ScannerError::SystemError(
                    "Numérisation terminée mais aucun fichier généré".into(),
                ));
            }
        };

        // ── Cleanup ──
        let _: () = msg_send![target_device, requestCloseSession];
        run_loop_for(1.0);

        (*browser_del).set_ivar::<*mut std::ffi::c_void>("_devices_ptr", std::ptr::null_mut());
        for &d in devices_vec.iter() { let _: () = msg_send![d, release]; }
        let _: () = msg_send![browser, stop];
        let _: () = msg_send![dev_del, release];
        let _: () = msg_send![browser_del, release];
        let _: () = msg_send![browser, release];
        drop(devices_vec);

        result
    }
}

impl ScannerBackend for IcaBackend {
    fn list_devices(&self) -> Result<Vec<ScannerDevice>, ScannerError> {
        #[cfg(target_os = "macos")]
        {
            discover_devices(5)
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
