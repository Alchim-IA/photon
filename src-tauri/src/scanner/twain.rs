use crate::scanner::*;

// ─── TWAIN Backend ───────────────────────────────────────────────
// Provides TWAIN protocol support on Windows for legacy scanners
// that don't expose a WIA driver.

#[cfg(windows)]
use std::ffi::c_void;
#[cfg(windows)]
use std::sync::{Arc, Mutex};
#[cfg(windows)]
use windows::Win32::Foundation::*;
#[cfg(windows)]
use windows::Win32::System::LibraryLoader::*;
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::*;
#[cfg(windows)]
use windows::core::*;

// ─── TWAIN Constants ─────────────────────────────────────────────

#[cfg(windows)]
mod tw {
    // Data groups
    pub const DG_CONTROL: u32 = 0x0001;
    pub const DG_IMAGE: u32 = 0x0002;

    // DAT (Data Argument Types)
    pub const DAT_CAPABILITY: u16 = 0x0001;
    pub const DAT_IDENTITY: u16 = 0x0006;
    pub const DAT_PARENT: u16 = 0x000C;
    pub const DAT_USERINTERFACE: u16 = 0x0009;
    pub const DAT_IMAGEINFO: u16 = 0x0101;
    pub const DAT_IMAGENATIVEXFER: u16 = 0x0104;
    pub const DAT_PENDINGXFERS: u16 = 0x0005;
    pub const DAT_STATUS: u16 = 0x0008;

    // MSG (Messages)
    pub const MSG_GET: u16 = 0x0001;
    pub const MSG_SET: u16 = 0x0006;
    pub const MSG_GETFIRST: u16 = 0x0004;
    pub const MSG_GETNEXT: u16 = 0x0005;
    pub const MSG_OPENDSM: u16 = 0x0301;
    pub const MSG_CLOSEDSM: u16 = 0x0302;
    pub const MSG_OPENDS: u16 = 0x0401;
    pub const MSG_CLOSEDS: u16 = 0x0402;
    pub const MSG_ENABLEDS: u16 = 0x0501;
    pub const MSG_DISABLEDS: u16 = 0x0502;
    pub const MSG_RESET: u16 = 0x0007;
    pub const MSG_ENDXFER: u16 = 0x0701;

    // Return codes
    pub const TWRC_SUCCESS: u16 = 0;
    pub const TWRC_FAILURE: u16 = 1;
    pub const TWRC_ENDOFLIST: u16 = 7;
    pub const TWRC_XFERDONE: u16 = 0;

    // Capabilities
    pub const CAP_XFERCOUNT: u16 = 0x0001;
    pub const CAP_FEEDERENABLED: u16 = 0x1002;
    pub const CAP_DUPLEXENABLED: u16 = 0x1013;
    pub const ICAP_XRESOLUTION: u16 = 0x1118;
    pub const ICAP_YRESOLUTION: u16 = 0x1119;
    pub const ICAP_PIXELTYPE: u16 = 0x0101;
    pub const ICAP_UNITS: u16 = 0x0102;

    // Pixel types
    pub const TWPT_BW: u16 = 0;
    pub const TWPT_GRAY: u16 = 1;
    pub const TWPT_RGB: u16 = 2;

    // Container types
    pub const TWON_ONEVALUE: u16 = 5;

    // Units
    pub const TWUN_INCHES: u16 = 0;

    // Supported groups
    pub const DG_APP2: u32 = 0x0003;
}

// ─── TWAIN Types (FFI) ──────────────────────────────────────────

#[cfg(windows)]
#[repr(C)]
#[derive(Clone, Copy)]
struct TwFix32 {
    whole: i16,
    frac: u16,
}

#[cfg(windows)]
impl TwFix32 {
    fn from_f64(val: f64) -> Self {
        let whole = val.floor() as i16;
        let frac = ((val - whole as f64) * 65536.0) as u16;
        Self { whole, frac }
    }

    fn to_f64(self) -> f64 {
        self.whole as f64 + self.frac as f64 / 65536.0
    }
}

#[cfg(windows)]
#[repr(C)]
#[derive(Clone, Copy)]
struct TwVersion {
    major_num: u16,
    minor_num: u16,
    language: u16,
    country: u16,
    info: [u8; 34],
}

#[cfg(windows)]
#[repr(C)]
#[derive(Clone, Copy)]
struct TwIdentity {
    id: u32,
    version: TwVersion,
    protocol_major: u16,
    protocol_minor: u16,
    supported_groups: u32,
    manufacturer: [u8; 34],
    product_family: [u8; 34],
    product_name: [u8; 34],
}

