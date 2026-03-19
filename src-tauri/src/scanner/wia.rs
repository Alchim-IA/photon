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
            // Try STA first (required by some WIA drivers like Avision),
            // fall back to MTA if STA fails (e.g. thread already MTA-initialized)
            let initialized = unsafe {
                let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
                if hr.is_ok() {
                    log::info!("[WIA] Backend initialisé en STA (APARTMENTTHREADED)");
                    true
                } else {
                    let hr2 = CoInitializeEx(None, COINIT_MULTITHREADED);
                    log::info!("[WIA] Backend initialisé en MTA (MULTITHREADED), STA échoué: {:?}, MTA ok={}", hr, hr2.is_ok());
                    hr2.is_ok()
                }
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

/// Query the driver for supported resolutions via GetPropertyAttributes
#[cfg(windows)]
unsafe fn query_supported_resolutions(storage: &IWiaPropertyStorage) -> Vec<u32> {
    let propspec = PROPSPEC {
        ulKind: PRSPEC_PROPID,
        Anonymous: PROPSPEC_0 { propid: WIA_IPS_XRES },
    };
    let mut flags: u32 = 0;
    let mut propvar = PROPVARIANT::new();

    if storage.GetPropertyAttributes(1, &propspec, &mut flags, &mut propvar).is_err() {
        log::warn!("[WIA] query_resolutions: GetPropertyAttributes échoué");
        return Vec::new();
    }

    let access = flags & 0xFF; // lower byte = access flags
    let kind = flags & 0xFF00; // WIA_PROP_LIST, WIA_PROP_RANGE, etc. are in bits
    // Actually WIA_PROP flags use specific bit positions:
    // WIA_PROP_NONE=8, WIA_PROP_RANGE=16, WIA_PROP_LIST=32, WIA_PROP_FLAG=64
    log::debug!("[WIA] query_resolutions: flags=0x{:08X} (access=0x{:02X})", flags, access);

    if (flags & WIA_PROP_LIST) != 0 {
        // PROPVARIANT contains a VT_VECTOR|VT_I4 or similar list
        // For WIA_PROP_LIST, the propvariant contains: [count, nominal, val1, val2, ...]
        // Try reading as a vector of i32
        let raw = propvar.as_raw();
        let vt = raw.Anonymous.Anonymous.vt;
        log::debug!("[WIA] query_resolutions: LIST, vt={}", vt);

        // VT_VECTOR | VT_I4 = 0x1000 | 0x3 = 0x1003
        if vt == 0x1003 {
            let vec_data = &raw.Anonymous.Anonymous.Anonymous.cal;
            let count = vec_data.cElems as usize;
            let ptr = vec_data.pElems;
            if !ptr.is_null() && count > 2 {
                // Skip first 2 entries (count header + nominal value), rest are valid values
                let mut resolutions = Vec::new();
                // Actually WIA LIST format: element 0 = count of valid values, element 1 = nominal
                // elements 2..N are the valid values
                let header_count = *ptr as usize;
                let start = 2; // skip count + nominal
                let end = std::cmp::min(start + header_count, count);
                for i in start..end {
                    let val = *ptr.add(i) as u32;
                    if val > 0 && val <= 4800 {
                        resolutions.push(val);
                    }
                }
                log::info!("[WIA] query_resolutions: LIST = {:?}", resolutions);
                return resolutions;
            }
        }
    } else if (flags & WIA_PROP_RANGE) != 0 {
        // PROPVARIANT contains a VT_VECTOR|VT_I4 with [min, max, step, nominal]
        let raw = propvar.as_raw();
        let vt = raw.Anonymous.Anonymous.vt;
        log::debug!("[WIA] query_resolutions: RANGE, vt={}", vt);

        if vt == 0x1003 {
            let vec_data = &raw.Anonymous.Anonymous.Anonymous.cal;
            let count = vec_data.cElems as usize;
            let ptr = vec_data.pElems;
            if !ptr.is_null() && count >= 4 {
                let min = *ptr as u32;
                let max = *ptr.add(1) as u32;
                let step = *ptr.add(2) as u32;
                log::info!("[WIA] query_resolutions: RANGE min={}, max={}, step={}", min, max, step);
                // Generate common resolutions within the range
                let mut resolutions = Vec::new();
                for &dpi in &[75u32, 100, 150, 200, 300, 400, 600, 1200] {
                    if dpi >= min && dpi <= max {
                        resolutions.push(dpi);
                    }
                }
                if !resolutions.is_empty() {
                    return resolutions;
                }
            }
        }
    }

    // Try reading current value as fallback
    if let Ok(current) = read_wia_property_i32(storage, WIA_IPS_XRES) {
        log::info!("[WIA] query_resolutions: valeur actuelle = {}", current);
    }

    Vec::new()
}

/// Query the driver for document handling capabilities (ADF, duplex, flatbed)
#[cfg(windows)]
unsafe fn query_document_handling(storage: &IWiaPropertyStorage) -> (bool, bool) {
    // WIA_DPS_DOCUMENT_HANDLING_CAPABILITIES (3086) tells us what the scanner supports
    // Bit 0x01 = FEED (ADF), Bit 0x02 = FLAT (flatbed), Bit 0x04 = DUP (duplex)
    // Bit 0x08 = DETECT_FLAT, Bit 0x10 = DETECT_SCAN, Bit 0x20 = DETECT_FEED, etc.
    if let Ok(caps) = read_wia_property_i32(storage, WIA_DPS_DOCUMENT_HANDLING_CAPABILITIES) {
        let has_adf = (caps & 0x01) != 0;
        let has_duplex = (caps & 0x04) != 0;
        log::info!("[WIA] query_doc_handling: caps=0x{:04X} adf={} duplex={}", caps, has_adf, has_duplex);
        return (has_adf, has_duplex);
    }
    // Fallback: try WIA_IPS_DOCUMENT_HANDLING_SELECT for current setting
    if let Ok(sel) = read_wia_property_i32(storage, WIA_IPS_DOCUMENT_HANDLING_SELECT) {
        log::debug!("[WIA] query_doc_handling: select=0x{:04X}", sel);
        return ((sel & 0x01) != 0, (sel & 0x04) != 0);
    }
    (false, false)
}

/// Query supported color modes from the driver
#[cfg(windows)]
unsafe fn query_color_modes(storage: &IWiaPropertyStorage) -> Vec<String> {
    let propspec = PROPSPEC {
        ulKind: PRSPEC_PROPID,
        Anonymous: PROPSPEC_0 { propid: WIA_IPA_DATATYPE },
    };
    let mut flags: u32 = 0;
    let mut propvar = PROPVARIANT::new();

    if storage.GetPropertyAttributes(1, &propspec, &mut flags, &mut propvar).is_err() {
        return Vec::new();
    }

    let mut modes = Vec::new();
    if (flags & WIA_PROP_LIST) != 0 {
        let raw = propvar.as_raw();
        if raw.Anonymous.Anonymous.vt == 0x1003 {
            let vec_data = &raw.Anonymous.Anonymous.Anonymous.cal;
            let count = vec_data.cElems as usize;
            let ptr = vec_data.pElems;
            if !ptr.is_null() && count > 2 {
                let header_count = *ptr as usize;
                let end = std::cmp::min(2 + header_count, count);
                for i in 2..end {
                    match *ptr.add(i) {
                        0 => modes.push("Noir et blanc".to_string()),   // WIA_DATA_THRESHOLD
                        1 => modes.push("Niveaux de gris".to_string()), // WIA_DATA_GRAYSCALE
                        2 => modes.push("Couleur".to_string()),         // WIA_DATA_COLOR
                        3 => modes.push("Couleur".to_string()),         // WIA_DATA_RAW_RGB (treat as color)
                        v => log::debug!("[WIA] query_color_modes: mode inconnu {}", v),
                    }
                }
            }
        }
    }
    if !modes.is_empty() {
        log::info!("[WIA] query_color_modes: {:?}", modes);
    }
    modes
}

#[cfg(windows)]
fn find_scanner_item(root: &IWiaItem2) -> Result<IWiaItem2> {
    unsafe {
        // First, try to find a child item with WiaItemTypeTransfer (0x8)
        match root.EnumChildItems(None) {
            Ok(enumerator) => {
                let mut child_index = 0u32;
                loop {
                    let mut item: Option<IWiaItem2> = None;
                    let mut fetched: u32 = 0;
                    let hr = enumerator.Next(1, &mut item, &mut fetched);
                    if hr.is_err() || fetched == 0 {
                        log::warn!("[WIA] find_scanner_item: plus d'enfants après {} items inspectés", child_index);
                        break;
                    }
                    if let Some(item) = item.take() {
                        let item_type = {
                            let storage: IWiaPropertyStorage = item.cast()?;
                            read_wia_property_i32(&storage, WIA_IPA_ITEM_FLAGS).unwrap_or(0)
                        };
                        log::debug!("[WIA] find_scanner_item: enfant #{} item_type=0x{:08X}", child_index, item_type);
                        // WiaItemTypeTransfer = 0x2000 (NOT 0x8 which is WiaItemTypeRoot)
                        if (item_type & 0x2000) != 0 {
                            log::info!("[WIA] find_scanner_item: trouvé item transférable à l'index {}", child_index);
                            return Ok(item);
                        }
                    }
                    child_index += 1;
                }
            }
            Err(e) => {
                log::warn!("[WIA] find_scanner_item: EnumChildItems échoué: {}", e);
            }
        }

        // Fallback: some scanners (older Kodak, WIA 1.0 compat) expose the root item
        // itself as the transfer-capable item — check if root has WiaItemTypeTransfer
        match root.cast::<IWiaPropertyStorage>() {
            Ok(storage) => {
                let root_type = read_wia_property_i32(&storage, WIA_IPA_ITEM_FLAGS).unwrap_or(0);
                log::info!("[WIA] find_scanner_item: vérification root item_type=0x{:08X}", root_type);
                if (root_type & 0x2000) != 0 {
                    log::info!("[WIA] find_scanner_item: root item est transférable (WIA 1.0 compat)");
                    return Ok(root.clone());
                }
            }
            Err(e) => {
                log::warn!("[WIA] find_scanner_item: cast root -> IWiaPropertyStorage échoué: {}", e);
            }
        }

        // Last resort: try to use the first child item regardless of flags
        if let Ok(enumerator) = root.EnumChildItems(None) {
            let mut item: Option<IWiaItem2> = None;
            let mut fetched: u32 = 0;
            let hr = enumerator.Next(1, &mut item, &mut fetched);
            if hr.is_ok() && fetched > 0 {
                if let Some(item) = item.take() {
                    log::info!("[WIA] find_scanner_item: utilisation du premier enfant en dernier recours");
                    return Ok(item);
                }
            }
        }

        // Absolute last resort: return root itself — some drivers work with direct transfer on root
        log::warn!("[WIA] find_scanner_item: aucun item trouvé, tentative avec root directement");
        Ok(root.clone())
    }
}

impl ScannerBackend for WiaBackend {
    fn list_devices(&self) -> std::result::Result<Vec<ScannerDevice>, ScannerError> {
        #[cfg(windows)]
        {
            log::info!("[WIA] list_devices: début de l'énumération des scanners");
            unsafe {
                // COM must be initialized on each thread
                let _com_init = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
                log::debug!("[WIA] list_devices: COM init sur thread: {:?}", _com_init);

                let dev_mgr: IWiaDevMgr2 = CoCreateInstance(&WiaDevMgr2, None, CLSCTX_ALL)
                    .map_err(|e| {
                        log::error!("[WIA] list_devices: impossible de créer WIA Device Manager: {}", e);
                        ScannerError::SystemError(format!("Impossible de créer WIA Device Manager: {}", e))
                    })?;
                log::debug!("[WIA] list_devices: WIA Device Manager créé");

                let enumerator = dev_mgr.EnumDeviceInfo(WIA_DEVINFO_ENUM_LOCAL as i32)
                    .map_err(|e| {
                        log::error!("[WIA] list_devices: erreur énumération: {}", e);
                        ScannerError::SystemError(format!("Erreur énumération: {}", e))
                    })?;
                log::debug!("[WIA] list_devices: énumérateur obtenu");

                let mut devices = Vec::new();
                let mut device_index = 0u32;

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

                        log::info!(
                            "[WIA] list_devices: périphérique #{}: name='{}', vendor='{}', id='{}', dev_type=0x{:08X}",
                            device_index, name, vendor, id, dev_type
                        );

                        // Check if it's a scanner (type & 0x000F == 1)
                        // Some scanners (older Kodak, etc.) report dev_type=0 — include them too
                        let dev_subtype = dev_type & 0x000F;
                        if dev_subtype != 1 && dev_subtype != 0 {
                            log::debug!("[WIA] list_devices: #{} ignoré (pas un scanner, type=0x{:04X})", device_index, dev_subtype);
                            device_index += 1;
                            continue;
                        }
                        if dev_subtype == 0 {
                            log::info!("[WIA] list_devices: #{} type inconnu (0x0000), inclus en tant que scanner potentiel", device_index);
                        }

                        let mut caps = ScannerCapabilities::default();

                        let bstr_dev_id = BSTR::from(&id);
                        match dev_mgr.CreateDevice(0, &bstr_dev_id)
                            .or_else(|_| dev_mgr.CreateDevice(CLSCTX_LOCAL_SERVER.0 as i32, &bstr_dev_id))
                        {
                            Ok(device) => {
                                log::debug!("[WIA] list_devices: #{} CreateDevice OK", device_index);
                                match find_scanner_item(&device) {
                                    Ok(scanner_item) => {
                                        log::debug!("[WIA] list_devices: #{} scanner item trouvé", device_index);
                                        if let Ok(item_storage) = scanner_item.cast::<IWiaPropertyStorage>() {
                                            // Query resolutions from driver
                                            let driver_resolutions = query_supported_resolutions(&item_storage);
                                            if !driver_resolutions.is_empty() {
                                                caps.resolutions = driver_resolutions;
                                            }

                                            // Query color modes from driver
                                            let driver_modes = query_color_modes(&item_storage);
                                            if !driver_modes.is_empty() {
                                                caps.color_modes = driver_modes;
                                            }

                                            // Query ADF/duplex from device root capabilities
                                            if let Ok(dev_storage) = device.cast::<IWiaPropertyStorage>() {
                                                let (adf, duplex) = query_document_handling(&dev_storage);
                                                caps.supports_adf = adf;
                                                caps.supports_duplex = duplex;
                                            }
                                            // Fallback: also check item-level properties
                                            if !caps.supports_adf {
                                                let (adf, duplex) = query_document_handling(&item_storage);
                                                if adf { caps.supports_adf = true; }
                                                if duplex { caps.supports_duplex = true; }
                                            }
                                        } else {
                                            log::warn!("[WIA] list_devices: #{} impossible de caster en IWiaPropertyStorage", device_index);
                                        }
                                    }
                                    Err(e) => {
                                        log::warn!("[WIA] list_devices: #{} find_scanner_item échoué: {}", device_index, e);
                                    }
                                }
                            }
                            Err(e) => {
                                log::warn!("[WIA] list_devices: #{} CreateDevice échoué: {}", device_index, e);
                            }
                        }

                        // Ensure default resolutions if none were detected
                        if caps.resolutions.is_empty() {
                            caps.resolutions = vec![150, 300, 600];
                            log::info!("[WIA] list_devices: #{} résolutions par défaut appliquées", device_index);
                        }

                        log::info!(
                            "[WIA] list_devices: scanner ajouté: '{}' ({}) résolutions={:?} adf={} duplex={}",
                            name, vendor, caps.resolutions, caps.supports_adf, caps.supports_duplex
                        );
                        devices.push(ScannerDevice {
                            id,
                            name,
                            vendor,
                            capabilities: caps,
                        });
                    }
                    device_index += 1;
                }

                log::info!("[WIA] list_devices: {} scanner(s) trouvé(s)", devices.len());
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
            log::info!(
                "[WIA] scan: début — device_id='{}', dpi={}, color_mode='{}', duplex={}, paper='{}'",
                options.device_id, options.dpi, options.color_mode, options.duplex, options.paper_format
            );

            // Strategy 1: WIA 2.0 (IWiaTransfer::Download) with progressive property fallback
            let strategies: &[&str] = &["full", "minimal", "defaults"];
            let mut wia2_error = None;

            for strategy in strategies {
                log::info!("[WIA] scan: === WIA 2.0 stratégie '{}' ===", strategy);
                match self.try_scan_wia2(&options, strategy) {
                    Ok(result) => return Ok(result),
                    Err(e) => {
                        log::warn!("[WIA] scan: WIA 2.0 '{}' échoué: {}", strategy, e);
                        wia2_error = Some(e);
                    }
                }
            }

            // Strategy 2: WIA 1.0 fallback (IWiaDataTransfer)
            // Some older scanners (Kodak i2400, etc.) only work with WIA 1.0 transfer
            log::info!("[WIA] scan: === Fallback WIA 1.0 ===");
            match self.try_scan_wia1(&options) {
                Ok(result) => return Ok(result),
                Err(e) => {
                    log::error!("[WIA] scan: WIA 1.0 fallback échoué: {}", e);
                    // Return the most informative error
                    return Err(ScannerError::SystemError(format!(
                        "Échec numérisation. WIA 2.0: {}. WIA 1.0: {}. Vérifiez que le scanner est allumé et qu'il y a du papier dans le chargeur.",
                        wia2_error.as_ref().map(|e| e.to_string()).unwrap_or_default(),
                        e
                    )));
                }
            }
        }
        #[cfg(not(windows))]
        {
            let _ = options;
            Err(ScannerError::SystemError("WIA n'est disponible que sur Windows".to_string()))
        }
    }
}

