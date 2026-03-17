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
                        // WiaItemTypeTransfer (0x8) means the item can transfer data
                        if (item_type & 0x8) != 0 {
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
                if (root_type & 0x8) != 0 {
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
                                        let item_storage: std::result::Result<IWiaPropertyStorage, _> = scanner_item.cast();
                                        if let Ok(item_storage) = item_storage {
                                            if let Ok(xres) = read_wia_property_i32(&item_storage, WIA_IPS_XRES) {
                                                log::debug!("[WIA] list_devices: #{} résolution X actuelle: {}", device_index, xres);
                                                caps.resolutions = vec![75, 150, 300, 600];
                                                if xres >= 1200 {
                                                    caps.resolutions.push(1200);
                                                }
                                            } else {
                                                log::warn!("[WIA] list_devices: #{} impossible de lire WIA_IPS_XRES", device_index);
                                            }

                                            if let Ok(doc_handling) = read_wia_property_i32(&item_storage, WIA_IPS_DOCUMENT_HANDLING_SELECT) {
                                                caps.supports_adf = (doc_handling & 0x01) != 0;
                                                caps.supports_duplex = (doc_handling & 0x04) != 0;
                                                log::debug!("[WIA] list_devices: #{} doc_handling=0x{:04X} adf={} duplex={}", device_index, doc_handling, caps.supports_adf, caps.supports_duplex);
                                            } else {
                                                log::debug!("[WIA] list_devices: #{} pas de WIA_IPS_DOCUMENT_HANDLING_SELECT", device_index);
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
            unsafe {
                // COM must be initialized on each thread — this scan runs in spawn_blocking
                let _com_init = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
                log::debug!("[WIA] scan: COM init sur thread de scan: {:?}", _com_init);

                let dev_mgr: IWiaDevMgr2 = CoCreateInstance(&WiaDevMgr2, None, CLSCTX_ALL)
                    .map_err(|e| {
                        log::error!("[WIA] scan: WIA init échoué: {}", e);
                        ScannerError::SystemError(format!("WIA init: {}", e))
                    })?;
                log::debug!("[WIA] scan: WIA Device Manager créé");

                let bstr_id = BSTR::from(&options.device_id);
                let device = dev_mgr.CreateDevice(0, &bstr_id)
                    .or_else(|e| {
                        log::warn!("[WIA] scan: CreateDevice(0) échoué: {} — retry avec CLSCTX_LOCAL_SERVER flags", e);
                        dev_mgr.CreateDevice(CLSCTX_LOCAL_SERVER.0 as i32, &bstr_id)
                    })
                    .map_err(|e| {
                        log::error!("[WIA] scan: CreateDevice échoué pour '{}': 0x{:08X} {}", options.device_id, e.code().0 as u32, e);
                        ScannerError::SystemError(format!("Connexion au scanner: {}", e))
                    })?;
                log::info!("[WIA] scan: connexion au scanner OK");

                let scanner_item = find_scanner_item(&device)
                    .map_err(|e| {
                        log::error!("[WIA] scan: find_scanner_item échoué: 0x{:08X} {}", e.code().0 as u32, e);
                        ScannerError::SystemError(format!("Aucun élément scanner trouvé (0x{:08X})", e.code().0 as u32))
                    })?;
                log::debug!("[WIA] scan: scanner item trouvé");

                let (paper_w, paper_h) = paper_dimensions(&options.paper_format);
                let width_px = mm_to_pixels(paper_w, options.dpi) as i32;
                let height_px = mm_to_pixels(paper_h, options.dpi) as i32;
                log::info!(
                    "[WIA] scan: format papier '{}' => {}x{} mm => {}x{} px @ {} dpi",
                    options.paper_format, paper_w, paper_h, width_px, height_px, options.dpi
                );

                // Set scan properties
                let props = [
                    ("WIA_IPS_XRES", WIA_IPS_XRES, options.dpi as i32),
                    ("WIA_IPS_YRES", WIA_IPS_YRES, options.dpi as i32),
                    ("WIA_IPS_XPOS", WIA_IPS_XPOS, 0),
                    ("WIA_IPS_YPOS", WIA_IPS_YPOS, 0),
                    ("WIA_IPS_XEXTENT", WIA_IPS_XEXTENT, width_px),
                    ("WIA_IPS_YEXTENT", WIA_IPS_YEXTENT, height_px),
                ];
                for (name, prop_id, value) in &props {
                    match write_wia_property_i32(&scanner_item, *prop_id, *value) {
                        Ok(()) => log::debug!("[WIA] scan: {} = {} OK", name, value),
                        Err(e) => log::warn!("[WIA] scan: {} = {} ÉCHEC: {}", name, value, e),
                    }
                }

                let data_type = match color_mode_id(&options.color_mode) {
                    1 => 2i32,  // WIA_DATA_COLOR
                    2 => 1i32,  // WIA_DATA_GRAYSCALE
                    4 => 0i32,  // WIA_DATA_THRESHOLD
                    _ => 2i32,
                };
                match write_wia_property_i32(&scanner_item, WIA_IPA_DATATYPE, data_type) {
                    Ok(()) => log::debug!("[WIA] scan: WIA_IPA_DATATYPE = {} (mode='{}') OK", data_type, options.color_mode),
                    Err(e) => log::warn!("[WIA] scan: WIA_IPA_DATATYPE = {} ÉCHEC: {}", data_type, e),
                }

                if options.duplex {
                    match write_wia_property_i32(&scanner_item, WIA_IPS_DOCUMENT_HANDLING_SELECT, 0x05) {
                        Ok(()) => log::debug!("[WIA] scan: duplex activé OK"),
                        Err(e) => log::warn!("[WIA] scan: activation duplex ÉCHEC: {}", e),
                    }
                }

                let transfer: IWiaTransfer = scanner_item.cast()
                    .map_err(|e| {
                        log::error!("[WIA] scan: cast IWiaTransfer échoué: {}", e);
                        ScannerError::SystemError(format!("Interface de transfert: {}", e))
                    })?;
                log::debug!("[WIA] scan: interface IWiaTransfer obtenue");

                let callback = WiaTransferCallback::new();
                let callback_interface: IWiaTransferCallback = callback.clone().into();

                log::info!("[WIA] scan: lancement du Download...");
                transfer.Download(0, &callback_interface)
                    .map_err(|e| {
                        log::error!("[WIA] scan: Download échoué: {} (HRESULT=0x{:08X})", e, e.code().0 as u32);
                        ScannerError::SystemError(format!("Erreur de numérisation: {}", e))
                    })?;
                log::info!("[WIA] scan: Download terminé OK");

                log::debug!("[WIA] scan: callback_count={}, stream_present={}", callback.get_callback_count(), callback.has_stream());
                let data = callback.get_data();
                log::info!("[WIA] scan: données récupérées du stream, taille={} octets", data.len());

                if data.is_empty() {
                    log::error!("[WIA] scan: AUCUNE DONNÉE — le stream est vide après le transfert");
                    return Err(ScannerError::SystemError("Aucune donnée reçue du scanner".to_string()));
                }

                // Log first bytes to identify format
                if data.len() >= 8 {
                    log::debug!(
                        "[WIA] scan: premiers octets: {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X}",
                        data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7]
                    );
                }

                log::debug!("[WIA] scan: décodage de l'image ({} octets)...", data.len());
                let img = ::image::load_from_memory(&data)
                    .map_err(|e| {
                        log::error!("[WIA] scan: décodage image échoué: {} (taille données={} octets)", e, data.len());
                        ScannerError::SystemError(format!("Décodage image: {}", e))
                    })?;

                let width = img.width();
                let height = img.height();
                log::info!("[WIA] scan: image décodée {}x{} pixels", width, height);

                let mut png_bytes = Vec::new();
                let mut cursor = std::io::Cursor::new(&mut png_bytes);
                img.write_to(&mut cursor, ::image::ImageFormat::Png)
                    .map_err(|e| {
                        log::error!("[WIA] scan: encodage PNG échoué: {}", e);
                        ScannerError::SystemError(format!("Encodage PNG: {}", e))
                    })?;
                log::info!("[WIA] scan: encodage PNG OK, taille={} octets", png_bytes.len());

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
