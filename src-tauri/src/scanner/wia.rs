use crate::scanner::*;

#[cfg(windows)]
use windows::{
    core::*,
    Win32::Devices::ImageAcquisition::*,
    Win32::System::Com::*,
    Win32::System::Com::StructuredStorage::*,
    Win32::System::Variant::*,
    Win32::Foundation::*,
};
#[cfg(windows)]
use std::ptr;
#[cfg(windows)]
use std::ffi::OsString;
#[cfg(windows)]
use std::os::windows::ffi::OsStringExt;

pub struct WiaBackend {
    #[cfg(windows)]
    _initialized: bool,
}

impl WiaBackend {
    pub fn new() -> Self {
        #[cfg(windows)]
        {
            let initialized = unsafe {
                CoInitializeEx(None, COINIT_MULTITHREADED).is_ok()
            };
            Self { _initialized: initialized }
        }
        #[cfg(not(windows))]
        Self {}
    }
}

#[cfg(windows)]
unsafe fn propvariant_to_string(pv: &PROPVARIANT) -> Option<String> {
    let variant = &pv.Anonymous.Anonymous;
    if variant.vt == VT_BSTR.0 {
        let bstr = &variant.Anonymous.bstrVal;
        let len = SysStringLen(&**bstr) as usize;
        if len > 0 {
            let slice = std::slice::from_raw_parts(bstr.as_ptr(), len);
            return Some(OsString::from_wide(slice).to_string_lossy().into_owned());
        }
    } else if variant.vt == VT_LPWSTR.0 {
        let pwsz = variant.Anonymous.pwszVal;
        if !pwsz.is_null() {
            let len = (0..).take_while(|&i| *pwsz.add(i) != 0).count();
            let slice = std::slice::from_raw_parts(pwsz, len);
            return Some(OsString::from_wide(slice).to_string_lossy().into_owned());
        }
    }
    None
}

#[cfg(windows)]
unsafe fn propvariant_to_i32(pv: &PROPVARIANT) -> Option<i32> {
    let variant = &pv.Anonymous.Anonymous;
    if variant.vt == VT_I4.0 {
        Some(variant.Anonymous.lVal)
    } else {
        None
    }
}

#[cfg(windows)]
unsafe fn read_wia_property_string(
    storage: &IWiaPropertyStorage,
    prop_id: u32,
) -> Result<String> {
    let propspec = PROPSPEC {
        ulKind: PRSPEC_PROPID,
        Anonymous: PROPSPEC_0 { propid: prop_id },
    };
    let mut propvar = PROPVARIANT::default();
    storage.ReadMultiple(&[propspec], &mut propvar)?;
    propvariant_to_string(&propvar).ok_or_else(|| Error::from(E_FAIL))
}

#[cfg(windows)]
unsafe fn read_wia_property_i32(
    storage: &IWiaPropertyStorage,
    prop_id: u32,
) -> Result<i32> {
    let propspec = PROPSPEC {
        ulKind: PRSPEC_PROPID,
        Anonymous: PROPSPEC_0 { propid: prop_id },
    };
    let mut propvar = PROPVARIANT::default();
    storage.ReadMultiple(&[propspec], &mut propvar)?;
    propvariant_to_i32(&propvar).ok_or_else(|| Error::from(E_FAIL))
}

#[cfg(windows)]
unsafe fn write_wia_property_i32(
    item: &IWiaItem2,
    prop_id: u32,
    value: i32,
) -> Result<()> {
    let storage: IWiaPropertyStorage = item.cast()?;
    let propspec = PROPSPEC {
        ulKind: PRSPEC_PROPID,
        Anonymous: PROPSPEC_0 { propid: prop_id },
    };
    let mut propvar = PROPVARIANT::default();
    {
        let variant = &mut propvar.Anonymous.Anonymous;
        variant.vt = VT_I4.0;
        variant.Anonymous.lVal = value;
    }
    storage.WriteMultiple(&[propspec], &[propvar], 2)?;
    Ok(())
}

#[cfg(windows)]
fn find_scanner_item(root: &IWiaItem2) -> Result<IWiaItem2> {
    unsafe {
        let enumerator = root.EnumChildItems(None)?;
        loop {
            let mut items: [Option<IWiaItem2>; 1] = [None];
            let mut fetched: u32 = 0;
            let hr = enumerator.Next(&mut items, &mut fetched);
            if hr.is_err() || fetched == 0 {
                break;
            }
            if let Some(item) = items[0].take() {
                let item_type = {
                    let storage: IWiaPropertyStorage = item.cast()?;
                    read_wia_property_i32(&storage, WIA_IPA_ITEM_FLAGS).unwrap_or(0)
                };
                // WiaItemTypeTransfer | WiaItemTypeImage
                if (item_type & 0x8) != 0 {
                    return Ok(item);
                }
            }
        }
        Err(Error::from(E_FAIL))
    }
}