#[cfg(windows)]
impl WiaBackend {
    /// WIA 2.0 scan using IWiaTransfer::Download
    fn try_scan_wia2(
        &self,
        options: &ScanOptions,
        strategy: &str,
    ) -> std::result::Result<ScanResult, ScannerError> {
        unsafe {
            let _com_init = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

            let dev_mgr: IWiaDevMgr2 = CoCreateInstance(&WiaDevMgr2, None, CLSCTX_ALL)
                .map_err(|e| ScannerError::SystemError(format!("WIA 2.0 init: {}", e)))?;

            let bstr_id = BSTR::from(&options.device_id);
            let device = dev_mgr.CreateDevice(0, &bstr_id)
                .or_else(|_| dev_mgr.CreateDevice(CLSCTX_LOCAL_SERVER.0 as i32, &bstr_id))
                .map_err(|e| ScannerError::SystemError(format!("CreateDevice: {}", e)))?;
            log::info!("[WIA] scan [wia2/{}]: connexion OK", strategy);

            let scanner_item = find_scanner_item(&device)
                .map_err(|e| ScannerError::SystemError(format!("find_scanner_item: 0x{:08X}", e.code().0 as u32)))?;

            // ALWAYS select feeder source — document scanners (Kodak i2400, etc.)
            // have no flatbed and MUST have the feeder explicitly selected
            let feeder_val = if options.duplex { 0x05i32 } else { 0x01i32 }; // FEEDER or FEEDER+DUPLEX
            match write_wia_property_i32(&scanner_item, WIA_IPS_DOCUMENT_HANDLING_SELECT, feeder_val) {
                Ok(()) => log::info!("[WIA] [wia2/{}]: DOCUMENT_HANDLING_SELECT = 0x{:02X} OK", strategy, feeder_val),
                Err(e) => log::warn!("[WIA] [wia2/{}]: DOCUMENT_HANDLING_SELECT ÉCHEC: {} (flatbed?)", strategy, e),
            }

            // Configure properties based on strategy
            if strategy != "defaults" {
                for (name, prop_id, value) in &[
                    ("WIA_IPS_XRES", WIA_IPS_XRES, options.dpi as i32),
                    ("WIA_IPS_YRES", WIA_IPS_YRES, options.dpi as i32),
                ] {
                    match write_wia_property_i32(&scanner_item, *prop_id, *value) {
                        Ok(()) => log::debug!("[WIA] [wia2/{}]: {} = {} OK", strategy, name, value),
                        Err(e) => log::warn!("[WIA] [wia2/{}]: {} ÉCHEC: {}", strategy, name, e),
                    }
                }
                let data_type = match color_mode_id(&options.color_mode) {
                    1 => 2i32, 2 => 1i32, 4 => 0i32, _ => 2i32,
                };
                let _ = write_wia_property_i32(&scanner_item, WIA_IPA_DATATYPE, data_type);
            }

            if strategy == "full" {
                let (paper_w, paper_h) = paper_dimensions(&options.paper_format);
                let width_px = mm_to_pixels(paper_w, options.dpi) as i32;
                let height_px = mm_to_pixels(paper_h, options.dpi) as i32;
                for (name, prop_id, value) in &[
                    ("WIA_IPS_XPOS", WIA_IPS_XPOS, 0),
                    ("WIA_IPS_YPOS", WIA_IPS_YPOS, 0),
                    ("WIA_IPS_XEXTENT", WIA_IPS_XEXTENT, width_px),
                    ("WIA_IPS_YEXTENT", WIA_IPS_YEXTENT, height_px),
                ] {
                    match write_wia_property_i32(&scanner_item, *prop_id, *value) {
                        Ok(()) => log::debug!("[WIA] [wia2/full]: {} = {} OK", name, value),
                        Err(e) => log::warn!("[WIA] [wia2/full]: {} ÉCHEC: {}", name, e),
                    }
                }
            }

            let transfer: IWiaTransfer = scanner_item.cast()
                .map_err(|e| ScannerError::SystemError(format!("IWiaTransfer cast: {}", e)))?;

            let callback = WiaTransferCallback::new();
            let callback_interface: IWiaTransferCallback = callback.clone().into();

            log::info!("[WIA] [wia2/{}]: Download...", strategy);
            transfer.Download(0, &callback_interface)
                .map_err(|e| {
                    log::error!("[WIA] [wia2/{}]: Download ÉCHEC: 0x{:08X}", strategy, e.code().0 as u32);
                    ScannerError::SystemError(format!("Download ({}): {}", strategy, e))
                })?;

            let data = callback.get_data();
            if data.is_empty() {
                return Err(ScannerError::SystemError("Stream vide".to_string()));
            }

            decode_scan_data(&data)
        }
    }

