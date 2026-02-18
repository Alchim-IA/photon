use crate::scanner::*;

#[cfg(windows)]
use windows::{
    core::*,
    Win32::Devices::ImageAcquisition::*,
    Win32::System::Com::*,
    Win32::System::Com::StructuredStorage::*,
    Win32::Foundation::*,
};
#[cfg(windows)]
use std::ptr;

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
    // Try BSTR extraction first
    if let Ok(bstr) = BSTR::try_from(pv) {
        return Some(bstr.to_string());
    }
    None
}

#[cfg(windows)]
unsafe fn propvariant_to_i32(pv: &PROPVARIANT) -> Option<i32> {
    i32::try_from(pv).ok()
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
    storage.ReadMultiple(1, &propspec, &mut propvar)?;
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
    storage.ReadMultiple(1, &propspec, &mut propvar)?;
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
    let propvar = PROPVARIANT::from(value);
    storage.WriteMultiple(1, &propspec, &propvar, 2)?;
    Ok(())
}

#[cfg(windows)]
fn find_scanner_item(root: &IWiaItem2) -> Result<IWiaItem2> {
    unsafe {
        let enumerator = root.EnumChildItems(None)?;
        loop {
            let mut item: Option<IWiaItem2> = None;
            let mut fetched: u32 = 0;
            let hr = enumerator.Next(1, &mut item, &mut fetched);
            if hr.is_err() || fetched == 0 {
                break;
            }
            if let Some(item) = item.take() {
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

                let enumerator = dev_mgr.EnumDeviceInfo(WIA_DEVINFO_ENUM_LOCAL as i32)
                    .map_err(|e| ScannerError::SystemError(format!("Erreur énumération: {}", e)))?;

                let mut devices = Vec::new();

                loop {
                    let mut storage_opt: Option<IWiaPropertyStorage> = None;
                    let mut fetched: u32 = 0;
                    let hr = enumerator.Next(1, &mut storage_opt, &mut fetched);
                    if hr.is_err() || fetched == 0 {
                        break;
                    }

                    if let Some(storage) = storage_opt.take() {
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

                        let mut caps = ScannerCapabilities::default();

                        if let Ok(device) = dev_mgr.CreateDevice(0, &BSTR::from(&id)) {
                            if let Ok(scanner_item) = find_scanner_item(&device) {
                                let item_storage: std::result::Result<IWiaPropertyStorage, _> = scanner_item.cast();
                                if let Ok(item_storage) = item_storage {
                                    if let Ok(xres) = read_wia_property_i32(&item_storage, WIA_IPS_XRES) {
                                        caps.resolutions = vec![75, 150, 300, 600];
                                        if xres >= 1200 {
                                            caps.resolutions.push(1200);
                                        }
                                    }

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

                let (paper_w, paper_h) = paper_dimensions(&options.paper_format);
                let width_px = mm_to_pixels(paper_w, options.dpi) as i32;
                let height_px = mm_to_pixels(paper_h, options.dpi) as i32;

                let _ = write_wia_property_i32(&scanner_item, WIA_IPS_XRES, options.dpi as i32);
                let _ = write_wia_property_i32(&scanner_item, WIA_IPS_YRES, options.dpi as i32);
                let _ = write_wia_property_i32(&scanner_item, WIA_IPS_XPOS, 0);
                let _ = write_wia_property_i32(&scanner_item, WIA_IPS_YPOS, 0);
                let _ = write_wia_property_i32(&scanner_item, WIA_IPS_XEXTENT, width_px);
                let _ = write_wia_property_i32(&scanner_item, WIA_IPS_YEXTENT, height_px);

                let data_type = match color_mode_id(&options.color_mode) {
                    1 => 2i32,  // WIA_DATA_COLOR
                    2 => 1i32,  // WIA_DATA_GRAYSCALE
                    4 => 0i32,  // WIA_DATA_THRESHOLD
                    _ => 2i32,
                };
                let _ = write_wia_property_i32(&scanner_item, WIA_IPA_DATATYPE, data_type);

                if options.duplex {
                    let _ = write_wia_property_i32(&scanner_item, WIA_IPS_DOCUMENT_HANDLING_SELECT, 0x05);
                }

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
        let _ = (lflags, pwiatransferparams);
        Ok(())
    }

    fn GetNextStream(
        &self,
        lflags: i32,
        bstritemname: &BSTR,
        bstrfullitemname: &BSTR,
    ) -> Result<IStream> {
        let _ = (lflags, bstritemname, bstrfullitemname);
        unsafe {
            let stream = CreateStreamOnHGlobal(HGLOBAL::default(), true)?;
            Ok(stream)
        }
    }
}
