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

/// CGRect-compatible layout for ICA scan area.
#[cfg(target_os = "macos")]
#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct CGRect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

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

// ObjC pointers are passed between threads as usize (pointer-as-integer).
// This avoids Send issues with *mut Object. Cast back with `x as *mut Object`.

// ─── Short non-blocking dispatch to main queue ───────────────────

/// Dispatch a SHORT closure to the main GCD queue and wait for it.
/// The closure MUST NOT block — it should complete in microseconds.
/// Between calls, the main run loop is free for ICA's callbacks.
#[cfg(target_os = "macos")]
fn dispatch_main_short<F, R>(f: F) -> R
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    use std::sync::{Arc, Mutex};

    extern "C" {
        static _dispatch_main_q: u8;
        fn dispatch_async_f(
            queue: *const u8,
            context: *mut std::ffi::c_void,
            work: extern "C" fn(*mut std::ffi::c_void),
        );
    }

    struct Context<R> {
        f: Option<Box<dyn FnOnce() -> R + Send>>,
        result: Option<R>,
        done: bool,
    }

    let ctx = Arc::new(Mutex::new(Context {
        f: Some(Box::new(f)),
        result: None,
        done: false,
    }));

    extern "C" fn trampoline<R: Send + 'static>(raw: *mut std::ffi::c_void) {
        unsafe {
            let ctx = Arc::from_raw(raw as *const Mutex<Context<R>>);
            let func = {
                let mut guard = ctx.lock().unwrap();
                guard.f.take().unwrap()
            };
            let result = func();
            {
                let mut guard = ctx.lock().unwrap();
                guard.result = Some(result);
                guard.done = true;
            }
        }
    }

    unsafe {
        let queue = &_dispatch_main_q as *const u8;
        let ctx_ptr = Arc::into_raw(ctx.clone()) as *mut std::ffi::c_void;
        dispatch_async_f(queue, ctx_ptr, trampoline::<R>);
    }

    loop {
        {
            let guard = ctx.lock().unwrap();
            if guard.done {
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let mut guard = ctx.lock().unwrap();
    guard.result.take().unwrap()
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
            eprintln!("[ICA-DEBUG] Device added (total: {})", vec.len());
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

// ─── ICDeviceDelegate + ICScannerDeviceDelegate ──────────────────

#[cfg(target_os = "macos")]
static REGISTER_DEVICE_DELEGATE: Once = Once::new();

#[cfg(target_os = "macos")]
fn device_delegate_class() -> &'static Class {
    REGISTER_DEVICE_DELEGATE.call_once(|| {
        let superclass = Class::get("NSObject").unwrap();
        let mut decl = ClassDecl::new("PhotonICDeviceDelegate", superclass).unwrap();

        decl.add_ivar::<u8>("_session_open");
        decl.add_ivar::<u8>("_scan_done");
        decl.add_ivar::<u8>("_unit_ready");
        decl.add_ivar::<*mut Object>("_scan_error");
        decl.add_ivar::<*mut Object>("_functional_unit");

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
                device_did_remove as extern "C" fn(&mut Object, Sel, *mut Object),
            );
            decl.add_method(
                sel!(device:didReceiveStatusInformation:),
                device_did_receive_status
                    as extern "C" fn(&mut Object, Sel, *mut Object, *mut Object),
            );

            // ICScannerDeviceDelegate
            decl.add_method(
                sel!(scannerDevice:didSelectFunctionalUnit:error:),
                scanner_did_select_functional_unit
                    as extern "C" fn(&mut Object, Sel, *mut Object, *mut Object, *mut Object),
            );
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
    device: *mut Object,
    error: *mut Object,
) {
    unsafe {
        if error.is_null() {
            this.set_ivar::<u8>("_session_open", 1);
            eprintln!("[ICA-DEBUG] Session opened successfully");

            // Immediately request flatbed FU in the same run loop turn
            // This is critical for network scanners
            let _: () = msg_send![device, requestSelectFunctionalUnit: 0u64];
            eprintln!("[ICA-DEBUG] Requested flatbed FU from didOpenSession callback");
        } else {
            this.set_ivar::<u8>("_session_open", 2);
            let desc: *mut Object = msg_send![error, localizedDescription];
            let err_str = nsstring_to_string(desc);
            eprintln!("[ICA-DEBUG] Session open error: {}", err_str);
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
    eprintln!("[ICA-DEBUG] Session closed");
}

#[cfg(target_os = "macos")]
extern "C" fn device_did_remove(_this: &mut Object, _sel: Sel, _device: *mut Object) {}

#[cfg(target_os = "macos")]
extern "C" fn device_did_receive_status(
    _this: &mut Object,
    _sel: Sel,
    _device: *mut Object,
    _status: *mut Object,
) {
}

#[cfg(target_os = "macos")]
extern "C" fn scanner_did_select_functional_unit(
    this: &mut Object,
    _sel: Sel,
    device: *mut Object,
    fu: *mut Object,
    error: *mut Object,
) {
    unsafe {
        if error.is_null() {
            this.set_ivar::<u8>("_unit_ready", 1);
            // Store the functional unit so we can use it later
            if !fu.is_null() {
                let _: () = msg_send![fu, retain];
                this.set_ivar::<*mut Object>("_functional_unit", fu);
                eprintln!("[ICA-DEBUG] Functional unit selected OK (fu={:p})", fu);
            } else {
                // Try to get it from the device
                let sel_fu: *mut Object = msg_send![device, selectedFunctionalUnit];
                if !sel_fu.is_null() {
                    let _: () = msg_send![sel_fu, retain];
                    this.set_ivar::<*mut Object>("_functional_unit", sel_fu);
                    eprintln!("[ICA-DEBUG] Got FU from device in callback (fu={:p})", sel_fu);
                } else {
                    eprintln!("[ICA-DEBUG] Functional unit callback: fu=null, device.selectedFU=null");
                }
            }
        } else {
            let desc: *mut Object = msg_send![error, localizedDescription];
            let code: i64 = msg_send![error, code];
            eprintln!(
                "[ICA-DEBUG] Functional unit selection error (code={}): {}",
                code, nsstring_to_string(desc)
            );
            // If flatbed failed, try document feeder
            if code == -9922 {
                eprintln!("[ICA-DEBUG] Flatbed failed, trying document feeder (type=3)");
                let _: () = msg_send![device, requestSelectFunctionalUnit: 3u64];
            }
        }
    }
}

#[cfg(target_os = "macos")]
extern "C" fn scanner_did_scan_to_url(
    _this: &mut Object,
    _sel: Sel,
    _device: *mut Object,
    url: *mut Object,
) {
    unsafe {
        let path: *mut Object = msg_send![url, path];
        eprintln!("[ICA-DEBUG] Scanned to: {}", nsstring_to_string(path));
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
            eprintln!("[ICA-DEBUG] Scan error: {}", nsstring_to_string(desc));
            this.set_ivar::<*mut Object>("_scan_error", error);
        } else {
            eprintln!("[ICA-DEBUG] Scan completed successfully");
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

/// Discover ICA scanner devices using short dispatch blocks.
/// Each ObjC API call dispatches to main, returns quickly.
/// ICA callbacks run on the main run loop between our blocks.
#[cfg(target_os = "macos")]
fn discover_devices(timeout_secs: u64) -> Result<Vec<ScannerDevice>, ScannerError> {
    use std::sync::{Arc, Mutex};
    use std::sync::atomic::{AtomicUsize, Ordering};

    eprintln!("[ICA-DEBUG] discover_devices start, timeout={}s", timeout_secs);

    // Shared device count — updated by callback on main thread, read by this thread
    let device_count = Arc::new(AtomicUsize::new(0));

    // Phase 1: Create browser, delegate, start browsing (on main thread)
    let (browser, delegate, devices_storage) = dispatch_main_short(move || unsafe {
        let browser_class = Class::get("ICDeviceBrowser").unwrap();
        let browser: *mut Object = msg_send![browser_class, alloc];
        let browser: *mut Object = msg_send![browser, init];

        // Allocate device storage on the heap — persists between dispatch blocks
        let devices_storage = Box::into_raw(Box::new(Vec::<*mut Object>::new()));
        let devices_ptr = devices_storage as *mut std::ffi::c_void;

        let delegate_cls = browser_delegate_class();
        let delegate: *mut Object = msg_send![delegate_cls, alloc];
        let delegate: *mut Object = msg_send![delegate, init];
        (*delegate).set_ivar("_devices_ptr", devices_ptr);

        let _: () = msg_send![browser, setDelegate: delegate];

        let mask: u64 = 0x00000002 | 0x00000100 | 0x00000200 | 0x00000400 | 0x00000800;
        let _: () = msg_send![browser, setBrowsedDeviceTypeMask: mask];
        let _: () = msg_send![browser, start];

        eprintln!("[ICA-DEBUG] Browser started");
        (browser as usize, delegate as usize, devices_storage as usize)
    });

    // Phase 2: Wait for devices — poll from this thread, callbacks run on main
    let deadline = std::time::Instant::now() + Duration::from_secs(timeout_secs.max(3).min(8));
    let mut found_devices = false;
    while std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(500));

        // Short dispatch to read device count
        let storage = devices_storage;
        let count = dispatch_main_short(move || unsafe {
            let vec = &*(storage as *const Vec<*mut Object>);
            vec.len()
        });

        if count > 0 {
            eprintln!("[ICA-DEBUG] Found {} devices, waiting 1s more", count);
            std::thread::sleep(Duration::from_secs(1));
            found_devices = true;
            break;
        }
    }

    // Phase 3: Read device info and cleanup (on main thread)
    let storage = devices_storage;
    let b = browser;
    let d = delegate;
    let result_devices = dispatch_main_short(move || unsafe {
        let devices_vec = &*(storage as *const Vec<*mut Object>);
        let mut result = Vec::new();
        let mut seen_ids = std::collections::HashSet::new();

        eprintln!("[ICA-DEBUG] Reading {} devices", devices_vec.len());

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
                continue;
            }
            seen_ids.insert(device_id.clone());

            eprintln!("[ICA-DEBUG] Device: {} ({})", name, device_id);
            result.push(ScannerDevice {
                id: device_id,
                name,
                vendor: String::new(),
                capabilities: ScannerCapabilities::default(),
            });
        }

        // Cleanup: stop browser, release devices, free storage
        let del_obj = d as *mut Object;
        let browser_obj = b as *mut Object;
        (*del_obj).set_ivar::<*mut std::ffi::c_void>("_devices_ptr", std::ptr::null_mut());
        let _: () = msg_send![browser_obj, stop];

        for &device in devices_vec.iter() {
            let _: () = msg_send![device, release];
        }

        // Free the storage
        let _ = Box::from_raw(storage as *mut Vec<*mut Object>);

        let _: () = msg_send![del_obj, release];
        let _: () = msg_send![browser_obj, release];

        result
    });

    Ok(result_devices)
}

// ─── Scanning ────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
fn perform_scan(device_id: &str, options: &ScanOptions) -> Result<ScanResult, ScannerError> {
    let device_id = device_id.to_string();
    let options = options.clone();

    eprintln!("[ICA-DEBUG] perform_scan start, device_id={}", device_id);

    // Phase 1: Create browser, start discovery (short block)
    let (browser, browser_del, devices_storage) = dispatch_main_short(move || unsafe {
        let browser_class = Class::get("ICDeviceBrowser").unwrap();
        let browser: *mut Object = msg_send![browser_class, alloc];
        let browser: *mut Object = msg_send![browser, init];

        let devices_storage = Box::into_raw(Box::new(Vec::<*mut Object>::new()));
        let devices_ptr = devices_storage as *mut std::ffi::c_void;

        let del_cls = browser_delegate_class();
        let del: *mut Object = msg_send![del_cls, alloc];
        let del: *mut Object = msg_send![del, init];
        (*del).set_ivar("_devices_ptr", devices_ptr);
        let _: () = msg_send![browser, setDelegate: del];

        let mask: u64 = 0x00000002 | 0x00000100 | 0x00000200 | 0x00000400 | 0x00000800;
        let _: () = msg_send![browser, setBrowsedDeviceTypeMask: mask];
        let _: () = msg_send![browser, start];

        (browser as usize, del as usize, devices_storage as usize)
    });

    // Phase 2: Wait for devices
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(500));
        let storage = devices_storage;
        let count = dispatch_main_short(move || unsafe {
            let vec = &*(storage as *const Vec<*mut Object>);
            vec.len()
        });
        if count > 0 {
            std::thread::sleep(Duration::from_millis(500));
            break;
        }
    }

    // Phase 3: Find target device, set up device delegate, request open session (short block)
    let dev_id = device_id.clone();
    let storage = devices_storage;
    let b = browser;
    let bd = browser_del;

    let setup_result: Result<(usize, usize, usize, usize, usize), ScannerError> = dispatch_main_short(move || unsafe {
        let devices_vec = &*(storage as *const Vec<*mut Object>);
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
            if uid == dev_id || format!("ica-{}", i) == dev_id {
                target_device = device;
                break;
            }
        }

        if target_device.is_null() {
            // Cleanup
            let bd_obj = bd as *mut Object;
            let b_obj = b as *mut Object;
            (*bd_obj).set_ivar::<*mut std::ffi::c_void>("_devices_ptr", std::ptr::null_mut());
            let _: () = msg_send![b_obj, stop];
            for &d in devices_vec.iter() { let _: () = msg_send![d, release]; }
            let _ = Box::from_raw(storage as *mut Vec<*mut Object>);
            let _: () = msg_send![bd_obj, release];
            let _: () = msg_send![b_obj, release];
            return Err(ScannerError::NoDeviceFound);
        }

        // Retain target so it stays alive
        let _: () = msg_send![target_device, retain];

        let dev_name: *mut Object = msg_send![target_device, name];
        eprintln!("[ICA-DEBUG] Target device: {}", nsstring_to_string(dev_name));

        // Keep browser alive — ICA needs it for device connectivity
        // We'll stop it after session is confirmed open

        // Set up device delegate
        let dev_del_cls = device_delegate_class();
        let dev_del: *mut Object = msg_send![dev_del_cls, alloc];
        let dev_del: *mut Object = msg_send![dev_del, init];
        (*dev_del).set_ivar::<u8>("_session_open", 0);
        (*dev_del).set_ivar::<u8>("_scan_done", 0);
        (*dev_del).set_ivar::<u8>("_unit_ready", 0);
        (*dev_del).set_ivar::<*mut Object>("_scan_error", std::ptr::null_mut());
        (*dev_del).set_ivar::<*mut Object>("_functional_unit", std::ptr::null_mut());
        let _: () = msg_send![target_device, setDelegate: dev_del];

        // Request session open in the SAME block as setDelegate
        // ICA needs these to happen in the same run loop turn
        let already_open: BOOL = msg_send![target_device, hasOpenSession];
        if already_open == YES {
            eprintln!("[ICA-DEBUG] Session already open");
            (*dev_del).set_ivar::<u8>("_session_open", 1);
        } else {
            eprintln!("[ICA-DEBUG] Requesting session open...");
            let _: () = msg_send![target_device, requestOpenSession];
        }

        Ok((target_device as usize, dev_del as usize, b, bd, storage))
    });

    let (target_device, dev_del, browser, browser_del, devices_storage) = setup_result?;

    // Phase 4: Wait for session open — poll from this thread
    // 30s timeout to allow sleeping scanners to wake up
    let session_deadline = std::time::Instant::now() + Duration::from_secs(30);
    loop {
        std::thread::sleep(Duration::from_millis(300));

        let dd = dev_del;
        let td = target_device;
        let state: u8 = dispatch_main_short(move || unsafe {
            let dd_obj = dd as *mut Object;
            let td_obj = td as *mut Object;
            let ivar_state: u8 = *(*dd_obj).get_ivar("_session_open");
            if ivar_state != 0 {
                return ivar_state;
            }
            // If functional unit was auto-selected, session IS open
            // (ICA auto-selects FU on session open for some scanners)
            let unit_ready: u8 = *(*dd_obj).get_ivar("_unit_ready");
            if unit_ready != 0 {
                eprintln!("[ICA-DEBUG] unit_ready=1 implies session is open");
                (*dd_obj).set_ivar::<u8>("_session_open", 1);
                return 1;
            }
            // Also check hasOpenSession directly
            let has_open: BOOL = msg_send![td_obj, hasOpenSession];
            if has_open == YES {
                eprintln!("[ICA-DEBUG] hasOpenSession=YES (callback was missed)");
                (*dd_obj).set_ivar::<u8>("_session_open", 1);
                return 1;
            }
            0
        });

        if state != 0 {
            eprintln!("[ICA-DEBUG] Session state: {}", state);
            if state == 2 {
                // Error — cleanup
                let td = target_device;
                let dd = dev_del;
                let b = browser;
                let bd = browser_del;
                let st = devices_storage;
                dispatch_main_short(move || unsafe {
                    let td_obj = td as *mut Object;
                    let dd_obj = dd as *mut Object;
                    let b_obj = b as *mut Object;
                    let bd_obj = bd as *mut Object;
                    let _: () = msg_send![td_obj, requestCloseSession];
                    (*bd_obj).set_ivar::<*mut std::ffi::c_void>("_devices_ptr", std::ptr::null_mut());
                    let _: () = msg_send![b_obj, stop];
                    let devices_vec = &*(st as *const Vec<*mut Object>);
                    for &d in devices_vec.iter() { if d != td_obj { let _: () = msg_send![d, release]; } }
                    let _ = Box::from_raw(st as *mut Vec<*mut Object>);
                    let _: () = msg_send![td_obj, release];
                    let _: () = msg_send![dd_obj, release];
                    let _: () = msg_send![bd_obj, release];
                    let _: () = msg_send![b_obj, release];
                });
                return Err(ScannerError::SystemError(
                    "Le scanner a refusé la connexion. Vérifiez qu'il n'est pas utilisé par une autre application.".into()
                ));
            }
            break; // state == 1, session open
        }

        if std::time::Instant::now() >= session_deadline {
            eprintln!("[ICA-DEBUG] Session timeout!");
            // Cleanup
            let td = target_device;
            let dd = dev_del;
            let b = browser;
            let bd = browser_del;
            let st = devices_storage;
            dispatch_main_short(move || unsafe {
                let td_obj = td as *mut Object;
                let dd_obj = dd as *mut Object;
                let b_obj = b as *mut Object;
                let bd_obj = bd as *mut Object;
                let _: () = msg_send![td_obj, requestCloseSession];
                (*bd_obj).set_ivar::<*mut std::ffi::c_void>("_devices_ptr", std::ptr::null_mut());
                let _: () = msg_send![b_obj, stop];
                let devices_vec = &*(st as *const Vec<*mut Object>);
                for &d in devices_vec.iter() { if d != td_obj { let _: () = msg_send![d, release]; } }
                let _ = Box::from_raw(st as *mut Vec<*mut Object>);
                let _: () = msg_send![td_obj, release];
                let _: () = msg_send![dd_obj, release];
                let _: () = msg_send![bd_obj, release];
                let _: () = msg_send![b_obj, release];
            });
            return Err(ScannerError::SystemError(
                "Impossible d'ouvrir la session avec le scanner (timeout 30s). Vérifiez que le scanner est allumé et connecté.".into()
            ));
        }
    }

    eprintln!("[ICA-DEBUG] Session open! Waiting for functional unit...");

    // Phase 5: Poll for functional unit to become available
    // Network scanners need time after session open to report their FU
    let fu_deadline = std::time::Instant::now() + Duration::from_secs(15);
    loop {
        std::thread::sleep(Duration::from_millis(500));

        let td = target_device;
        let dd = dev_del;
        let fu_info: (bool, String) = dispatch_main_short(move || unsafe {
            let td_obj = td as *mut Object;
            let dd_obj = dd as *mut Object;

            // Check all possible sources
            let stored: *mut Object = *(*dd_obj).get_ivar("_functional_unit");
            let selected: *mut Object = msg_send![td_obj, selectedFunctionalUnit];

            // Also try to get the class name of the device to understand what we're dealing with
            let cls: *mut Object = msg_send![td_obj, class];
            let cls_name: *mut Object = msg_send![cls, description];
            let cls_str = nsstring_to_string(cls_name);

            let info = format!(
                "stored={:p}, selected={:p}, class={}",
                stored, selected, cls_str
            );

            if !stored.is_null() || !selected.is_null() {
                (true, info)
            } else {
                (false, info)
            }
        });

        eprintln!("[ICA-DEBUG] FU poll: {}", fu_info.1);

        if fu_info.0 {
            eprintln!("[ICA-DEBUG] Functional unit found!");
            break;
        }

        if std::time::Instant::now() >= fu_deadline {
            eprintln!("[ICA-DEBUG] FU poll timeout — will try requestSelectFunctionalUnit");

            // Last resort: try requesting one
            let td = target_device;
            let dd = dev_del;
            dispatch_main_short(move || unsafe {
                let td_obj = td as *mut Object;
                let dd_obj = dd as *mut Object;
                (*dd_obj).set_ivar::<u8>("_unit_ready", 0);
                // Try flatbed first, then document feeder
                let _: () = msg_send![td_obj, requestSelectFunctionalUnit: 0u64];
                eprintln!("[ICA-DEBUG] Requested flatbed FU");
            });

            // Wait a bit for the request
            std::thread::sleep(Duration::from_secs(5));
            break;
        }
    }

    // Phase 6: Configure transfer
    let tmp_dir = std::env::temp_dir().join("document-scanner-scans");
    std::fs::create_dir_all(&tmp_dir)
        .map_err(|e| ScannerError::SystemError(format!("Dossier temp: {}", e)))?;
    if let Ok(entries) = std::fs::read_dir(&tmp_dir) {
        for entry in entries.flatten() {
            let _ = std::fs::remove_file(entry.path());
        }
    }

    let tmp_path = tmp_dir.to_string_lossy().to_string();
    let td = target_device;

    dispatch_main_short(move || unsafe {
        let td_obj = td as *mut Object;

        let tmp_cstr = std::ffi::CString::new(tmp_path).unwrap();
        let ns_tmp: *mut Object =
            msg_send![class!(NSString), stringWithUTF8String: tmp_cstr.as_ptr()];
        let ns_url: *mut Object = msg_send![class!(NSURL), fileURLWithPath: ns_tmp];

        let _: () = msg_send![td_obj, setTransferMode: 0u64];
        let _: () = msg_send![td_obj, setDownloadsDirectory: ns_url];

        let doc_name_cstr = std::ffi::CString::new("Scan").unwrap();
        let ns_doc_name: *mut Object =
            msg_send![class!(NSString), stringWithUTF8String: doc_name_cstr.as_ptr()];
        let _: () = msg_send![td_obj, setDocumentName: ns_doc_name];

        let uti_cstr = std::ffi::CString::new("public.tiff").unwrap();
        let ns_uti: *mut Object =
            msg_send![class!(NSString), stringWithUTF8String: uti_cstr.as_ptr()];
        let _: () = msg_send![td_obj, setDocumentUTI: ns_uti];

        eprintln!("[ICA-DEBUG] Transfer configured");
    });

    // Phase 7: Configure scan parameters and start scan
    let opts = options.clone();
    let td = target_device;
    let dd = dev_del;

    dispatch_main_short(move || unsafe {
        let td_obj = td as *mut Object;
        let dd_obj = dd as *mut Object;

        // Try stored FU first, then device's selectedFunctionalUnit
        let mut fu: *mut Object = *(*dd_obj).get_ivar("_functional_unit");
        if fu.is_null() {
            fu = msg_send![td_obj, selectedFunctionalUnit];
        }
        eprintln!("[ICA-DEBUG] Phase 7: functional unit = {:p}", fu);
        if !fu.is_null() {
            let _: () = msg_send![fu, setMeasurementUnit: 0u64];

            let dpi = opts.dpi as u64;
            let _: () = msg_send![fu, setResolution: dpi];

            let (paper_w, paper_h) = paper_dimensions(&opts.paper_format);
            let width_inches = paper_w / 25.4;
            let height_inches = paper_h / 25.4;
            let scan_area = CGRect {
                x: 0.0,
                y: 0.0,
                width: width_inches,
                height: height_inches,
            };
            let _: () = msg_send![fu, setScanArea: scan_area];

            let pixel_data_type: u64 = match color_mode_id(&opts.color_mode) {
                1 => 2,
                2 => 1,
                4 => 0,
                _ => 2,
            };
            let _: () = msg_send![fu, setPixelDataType: pixel_data_type];

            let bit_depth: u64 = if pixel_data_type == 0 { 1 } else { 8 };
            let _: () = msg_send![fu, setBitDepth: bit_depth];

            if opts.duplex {
                let responds: BOOL =
                    msg_send![fu, respondsToSelector: sel!(setDuplexScanningEnabled:)];
                if responds == YES {
                    let _: () = msg_send![fu, setDuplexScanningEnabled: YES];
                }
            }

            eprintln!(
                "[ICA-DEBUG] Scan params: {}dpi, {:.1}x{:.1}in, pixel={}, bits={}",
                dpi, width_inches, height_inches, pixel_data_type, bit_depth
            );
        } else {
            eprintln!("[ICA-DEBUG] WARNING: No FU, trying requestOverviewScan as fallback");
        }

        (*dd_obj).set_ivar::<u8>("_scan_done", 0);

        // If no FU available, try requestOverviewScan as last resort
        if fu.is_null() {
            let responds: BOOL = msg_send![td_obj, respondsToSelector: sel!(requestOverviewScan)];
            if responds == YES {
                eprintln!("[ICA-DEBUG] Trying requestOverviewScan");
                let _: () = msg_send![td_obj, requestOverviewScan];
            } else {
                eprintln!("[ICA-DEBUG] No overview scan either, trying requestScan anyway");
                let _: () = msg_send![td_obj, requestScan];
            }
        } else {
            let _: () = msg_send![td_obj, requestScan];
            eprintln!("[ICA-DEBUG] Scan requested with FU");
        }
    });

    // Phase 8: Wait for scan completion
    let scan_timeout = Duration::from_secs(120);
    let scan_start = std::time::Instant::now();

    let scan_result = loop {
        if scan_start.elapsed() >= scan_timeout {
            eprintln!("[ICA-DEBUG] Scan timed out");
            break Err(ScannerError::SystemError(
                "Délai de numérisation dépassé".into(),
            ));
        }

        std::thread::sleep(Duration::from_secs(1));

        let dd = dev_del;
        let (done, has_error, error_str) = dispatch_main_short(move || unsafe {
            let dd_obj = dd as *mut Object;
            let done: u8 = *(*dd_obj).get_ivar("_scan_done");
            if done == 0 {
                return (false, false, String::new());
            }
            let err: *mut Object = *(*dd_obj).get_ivar("_scan_error");
            if !err.is_null() {
                let desc: *mut Object = msg_send![err, localizedDescription];
                (true, true, nsstring_to_string(desc))
            } else {
                (true, false, String::new())
            }
        });

        if !done {
            continue;
        }

        if has_error {
            break Err(ScannerError::SystemError(format!("Erreur scan: {}", error_str)));
        }

        // Scan succeeded — read output file
        if let Ok(entries) = std::fs::read_dir(&tmp_dir) {
            let files: Vec<_> = entries
                .filter_map(|e| e.ok())
                .filter(|e| {
                    let p = e.path();
                    p.extension().map_or(false, |ext| {
                        let ext = ext.to_string_lossy().to_lowercase();
                        matches!(ext.as_str(), "tiff" | "tif" | "jpeg" | "jpg" | "png" | "pdf")
                    })
                })
                .collect();

            if let Some(entry) = files.last() {
                let file_path = entry.path();
                eprintln!("[ICA-DEBUG] Found scan output: {:?}", file_path);

                if let Ok(data) = std::fs::read(&file_path) {
                    if data.is_empty() {
                        break Err(ScannerError::SystemError("Fichier numérisé vide".into()));
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

        break Err(ScannerError::SystemError(
            "Numérisation terminée mais aucun fichier généré".into(),
        ));
    };

    // Phase 9: Cleanup everything (short block)
    let td = target_device;
    let dd = dev_del;
    let b = browser;
    let bd = browser_del;
    let st = devices_storage;

    dispatch_main_short(move || unsafe {
        let td_obj = td as *mut Object;
        let dd_obj = dd as *mut Object;
        let b_obj = b as *mut Object;
        let bd_obj = bd as *mut Object;

        let _: () = msg_send![td_obj, requestCloseSession];

        // Stop browser and release devices
        (*bd_obj).set_ivar::<*mut std::ffi::c_void>("_devices_ptr", std::ptr::null_mut());
        let _: () = msg_send![b_obj, stop];
        let devices_vec = &*(st as *const Vec<*mut Object>);
        for &d in devices_vec.iter() {
            if d != td_obj { let _: () = msg_send![d, release]; }
        }
        let _ = Box::from_raw(st as *mut Vec<*mut Object>);

        let _: () = msg_send![td_obj, release]; // our extra retain
        let _: () = msg_send![dd_obj, release];
        let _: () = msg_send![bd_obj, release];
        let _: () = msg_send![b_obj, release];
        eprintln!("[ICA-DEBUG] Cleanup done");
    });

    scan_result
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