    /// WIA 1.0 fallback using IWiaDevMgr + temporary file transfer
    /// Required for older scanners (Kodak i2400, etc.) that don't support WIA 2.0 transfers
    fn try_scan_wia1(
        &self,
        options: &ScanOptions,
    ) -> std::result::Result<ScanResult, ScannerError> {
        unsafe {
            let _com_init = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            log::info!("[WIA] scan [wia1]: initialisation WIA 1.0...");

            // Use WIA 1.0 Device Manager
            let dev_mgr: IWiaDevMgr = CoCreateInstance(&WiaDevMgr, None, CLSCTX_ALL)
                .map_err(|e| {
                    log::error!("[WIA] scan [wia1]: WIA 1.0 DevMgr échoué: {}", e);
                    ScannerError::SystemError(format!("WIA 1.0 init: {}", e))
                })?;
            log::info!("[WIA] scan [wia1]: WIA 1.0 Device Manager créé");

            let bstr_id = BSTR::from(&options.device_id);
            let device: IWiaItem = dev_mgr.CreateDevice(&bstr_id)
                .map_err(|e| {
                    log::error!("[WIA] scan [wia1]: CreateDevice échoué: 0x{:08X}", e.code().0 as u32);
                    ScannerError::SystemError(format!("WIA 1.0 CreateDevice: {}", e))
                })?;
            log::info!("[WIA] scan [wia1]: connexion OK");

            // Find child scanner item (WIA 1.0)
            let scanner_item = find_scanner_item_v1(&device)?;
            log::info!("[WIA] scan [wia1]: scanner item trouvé");

            // Set properties on the WIA 1.0 item
            if let Ok(storage) = scanner_item.cast::<IWiaPropertyStorage>() {
                // Resolution
                for (name, prop_id, value) in &[
                    ("WIA_IPS_XRES", WIA_IPS_XRES, options.dpi as i32),
                    ("WIA_IPS_YRES", WIA_IPS_YRES, options.dpi as i32),
                ] {
                    let propspec = PROPSPEC {
                        ulKind: PRSPEC_PROPID,
                        Anonymous: PROPSPEC_0 { propid: *prop_id },
                    };
                    let propvar = PROPVARIANT::from(*value);
                    match storage.WriteMultiple(1, &propspec, &propvar, 2) {
                        Ok(()) => log::debug!("[WIA] [wia1]: {} = {} OK", name, value),
                        Err(e) => log::warn!("[WIA] [wia1]: {} ÉCHEC: {}", name, e),
                    }
                }

                // Color mode
                let data_type = match color_mode_id(&options.color_mode) {
                    1 => 2i32, 2 => 1i32, 4 => 0i32, _ => 2i32,
                };
                let propspec = PROPSPEC {
                    ulKind: PRSPEC_PROPID,
                    Anonymous: PROPSPEC_0 { propid: WIA_IPA_DATATYPE },
                };
                let propvar = PROPVARIANT::from(data_type);
                let _ = storage.WriteMultiple(1, &propspec, &propvar, 2);

                // Set TYMED = TYMED_FILE (2) for file-based WIA 1.0 transfer
                let propspec_tymed = PROPSPEC {
                    ulKind: PRSPEC_PROPID,
                    Anonymous: PROPSPEC_0 { propid: 4108 }, // WIA_IPA_TYMED
                };
                let propvar_tymed = PROPVARIANT::from(2i32); // TYMED_FILE
                match storage.WriteMultiple(1, &propspec_tymed, &propvar_tymed, 2) {
                    Ok(()) => log::debug!("[WIA] [wia1]: TYMED = FILE OK"),
                    Err(e) => log::warn!("[WIA] [wia1]: TYMED ÉCHEC: {}", e),
                }

                // Set WIA_IPA_FORMAT to BMP GUID via raw PROPVARIANT
                set_format_bmp_v1(&storage);
            }

            // Try WIA 2.0 transfer on WIA 1.0 item (some compat drivers support this)
            // Cast WIA 1.0 IWiaItem to IUnknown, then try IWiaItem2
            let item_unknown: windows::core::IUnknown = scanner_item.cast()
                .map_err(|e| ScannerError::SystemError(format!("cast IUnknown: {}", e)))?;

            // Try casting to IWiaItem2 for WIA 2.0 transfer on WIA 1.0 device
            if let Ok(item2) = item_unknown.cast::<IWiaItem2>() {
                log::info!("[WIA] scan [wia1]: item supporte IWiaItem2, essai IWiaTransfer...");
                if let Ok(transfer) = item2.cast::<IWiaTransfer>() {
                    let callback = WiaTransferCallback::new();
                    let callback_interface: IWiaTransferCallback = callback.clone().into();
                    match transfer.Download(0, &callback_interface) {
                        Ok(()) => {
                            let data = callback.get_data();
                            if !data.is_empty() {
                                log::info!("[WIA] scan [wia1/IWiaTransfer]: succès, {} octets", data.len());
                                return decode_scan_data(&data);
                            }
                        }
                        Err(e) => log::warn!("[WIA] scan [wia1/IWiaTransfer]: Download échoué: 0x{:08X}", e.code().0 as u32),
                    }
                }
            }

            // Last resort: try IWiaDataTransfer with idtGetBandedData
            let transfer: IWiaDataTransfer = scanner_item.cast()
                .map_err(|e| {
                    log::error!("[WIA] scan [wia1]: cast IWiaDataTransfer échoué: {}", e);
                    ScannerError::SystemError(format!("IWiaDataTransfer: {}", e))
                })?;
            log::info!("[WIA] scan [wia1]: interface IWiaDataTransfer obtenue");

            // Use idtGetBandedData for band-based transfer
            let mut trans_info = WIA_DATA_TRANSFER_INFO {
                ulSize: std::mem::size_of::<WIA_DATA_TRANSFER_INFO>() as u32,
                ulSection: 0,
                ulBufferSize: 0, // Let WIA choose buffer size
                bDoubleBuffer: FALSE,
                ulReserved1: 0,
                ulReserved2: 0,
                ulReserved3: 0,
            };

            log::info!("[WIA] scan [wia1]: lancement idtGetBandedData...");
            let none_callback: Option<&IWiaDataCallback> = None;
            transfer.idtGetBandedData(&mut trans_info, none_callback)
                .map_err(|e| {
                    log::error!("[WIA] scan [wia1]: idtGetBandedData échoué: 0x{:08X} {}", e.code().0 as u32, e);
                    ScannerError::SystemError(format!("idtGetBandedData: {}", e))
                })?;

            Err(ScannerError::SystemError("WIA 1.0: transfert sans callback non supporté".to_string()))
        }
    }
}