#[cfg(windows)]
impl TwIdentity {
    fn zeroed() -> Self {
        unsafe { std::mem::zeroed() }
    }

    fn app_identity() -> Self {
        let mut id = Self::zeroed();
        id.version.major_num = 1;
        id.version.minor_num = 0;
        id.version.language = 13; // TWLG_FRENCH
        id.version.country = 33;  // TWCY_FRANCE
        copy_str(&mut id.version.info, "1.0");
        id.protocol_major = 2;
        id.protocol_minor = 4;
        id.supported_groups = tw::DG_APP2 | tw::DG_CONTROL as u32 | tw::DG_IMAGE as u32;
        copy_str(&mut id.manufacturer, "Photon");
        copy_str(&mut id.product_family, "Scanner");
        copy_str(&mut id.product_name, "Photon Scanner");
        id
    }

    fn product_name_str(&self) -> String {
        read_tw_str(&self.product_name)
    }

    fn manufacturer_str(&self) -> String {
        read_tw_str(&self.manufacturer)
    }
}

#[cfg(windows)]
fn copy_str(dest: &mut [u8], src: &str) {
    let bytes = src.as_bytes();
    let len = bytes.len().min(dest.len() - 1);
    dest[..len].copy_from_slice(&bytes[..len]);
    dest[len] = 0;
}

#[cfg(windows)]
fn read_tw_str(buf: &[u8]) -> String {
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    String::from_utf8_lossy(&buf[..end]).to_string()
}

#[cfg(windows)]
#[repr(C)]
#[derive(Clone, Copy)]
struct TwUserInterface {
    show_ui: i16,
    modal_ui: i16,
    parent: isize, // HWND
}

#[cfg(windows)]
#[repr(C)]
#[derive(Clone, Copy)]
struct TwCapability {
    cap: u16,
    con_type: u16,
    container: *mut c_void,
}

#[cfg(windows)]
#[repr(C)]
#[derive(Clone, Copy)]
struct TwOneValue {
    item_type: u16,
    item: u32,
}

#[cfg(windows)]
#[repr(C)]
#[derive(Clone, Copy)]
struct TwImageInfo {
    x_resolution: TwFix32,
    y_resolution: TwFix32,
    image_width: i32,
    image_length: i32,
    samples_per_pixel: i16,
    bits_per_sample: [i16; 8],
    bits_per_pixel: i16,
    planar: i16,
    pixel_type: i16,
    compression: u16,
}

#[cfg(windows)]
#[repr(C)]
#[derive(Clone, Copy)]
struct TwPendingXfers {
    count: u16,
    _eoj: u32,
}

#[cfg(windows)]
#[repr(C)]
#[derive(Clone, Copy)]
struct TwStatus {
    condition_code: u16,
    _data: u16,
}

// ─── DSM Entry Function Type ────────────────────────────────────

#[cfg(windows)]
type DsmEntryFn = unsafe extern "system" fn(
    *mut TwIdentity,       // pOrigin (app)
    *mut TwIdentity,       // pDest (source)
    u32,                   // DG
    u16,                   // DAT
    u16,                   // MSG
    *mut c_void,           // pData
) -> u16;

// ─── TWAIN Backend ──────────────────────────────────────────────

pub struct TwainBackend {
    #[cfg(windows)]
    dsm_entry: Option<DsmEntryFn>,
    #[cfg(windows)]
    _lib: Option<HMODULE>,
}

impl TwainBackend {
    pub fn new() -> Self {
        #[cfg(windows)]
        {
            // Try loading TWAINDSM.dll (new DSM) then twain_32.dll (legacy)
            let (entry, lib) = load_dsm("TWAINDSM.dll")
                .or_else(|| load_dsm("twain_32.dll"))
                .unwrap_or((None, None));

            if entry.is_some() {
                log::info!("TWAIN DSM chargé avec succès");
            } else {
                log::debug!("TWAIN DSM non disponible");
            }

            Self {
                dsm_entry: entry,
                _lib: lib,
            }
        }
        #[cfg(not(windows))]
        Self {}
    }
}