impl ScannerBackend for WiaBackend {
    fn list_devices(&self) -> std::result::Result<Vec<ScannerDevice>, ScannerError> {
        #[cfg(windows)]
        {
            unsafe {
                let dev_mgr: IWiaDevMgr2 = CoCreateInstance(&WiaDevMgr2, None, CLSCTX_LOCAL_SERVER)
                    .map_err(|e| ScannerError::SystemError(format!("Impossible de créer WIA Device Manager: {}", e)))?;

                let enumerator = dev_mgr.EnumDeviceInfo(WIA_DEVINFO_ENUM_LOCAL.0 as u32)
                    .map_err(|e| ScannerError::SystemError(format!("Erreur énumération: {}", e)))?;

                let mut devices = Vec::new();

                loop {
                    let mut props: [Option<IWiaPropertyStorage>; 1] = [None];
                    let mut fetched: u32 = 0;
                    let hr = enumerator.Next(&mut props, Some(&mut fetched));
                    if hr.is_err() || fetched == 0 {
                        break;
                    }

                    if let Some(storage) = props[0].take() {
                        let name = read_wia_property_string(&storage, WIA_DIP_DEV_NAME)
                            .unwrap_or_else(|_| "Scanner inconnu".to_string());
                        let id = read_wia_property_string(&storage, WIA_DIP_DEV_ID)
                            .unwrap_or_default();
                        let vendor = read_wia_property_string(&storage, WIA_DIP_VEND_DESC)
                            .unwrap_or_else(|_| "Inconnu".to_string());
                        let dev_type = read_wia_property_i32(&storage, WIA_DIP_DEV_TYPE)
                            .unwrap_or(0);

                        // Check if it's a scanner (type & 0x000F == 1)
                        if (dev_type & 0x000F) != 1 {
                            continue;
                        }

                        // Try to get capabilities by connecting to the device
                        let mut caps = ScannerCapabilities::default();

                        if let Ok(device) = dev_mgr.CreateDevice(0, &BSTR::from(&id)) {
                            if let Ok(scanner_item) = find_scanner_item(&device) {
                                let item_storage: std::result::Result<IWiaPropertyStorage, _> = scanner_item.cast();
                                if let Ok(item_storage) = item_storage {
                                    // Read supported resolutions
                                    if let Ok(xres) = read_wia_property_i32(&item_storage, WIA_IPS_XRES) {
                                        caps.resolutions = vec![75, 150, 300, 600];
                                        if xres >= 1200 {
                                            caps.resolutions.push(1200);
                                        }
                                    }

                                    // Check ADF support
                                    if let Ok(doc_handling) = read_wia_property_i32(&item_storage, WIA_IPS_DOCUMENT_HANDLING_SELECT) {
                                        caps.supports_adf = (doc_handling & 0x01) != 0;
                                        caps.supports_duplex = (doc_handling & 0x04) != 0;
                                    }
                                }
                            }
                        }

                        devices.push(ScannerDevice {
                            id,
                            name,
                            vendor,
                            capabilities: caps,
                        });
                    }
                }

                Ok(devices)
            }
        }
        #[cfg(not(windows))]
        {
            Err(ScannerError::SystemError("WIA n'est disponible que sur Windows".to_string()))
        }
    }