/// Set WIA_IPA_FORMAT = BMP on WIA 1.0 property storage
#[cfg(windows)]
fn set_format_bmp_v1(storage: &IWiaPropertyStorage) {
    unsafe {
        use windows_core::imp;
        let bmp_guid = imp::GUID {
            data1: 0xB96B3CAB,
            data2: 0x0728,
            data3: 0x11D3,
            data4: [0x9D, 0x7B, 0x00, 0x00, 0xF8, 0x1E, 0xF3, 0x2E],
        };
        let guid_ptr = Box::into_raw(Box::new(bmp_guid));
        let raw = imp::PROPVARIANT {
            Anonymous: imp::PROPVARIANT_0 {
                Anonymous: imp::PROPVARIANT_0_0 {
                    vt: 72, // VT_CLSID
                    wReserved1: 0,
                    wReserved2: 0,
                    wReserved3: 0,
                    Anonymous: imp::PROPVARIANT_0_0_0 { puuid: guid_ptr },
                },
            },
        };
        let propvar = PROPVARIANT::from_raw(raw);
        let propspec = PROPSPEC {
            ulKind: PRSPEC_PROPID,
            Anonymous: PROPSPEC_0 { propid: 4106 }, // WIA_IPA_FORMAT
        };
        match storage.WriteMultiple(1, &propspec, &propvar, 2) {
            Ok(()) => log::debug!("[WIA] [wia1]: WIA_IPA_FORMAT = BMP OK"),
            Err(e) => log::warn!("[WIA] [wia1]: WIA_IPA_FORMAT ÉCHEC: {}", e),
        }
        std::mem::forget(propvar);
        let _ = Box::from_raw(guid_ptr);
    }
}