#[cfg(windows)]
fn load_dsm(dll_name: &str) -> Option<(Option<DsmEntryFn>, Option<HMODULE>)> {
    unsafe {
        let dll_wide: Vec<u16> = dll_name.encode_utf16().chain(std::iter::once(0)).collect();
        let lib = LoadLibraryW(PCWSTR(dll_wide.as_ptr())).ok()?;
        let proc_name = s!("DSM_Entry");
        let proc = GetProcAddress(lib, proc_name)?;
        let entry: DsmEntryFn = std::mem::transmute(proc);
        Some((Some(entry), Some(lib)))
    }
}

// ─── TWAIN Session Helper ───────────────────────────────────────

#[cfg(windows)]
struct TwainSession {
    dsm_entry: DsmEntryFn,
    app_id: TwIdentity,
    hwnd: HWND,
    dsm_open: bool,
}

#[cfg(windows)]
impl TwainSession {
    fn new(dsm_entry: DsmEntryFn) -> Result<Self, ScannerError> {
        // Create a hidden message-only window for TWAIN
        let hwnd = unsafe {
            let class_name = w!("PhotonTwainHidden");
            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: Some(DefWindowProcW),
                lpszClassName: class_name,
                ..std::mem::zeroed()
            };
            RegisterClassExW(&wc);
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                class_name,
                w!("Photon TWAIN"),
                WS_OVERLAPPEDWINDOW,
                0, 0, 0, 0,
                HWND_MESSAGE,
                None,
                None,
                None,
            )
            .map_err(|e| ScannerError::SystemError(format!("Fenêtre TWAIN: {}", e)))?
        };

        let mut app_id = TwIdentity::app_identity();

        // Open DSM
        let rc = unsafe {
            (dsm_entry)(
                &mut app_id,
                std::ptr::null_mut(),
                tw::DG_CONTROL,
                tw::DAT_PARENT,
                tw::MSG_OPENDSM,
                &mut hwnd.0 as *mut isize as *mut c_void,
            )
        };

        if rc != tw::TWRC_SUCCESS {
            unsafe { let _ = DestroyWindow(hwnd); }
            return Err(ScannerError::SystemError(format!("TWAIN DSM_Open: rc={}", rc)));
        }

        Ok(Self {
            dsm_entry,
            app_id,
            hwnd,
            dsm_open: true,
        })
    }

    fn call(
        &mut self,
        dest: *mut TwIdentity,
        dg: u32,
        dat: u16,
        msg: u16,
        data: *mut c_void,
    ) -> u16 {
        unsafe { (self.dsm_entry)(&mut self.app_id, dest, dg, dat, msg, data) }
    }

    fn enumerate_sources(&mut self) -> Vec<TwIdentity> {
        let mut sources = Vec::new();
        let mut source = TwIdentity::zeroed();

        let rc = self.call(
            std::ptr::null_mut(),
            tw::DG_CONTROL,
            tw::DAT_IDENTITY,
            tw::MSG_GETFIRST,
            &mut source as *mut _ as *mut c_void,
        );

        if rc == tw::TWRC_SUCCESS {
            sources.push(source);

            loop {
                let mut next = TwIdentity::zeroed();
                let rc = self.call(
                    std::ptr::null_mut(),
                    tw::DG_CONTROL,
                    tw::DAT_IDENTITY,
                    tw::MSG_GETNEXT,
                    &mut next as *mut _ as *mut c_void,
                );
                if rc != tw::TWRC_SUCCESS {
                    break;
                }
                sources.push(next);
            }
        }

        sources
    }

    fn open_source(&mut self, source: &mut TwIdentity) -> Result<(), ScannerError> {
        let rc = self.call(
            std::ptr::null_mut(),
            tw::DG_CONTROL,
            tw::DAT_IDENTITY,
            tw::MSG_OPENDS,
            source as *mut _ as *mut c_void,
        );
        if rc != tw::TWRC_SUCCESS {
            return Err(ScannerError::SystemError(format!(
                "TWAIN: impossible d'ouvrir la source '{}'",
                source.product_name_str()
            )));
        }
        Ok(())
    }

    fn close_source(&mut self, source: &mut TwIdentity) {
        self.call(
            std::ptr::null_mut(),
            tw::DG_CONTROL,
            tw::DAT_IDENTITY,
            tw::MSG_CLOSEDS,
            source as *mut _ as *mut c_void,
        );
    }

    fn set_capability_u16(&mut self, source: &mut TwIdentity, cap_id: u16, value: u16) -> bool {
        let one_val = Box::new(TwOneValue {
            item_type: 4, // TWTY_UINT16
            item: value as u32,
        });
        let mut cap = TwCapability {
            cap: cap_id,
            con_type: tw::TWON_ONEVALUE,
            container: Box::into_raw(one_val) as *mut c_void,
        };
        let rc = self.call(
            source,
            tw::DG_CONTROL,
            tw::DAT_CAPABILITY,
            tw::MSG_SET,
            &mut cap as *mut _ as *mut c_void,
        );
        // Free the container
        if !cap.container.is_null() {
            unsafe { let _ = Box::from_raw(cap.container as *mut TwOneValue); }
        }
        rc == tw::TWRC_SUCCESS
    }

    fn set_capability_fix32(&mut self, source: &mut TwIdentity, cap_id: u16, value: f64) -> bool {
        let fix = TwFix32::from_f64(value);
        let combined = ((fix.whole as u32) << 16) | (fix.frac as u32);
        let one_val = Box::new(TwOneValue {
            item_type: 7, // TWTY_FIX32
            item: combined,
        });
        let mut cap = TwCapability {
            cap: cap_id,
            con_type: tw::TWON_ONEVALUE,
            container: Box::into_raw(one_val) as *mut c_void,
        };
        let rc = self.call(
            source,
            tw::DG_CONTROL,
            tw::DAT_CAPABILITY,
            tw::MSG_SET,
            &mut cap as *mut _ as *mut c_void,
        );
        if !cap.container.is_null() {
            unsafe { let _ = Box::from_raw(cap.container as *mut TwOneValue); }
        }
        rc == tw::TWRC_SUCCESS
    }

    fn enable_source_no_ui(&mut self, source: &mut TwIdentity) -> Result<(), ScannerError> {
        let mut ui = TwUserInterface {
            show_ui: 0, // No UI
            modal_ui: 0,
            parent: self.hwnd.0,
        };
        let rc = self.call(
            source,
            tw::DG_CONTROL,
            tw::DAT_USERINTERFACE,
            tw::MSG_ENABLEDS,
            &mut ui as *mut _ as *mut c_void,
        );
        if rc != tw::TWRC_SUCCESS {
            return Err(ScannerError::SystemError(
                "TWAIN: impossible d'activer la source sans UI".to_string(),
            ));
        }
        Ok(())
    }

    fn disable_source(&mut self, source: &mut TwIdentity) {
        let mut ui = TwUserInterface {
            show_ui: 0,
            modal_ui: 0,
            parent: self.hwnd.0,
        };
        self.call(
            source,
            tw::DG_CONTROL,
            tw::DAT_USERINTERFACE,
            tw::MSG_DISABLEDS,
            &mut ui as *mut _ as *mut c_void,
        );
    }

    fn get_image_info(&mut self, source: &mut TwIdentity) -> Result<TwImageInfo, ScannerError> {
        let mut info: TwImageInfo = unsafe { std::mem::zeroed() };
        let rc = self.call(
            source,
            tw::DG_IMAGE,
            tw::DAT_IMAGEINFO,
            tw::MSG_GET,
            &mut info as *mut _ as *mut c_void,
        );
        if rc != tw::TWRC_SUCCESS {
            return Err(ScannerError::SystemError("TWAIN: impossible de lire les infos image".to_string()));
        }
        Ok(info)
    }

    fn native_transfer(&mut self, source: &mut TwIdentity) -> Result<Vec<u8>, ScannerError> {
        let mut handle: isize = 0;
        let rc = self.call(
            source,
            tw::DG_IMAGE,
            tw::DAT_IMAGENATIVEXFER,
            tw::MSG_GET,
            &mut handle as *mut _ as *mut c_void,
        );
        if rc != tw::TWRC_XFERDONE {
            return Err(ScannerError::SystemError(format!("TWAIN: échec transfert natif (rc={})", rc)));
        }
        if handle == 0 {
            return Err(ScannerError::SystemError("TWAIN: handle DIB nul".to_string()));
        }

        // Convert DIB handle to image bytes
        let data = unsafe { dib_handle_to_bmp(handle as *mut c_void) };

        // Free the DIB handle
        unsafe {
            windows::Win32::System::Memory::GlobalFree(windows::Win32::Foundation::HGLOBAL(handle as *mut c_void));
        }

        data
    }

    fn end_transfer(&mut self, source: &mut TwIdentity) {
        let mut pending: TwPendingXfers = unsafe { std::mem::zeroed() };
        self.call(
            source,
            tw::DG_CONTROL,
            tw::DAT_PENDINGXFERS,
            tw::MSG_ENDXFER,
            &mut pending as *mut _ as *mut c_void,
        );
        // Reset if more pending
        if pending.count != 0 {
            self.call(
                source,
                tw::DG_CONTROL,
                tw::DAT_PENDINGXFERS,
                tw::MSG_RESET,
                &mut pending as *mut _ as *mut c_void,
            );
        }
    }
}