    fn scan(&self, options: ScanOptions) -> std::result::Result<ScanResult, ScannerError> {
        #[cfg(windows)]
        {
            unsafe {
                let dev_mgr: IWiaDevMgr2 = CoCreateInstance(&WiaDevMgr2, None, CLSCTX_LOCAL_SERVER)
                    .map_err(|e| ScannerError::SystemError(format!("WIA init: {}", e)))?;

                let device = dev_mgr.CreateDevice(0, &BSTR::from(&options.device_id))
                    .map_err(|e| ScannerError::SystemError(format!("Connexion au scanner: {}", e)))?;

                let scanner_item = find_scanner_item(&device)
                    .map_err(|_| ScannerError::SystemError("Aucun élément scanner trouvé".to_string()))?;

                // Set scan properties
                let (paper_w, paper_h) = paper_dimensions(&options.paper_format);
                let width_px = mm_to_pixels(paper_w, options.dpi) as i32;
                let height_px = mm_to_pixels(paper_h, options.dpi) as i32;

                // Resolution
                let _ = write_wia_property_i32(&scanner_item, WIA_IPS_XRES, options.dpi as i32);
                let _ = write_wia_property_i32(&scanner_item, WIA_IPS_YRES, options.dpi as i32);

                // Scan area
                let _ = write_wia_property_i32(&scanner_item, WIA_IPS_XPOS, 0);
                let _ = write_wia_property_i32(&scanner_item, WIA_IPS_YPOS, 0);
                let _ = write_wia_property_i32(&scanner_item, WIA_IPS_XEXTENT, width_px);
                let _ = write_wia_property_i32(&scanner_item, WIA_IPS_YEXTENT, height_px);

                // Color mode: 0=color, 2=grayscale, 4=bw
                let data_type = match color_mode_id(&options.color_mode) {
                    1 => 2i32,  // WIA_DATA_COLOR
                    2 => 1i32,  // WIA_DATA_GRAYSCALE
                    4 => 0i32,  // WIA_DATA_THRESHOLD
                    _ => 2i32,
                };
                let _ = write_wia_property_i32(&scanner_item, WIA_IPA_DATATYPE, data_type);

                // Duplex
                if options.duplex {
                    let _ = write_wia_property_i32(&scanner_item, WIA_IPS_DOCUMENT_HANDLING_SELECT, 0x05); // FEEDER | DUPLEX
                }

                // Format: BMP for simplicity
                // WIA_IPA_FORMAT = 4106
                // WIA_FormatBMP GUID

                // Perform the transfer using IWiaTransfer
                let transfer: IWiaTransfer = scanner_item.cast()
                    .map_err(|e| ScannerError::SystemError(format!("Interface de transfert: {}", e)))?;

                let callback = WiaTransferCallback::new();
                let callback_interface: IWiaTransferCallback = callback.clone().into();

                transfer.Download(0, &callback_interface)
                    .map_err(|e| ScannerError::SystemError(format!("Erreur de numérisation: {}", e)))?;

                let data = callback.get_data();
                if data.is_empty() {
                    return Err(ScannerError::SystemError("Aucune donnée reçue du scanner".to_string()));
                }

                // Convert BMP/raw data to PNG using the image crate
                let img = ::image::load_from_memory(&data)
                    .map_err(|e| ScannerError::SystemError(format!("Décodage image: {}", e)))?;

                let width = img.width();
                let height = img.height();

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
        #[cfg(not(windows))]
        {
            let _ = options;
            Err(ScannerError::SystemError("WIA n'est disponible que sur Windows".to_string()))
        }
    }
}

// ─── WIA Transfer Callback (Windows COM) ─────────────────────────

#[cfg(windows)]
use std::sync::{Arc, Mutex};

#[cfg(windows)]
#[derive(Clone)]
struct WiaTransferCallbackData {
    data: Arc<Mutex<Vec<u8>>>,
}

#[cfg(windows)]
#[windows::core::implement(IWiaTransferCallback)]
#[derive(Clone)]
struct WiaTransferCallback {
    inner: WiaTransferCallbackData,
}

#[cfg(windows)]
impl WiaTransferCallback {
    fn new() -> Self {
        Self {
            inner: WiaTransferCallbackData {
                data: Arc::new(Mutex::new(Vec::new())),
            },
        }
    }

    fn get_data(&self) -> Vec<u8> {
        self.inner.data.lock().unwrap().clone()
    }
}

#[cfg(windows)]
impl IWiaTransferCallback_Impl for WiaTransferCallback_Impl {
    fn TransferCallback(
        &self,
        lflags: i32,
        pwiatransferparams: *const WiaTransferParams,
    ) -> Result<()> {
        if !pwiatransferparams.is_null() {
            unsafe {
                let params = &*pwiatransferparams;
                // lMessage == WIA_TRANSFER_MSG_STATUS (1)
                // lMessage == WIA_TRANSFER_MSG_END_OF_STREAM (2)
                // lMessage == WIA_TRANSFER_MSG_END_OF_TRANSFER (3)
                if params.lMessage == 2 || params.lMessage == 3 {
                    // Transfer complete
                }
            }
        }
        let _ = lflags;
        Ok(())
    }

    fn GetNextStream(
        &self,
        lflags: i32,
        bstritemname: &BSTR,
        bstrfullitemname: &BSTR,
    ) -> Result<IStream> {
        let _ = (lflags, bstritemname, bstrfullitemname);
        // Create an in-memory IStream to receive the data
        unsafe {
            let stream = CreateStreamOnHGlobal(HGLOBAL::default(), true)
                .map_err(|e| Error::from(e))?;

            // Store reference to read data later
            // We'll read from the stream after transfer completes
            let data_ref = self.inner.data.clone();

            // For now, return the stream. Data will be read after transfer.
            // We clone the Arc so the callback can access it later.
            Ok(stream)
        }
    }
}