/// Find scanner item using WIA 1.0 IWiaItem interface
#[cfg(windows)]
fn find_scanner_item_v1(root: &IWiaItem) -> std::result::Result<IWiaItem, ScannerError> {
    unsafe {
        // Try to find a child item
        match root.EnumChildItems() {
            Ok(enumerator) => {
                loop {
                    let mut item: Option<IWiaItem> = None;
                    let mut fetched: u32 = 0;
                    let hr = enumerator.Next(1, &mut item, &mut fetched);
                    if hr.is_err() || fetched == 0 {
                        break;
                    }
                    if let Some(item) = item.take() {
                        match item.GetItemType() {
                            Ok(item_type) => {
                                log::debug!("[WIA] find_scanner_item_v1: enfant item_type=0x{:08X}", item_type);
                                // WiaItemTypeTransfer = 0x8
                                // WiaItemTypeTransfer = 0x2000
                                if (item_type & 0x2000) != 0 {
                                    log::info!("[WIA] find_scanner_item_v1: trouvé item transférable");
                                    return Ok(item);
                                }
                            }
                            Err(e) => log::warn!("[WIA] find_scanner_item_v1: GetItemType échoué: {}", e),
                        }
                    }
                }
            }
            Err(e) => {
                log::warn!("[WIA] find_scanner_item_v1: EnumChildItems échoué: {}", e);
            }
        }

        // Fallback: use root itself
        log::info!("[WIA] find_scanner_item_v1: utilisation du root item");
        // We need to clone the root - IWiaItem implements Clone via COM reference counting
        let root_unknown: windows::core::IUnknown = root.cast()
            .map_err(|e| ScannerError::SystemError(format!("cast root: {}", e)))?;
        let root_item: IWiaItem = root_unknown.cast()
            .map_err(|e| ScannerError::SystemError(format!("cast back: {}", e)))?;
        Ok(root_item)
    }
}