#[cfg(windows)]
impl Drop for TwainSession {
    fn drop(&mut self) {
        if self.dsm_open {
            let mut hwnd_val = self.hwnd.0;
            unsafe {
                (self.dsm_entry)(
                    &mut self.app_id,
                    std::ptr::null_mut(),
                    tw::DG_CONTROL,
                    tw::DAT_PARENT,
                    tw::MSG_CLOSEDSM,
                    &mut hwnd_val as *mut isize as *mut c_void,
                );
                let _ = DestroyWindow(self.hwnd);
            }
        }
    }
}

// ─── DIB to BMP conversion ──────────────────────────────────────

#[cfg(windows)]
unsafe fn dib_handle_to_bmp(handle: *mut c_void) -> Result<Vec<u8>, ScannerError> {
    use windows::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};

    let ptr = GlobalLock(HGLOBAL(handle));
    if ptr.is_null() {
        return Err(ScannerError::SystemError("TWAIN: GlobalLock échoué".to_string()));
    }

    let size = GlobalSize(HGLOBAL(handle));
    if size == 0 {
        GlobalUnlock(HGLOBAL(handle));
        return Err(ScannerError::SystemError("TWAIN: taille DIB nulle".to_string()));
    }

    // The native transfer returns a packed DIB (BITMAPINFOHEADER + palette + pixels)
    // We need to add a BITMAPFILEHEADER to make it a valid BMP
    let dib_data = std::slice::from_raw_parts(ptr as *const u8, size);

    // Read BITMAPINFOHEADER size to determine offset to pixel data
    let header_size = if dib_data.len() >= 4 {
        u32::from_le_bytes([dib_data[0], dib_data[1], dib_data[2], dib_data[3]])
    } else {
        40
    };

    // Calculate palette size
    let bits_per_pixel = if dib_data.len() >= 18 {
        u16::from_le_bytes([dib_data[14], dib_data[15]])
    } else {
        24
    };
    let colors_used = if dib_data.len() >= 36 {
        u32::from_le_bytes([dib_data[32], dib_data[33], dib_data[34], dib_data[35]])
    } else {
        0
    };
    let palette_entries = if bits_per_pixel <= 8 {
        if colors_used > 0 { colors_used } else { 1u32 << bits_per_pixel }
    } else {
        0
    };
    let palette_size = palette_entries * 4; // RGBQUAD = 4 bytes

    let pixel_offset = header_size + palette_size;
    let file_size = 14 + dib_data.len() as u32; // 14 = BITMAPFILEHEADER size

    // Build BMP file
    let mut bmp = Vec::with_capacity(file_size as usize);

    // BITMAPFILEHEADER (14 bytes)
    bmp.extend_from_slice(b"BM");                              // bfType
    bmp.extend_from_slice(&file_size.to_le_bytes());           // bfSize
    bmp.extend_from_slice(&0u16.to_le_bytes());                // bfReserved1
    bmp.extend_from_slice(&0u16.to_le_bytes());                // bfReserved2
    bmp.extend_from_slice(&(14 + pixel_offset).to_le_bytes()); // bfOffBits

    // Append the DIB data (header + palette + pixels)
    bmp.extend_from_slice(dib_data);

    GlobalUnlock(HGLOBAL(handle));

    Ok(bmp)
}

// ─── ScannerBackend Implementation ──────────────────────────────

impl ScannerBackend for TwainBackend {
    fn list_devices(&self) -> std::result::Result<Vec<ScannerDevice>, ScannerError> {
        #[cfg(windows)]
        {
            let dsm_entry = match self.dsm_entry {
                Some(e) => e,
                None => return Ok(Vec::new()), // No TWAIN DSM available
            };

            let mut session = TwainSession::new(dsm_entry)?;
            let sources = session.enumerate_sources();

            let devices: Vec<ScannerDevice> = sources
                .iter()
                .map(|src| {
                    let name = src.product_name_str();
                    let vendor = src.manufacturer_str();
                    ScannerDevice {
                        id: format!("twain://{}", src.id),
                        name: format!("{} (TWAIN)", name),
                        vendor,
                        capabilities: ScannerCapabilities::default(),
                    }
                })
                .collect();

            log::info!("TWAIN: {} source(s) trouvée(s)", devices.len());
            Ok(devices)
        }
        #[cfg(not(windows))]
        {
            Ok(Vec::new())
        }
    }

    fn scan(&self, options: ScanOptions) -> std::result::Result<ScanResult, ScannerError> {
        #[cfg(windows)]
        {
            let dsm_entry = self.dsm_entry
                .ok_or_else(|| ScannerError::NoDriver)?;

            // Parse TWAIN device ID
            let source_id: u32 = options.device_id
                .strip_prefix("twain://")
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| ScannerError::SystemError("ID TWAIN invalide".to_string()))?;

            let mut session = TwainSession::new(dsm_entry)?;

            // Find the source by ID
            let sources = session.enumerate_sources();
            let mut source = sources
                .into_iter()
                .find(|s| s.id == source_id)
                .ok_or(ScannerError::NoDeviceFound)?;

            // Open the source
            session.open_source(&mut source)?;

            // Set capabilities
            // Units = inches (required for resolution)
            session.set_capability_u16(&mut source, tw::ICAP_UNITS, tw::TWUN_INCHES);

            // Resolution
            let dpi = options.dpi as f64;
            session.set_capability_fix32(&mut source, tw::ICAP_XRESOLUTION, dpi);
            session.set_capability_fix32(&mut source, tw::ICAP_YRESOLUTION, dpi);

            // Color mode
            let pixel_type = match color_mode_id(&options.color_mode) {
                1 => tw::TWPT_RGB,
                2 => tw::TWPT_GRAY,
                4 => tw::TWPT_BW,
                _ => tw::TWPT_RGB,
            };
            session.set_capability_u16(&mut source, tw::ICAP_PIXELTYPE, pixel_type);

            // Transfer count = 1
            session.set_capability_u16(&mut source, tw::CAP_XFERCOUNT, 1);

            // Duplex / ADF
            if options.duplex {
                session.set_capability_u16(&mut source, tw::CAP_FEEDERENABLED, 1);
                session.set_capability_u16(&mut source, tw::CAP_DUPLEXENABLED, 1);
            }

            // Enable source without UI
            if let Err(e) = session.enable_source_no_ui(&mut source) {
                session.close_source(&mut source);
                return Err(e);
            }

            // Process Windows messages to let TWAIN work
            // TWAIN needs message pump for state transitions
            unsafe {
                let mut msg = MSG::default();
                let start = std::time::Instant::now();
                let timeout = std::time::Duration::from_secs(30);

                // Pump messages until transfer is ready or timeout
                loop {
                    if start.elapsed() > timeout {
                        session.disable_source(&mut source);
                        session.close_source(&mut source);
                        return Err(ScannerError::SystemError("TWAIN: timeout en attente du scanner".to_string()));
                    }

                    while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                        let _ = TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }

                    // Small sleep to avoid busy-waiting
                    std::thread::sleep(std::time::Duration::from_millis(50));

                    // Try to get image info - if it succeeds, transfer is ready
                    if session.get_image_info(&mut source).is_ok() {
                        break;
                    }
                }
            }