/// Decode raw image data (BMP, TIFF, etc.) to PNG ScanResult
#[cfg(windows)]
fn decode_scan_data(data: &[u8]) -> std::result::Result<ScanResult, ScannerError> {
    if data.len() >= 8 {
        log::debug!(
            "[WIA] decode: premiers octets: {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X}",
            data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7]
        );
    }

    let img = ::image::load_from_memory(data)
        .map_err(|e| {
            log::error!("[WIA] decode: échoué: {} ({} octets)", e, data.len());
            ScannerError::SystemError(format!("Décodage image: {}", e))
        })?;

    let width = img.width();
    let height = img.height();
    log::info!("[WIA] decode: image {}x{} px", width, height);

    let mut png_bytes = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut png_bytes);
    img.write_to(&mut cursor, ::image::ImageFormat::Png)
        .map_err(|e| ScannerError::SystemError(format!("Encodage PNG: {}", e)))?;
    log::info!("[WIA] decode: PNG {} octets — succès!", png_bytes.len());

    Ok(ScanResult {
        image_data: png_bytes,
        width,
        height,
    })
}

// ─── WIA Transfer Callback (Windows COM) ─────────────────────────

#[cfg(windows)]
use std::sync::{Arc, Mutex};
#[cfg(windows)]
use std::sync::atomic::{AtomicU32, Ordering};

#[cfg(windows)]
#[derive(Clone)]
struct WiaTransferCallbackData {
    stream: Arc<Mutex<Option<IStream>>>,
    callback_count: Arc<AtomicU32>,
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
                stream: Arc::new(Mutex::new(None)),
                callback_count: Arc::new(AtomicU32::new(0)),
            },
        }
    }

    fn get_callback_count(&self) -> u32 {
        self.inner.callback_count.load(Ordering::Relaxed)
    }

    fn has_stream(&self) -> bool {
        self.inner.stream.lock().unwrap().is_some()
    }

    fn get_data(&self) -> Vec<u8> {
        let guard = self.inner.stream.lock().unwrap();
        let stream = match guard.as_ref() {
            Some(s) => s,
            None => {
                log::error!("[WIA] get_data: aucun stream stocké — GetNextStream n'a jamais été appelé");
                return Vec::new();
            }
        };
        unsafe {
            // Seek to beginning of stream
            log::debug!("[WIA] get_data: seek au début du stream...");
            if let Err(e) = stream.Seek(0, STREAM_SEEK_SET, None) {
                log::error!("[WIA] get_data: Seek échoué: {}", e);
                return Vec::new();
            }

            // Get stream size via Stat
            let mut statstg = std::mem::zeroed::<STATSTG>();
            if let Err(e) = stream.Stat(&mut statstg, STATFLAG_NONAME) {
                log::error!("[WIA] get_data: Stat échoué: {}", e);
                return Vec::new();
            }
            let size = statstg.cbSize as usize;
            log::info!("[WIA] get_data: taille du stream = {} octets", size);

            if size == 0 {
                log::warn!("[WIA] get_data: stream vide (0 octets)");
                return Vec::new();
            }

            // Read data from stream
            let mut buffer = vec![0u8; size];
            let mut bytes_read: u32 = 0;
            let hr = stream.Read(
                buffer.as_mut_ptr() as *mut _,
                size as u32,
                Some(&mut bytes_read),
            );
            if hr.is_err() {
                log::error!("[WIA] get_data: Read échoué: HRESULT=0x{:08X}", hr.0 as u32);
                return Vec::new();
            }
            log::info!("[WIA] get_data: lu {} octets sur {} demandés", bytes_read, size);
            buffer.truncate(bytes_read as usize);
            buffer
        }
    }
}