            // Perform native transfer
            let bmp_data = match session.native_transfer(&mut source) {
                Ok(data) => data,
                Err(e) => {
                    session.end_transfer(&mut source);
                    session.disable_source(&mut source);
                    session.close_source(&mut source);
                    return Err(e);
                }
            };

            // End transfer and clean up
            session.end_transfer(&mut source);
            session.disable_source(&mut source);
            session.close_source(&mut source);

            // Decode BMP and convert to PNG
            let img = ::image::load_from_memory(&bmp_data)
                .map_err(|e| ScannerError::SystemError(format!("TWAIN: décodage image: {}", e)))?;

            let width = img.width();
            let height = img.height();

            let mut png_bytes = Vec::new();
            let mut cursor = std::io::Cursor::new(&mut png_bytes);
            img.write_to(&mut cursor, ::image::ImageFormat::Png)
                .map_err(|e| ScannerError::SystemError(format!("TWAIN: encodage PNG: {}", e)))?;

            log::info!("TWAIN: scan réussi {}x{}", width, height);

            Ok(ScanResult {
                image_data: png_bytes,
                width,
                height,
            })
        }
        #[cfg(not(windows))]
        {
            let _ = options;
            Err(ScannerError::SystemError("TWAIN n'est disponible que sur Windows".to_string()))
        }
    }
}