#[cfg(windows)]
impl IWiaTransferCallback_Impl for WiaTransferCallback_Impl {
    fn TransferCallback(
        &self,
        lflags: i32,
        pwiatransferparams: *const WiaTransferParams,
    ) -> Result<()> {
        let count = self.inner.callback_count.fetch_add(1, Ordering::Relaxed) + 1;
        if !pwiatransferparams.is_null() {
            let params = unsafe { &*pwiatransferparams };
            log::debug!(
                "[WIA] TransferCallback #{}: lflags=0x{:08X}, lMessage={}, lPercentComplete={}%, ulTransferredBytes={}, hrErrorStatus=0x{:08X}",
                count, lflags, params.lMessage, params.lPercentComplete, params.ulTransferredBytes, params.hrErrorStatus.0 as u32
            );
        } else {
            log::debug!("[WIA] TransferCallback #{}: lflags=0x{:08X}, params=null", count, lflags);
        }
        Ok(())
    }

    fn GetNextStream(
        &self,
        lflags: i32,
        bstritemname: &BSTR,
        bstrfullitemname: &BSTR,
    ) -> Result<IStream> {
        log::info!(
            "[WIA] GetNextStream: lflags=0x{:08X}, item='{}', fullname='{}'",
            lflags, bstritemname, bstrfullitemname
        );
        unsafe {
            let stream = CreateStreamOnHGlobal(HGLOBAL::default(), true)?;
            log::debug!("[WIA] GetNextStream: IStream créé et stocké");
            // Store the stream so we can read data from it after transfer
            *self.inner.stream.lock().unwrap() = Some(stream.clone());
            Ok(stream)
        }
    }
}
