use crate::scanner::*;
use std::process::{Command, Stdio};
use std::time::Duration;

/// An eSCL scanner discovered via Bonjour.
#[derive(Debug, Clone)]
struct EsclScanner {
    name: String,
    host: String, // e.g. "Lexmark-Noir-Blanc.local."
    port: u16,
    root: String, // e.g. "/eSCL"
    caps: Option<EsclCapabilities>,
}

#[derive(Debug, Clone)]
struct EsclCapabilities {
    resolutions: Vec<u32>,
    color_modes: Vec<String>,
    supports_duplex: bool,
    supports_adf: bool,
    max_width: u32,  // in 300ths of inch
    max_height: u32, // in 300ths of inch
}

pub struct EsclBackend;

impl EsclBackend {
    pub fn new() -> Self {
        Self
    }
}

// ─── Bonjour Discovery ──────────────────────────────────────────

/// Discover eSCL scanners using dns-sd (always available on macOS).
fn discover_escl() -> Vec<EsclScanner> {
    let mut scanners = Vec::new();

    // Step 1: Browse for _uscan._tcp services (3 second window)
    let instances = browse_uscan_services();
    eprintln!("[eSCL] Found {} _uscan._tcp instances", instances.len());

    // Step 2: Resolve each instance to hostname:port + TXT records
    for instance in &instances {
        eprintln!("[eSCL] Resolving: {}", instance);
        if let Some(scanner) = resolve_uscan_service(instance) {
            eprintln!(
                "[eSCL] Resolved: {} -> {}:{}{}",
                scanner.name, scanner.host, scanner.port, scanner.root
            );
            scanners.push(scanner);
        }
    }

    // Step 3: Fetch capabilities for each scanner
    for scanner in &mut scanners {
        scanner.caps = fetch_capabilities(&scanner.host, scanner.port, &scanner.root);
    }

    scanners
}

/// Browse _uscan._tcp services via dns-sd subprocess.
fn browse_uscan_services() -> Vec<String> {
    let mut child = match Command::new("dns-sd")
        .args(["-B", "_uscan._tcp", "local."])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[eSCL] dns-sd browse failed: {}", e);
            return vec![];
        }
    };

    std::thread::sleep(Duration::from_secs(3));
    let _ = child.kill();

    let output = match child.wait_with_output() {
        Ok(o) => o,
        Err(_) => return vec![],
    };

    let text = String::from_utf8_lossy(&output.stdout);
    let mut instances = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // Output format:
    // Browsing for _uscan._tcp.local.
    // DATE: ---Mon 19 Feb 2026---
    //  5:32:12.123  Add        3   4 local.               _uscan._tcp.         Lexmark Noir Blanc
    for line in text.lines() {
        // Lines with "Add" contain discovered services
        if !line.contains("Add") {
            continue;
        }
        // Split by whitespace: timestamp, Add, flags, if, domain, type, name...
        // The instance name is everything after the service type field
        if let Some(after_type) = line.split("_uscan._tcp.").nth(1) {
            let name = after_type.trim().to_string();
            if !name.is_empty() && seen.insert(name.clone()) {
                instances.push(name);
            }
        }
    }

    instances
}

/// Resolve a _uscan._tcp service instance to hostname, port, and TXT records.
fn resolve_uscan_service(instance: &str) -> Option<EsclScanner> {
    let mut child = Command::new("dns-sd")
        .args(["-L", instance, "_uscan._tcp", "local."])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    std::thread::sleep(Duration::from_secs(2));
    let _ = child.kill();

    let output = child.wait_with_output().ok()?;
    let text = String::from_utf8_lossy(&output.stdout);

    // Output format:
    // Lookup Lexmark Noir Blanc._uscan._tcp.local.
    // DATE: ---Mon 19 Feb 2026---
    //  5:32:14.456  Lexmark\032Noir\032Blanc._uscan._tcp.local. can be reached at Lexmark-Noir-Blanc.local.:80 (interface 4)
    //   txtvers=1 ty=Lexmark ... rs=eSCL ...

    let mut host = String::new();
    let mut port: u16 = 0;
    let mut root = String::from("/eSCL");

    for line in text.lines() {
        // Parse "can be reached at <host>:<port>"
        if let Some(idx) = line.find("can be reached at ") {
            let after = &line[idx + "can be reached at ".len()..];
            // Format: "hostname:port (interface N)"
            if let Some(paren) = after.find(" (") {
                let host_port = &after[..paren];
                if let Some(colon) = host_port.rfind(':') {
                    host = host_port[..colon].to_string();
                    port = host_port[colon + 1..].parse().unwrap_or(80);
                }
            }
        }

        // Parse TXT record for rs= (root path)
        let trimmed = line.trim();
        if trimmed.starts_with("txtvers=") || trimmed.starts_with("ty=") || trimmed.contains("rs=")
        {
            // TXT records are key=value pairs separated by spaces
            for part in trimmed.split_whitespace() {
                if let Some(val) = part.strip_prefix("rs=") {
                    root = if val.starts_with('/') {
                        val.to_string()
                    } else {
                        format!("/{}", val)
                    };
                }
            }
        }
    }

    if host.is_empty() || port == 0 {
        return None;
    }

    Some(EsclScanner {
        name: instance.to_string(),
        host,
        port,
        root,
        caps: None,
    })
}

// ─── eSCL Capabilities ──────────────────────────────────────────

fn fetch_capabilities(host: &str, port: u16, root: &str) -> Option<EsclCapabilities> {
    let url = format!("http://{}:{}{}/ScannerCapabilities", host, port, root);
    eprintln!("[eSCL] Fetching capabilities: {}", url);

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .ok()?;

    let resp = client.get(&url).send().ok()?;
    if !resp.status().is_success() {
        eprintln!("[eSCL] Capabilities request failed: {}", resp.status());
        return None;
    }

    let xml = resp.text().ok()?;
    parse_capabilities(&xml)
}

fn parse_capabilities(xml: &str) -> Option<EsclCapabilities> {
    // Simple XML parsing — extract key fields without a full XML parser
    let mut resolutions = Vec::new();
    let mut color_modes = Vec::new();
    let mut supports_duplex = false;
    let mut supports_adf = false;
    let mut max_width: u32 = 2550;  // default A4 at 300dpi
    let mut max_height: u32 = 3507;

    // Parse resolutions: <scan:XResolution>300</scan:XResolution>
    for cap in extract_xml_values(xml, "scan:XResolution") {
        if let Ok(r) = cap.parse::<u32>() {
            if !resolutions.contains(&r) {
                resolutions.push(r);
            }
        }
    }
    if resolutions.is_empty() {
        // Try DiscreteResolution > XResolution
        for cap in extract_xml_values(xml, "XResolution") {
            if let Ok(r) = cap.parse::<u32>() {
                if !resolutions.contains(&r) {
                    resolutions.push(r);
                }
            }
        }
    }
    if resolutions.is_empty() {
        resolutions = vec![150, 300, 600];
    }
    resolutions.sort();

    // Parse color modes: <scan:ColorMode>RGB24</scan:ColorMode>
    for mode in extract_xml_values(xml, "scan:ColorMode") {
        color_modes.push(mode);
    }
    if color_modes.is_empty() {
        for mode in extract_xml_values(xml, "ColorMode") {
            color_modes.push(mode);
        }
    }

    // Check for ADF: <pwg:InputSource>Feeder</pwg:InputSource> or ADF in platen section
    let input_sources: Vec<String> = extract_xml_values(xml, "pwg:InputSource")
        .into_iter()
        .chain(extract_xml_values(xml, "InputSource"))
        .collect();
    for src in &input_sources {
        let lower = src.to_lowercase();
        if lower.contains("feeder") || lower.contains("adf") {
            supports_adf = true;
        }
    }

    // Check duplex
    if xml.contains("<scan:Duplex>true</scan:Duplex>")
        || xml.contains("<scan:Duplex>T</scan:Duplex>")
        || xml.to_lowercase().contains("duplex=t")
    {
        supports_duplex = true;
    }

    // Parse platen max dimensions
    // Look for MaxWidth and MaxHeight in Platen section
    if let Some(platen_start) = xml.find("Platen") {
        let platen_section = &xml[platen_start..];
        if let Some(end) = platen_section.find("</scan:Platen").or_else(|| platen_section.find("</Platen")) {
            let section = &platen_section[..end];
            for w in extract_xml_values(section, "MaxWidth") {
                if let Ok(v) = w.parse::<u32>() {
                    max_width = v;
                }
            }
            for h in extract_xml_values(section, "MaxHeight") {
                if let Ok(v) = h.parse::<u32>() {
                    max_height = v;
                }
            }
        }
    }

    Some(EsclCapabilities {
        resolutions,
        color_modes,
        supports_duplex,
        supports_adf,
        max_width,
        max_height,
    })
}

/// Extract text content from XML tags like <tag>value</tag>.
fn extract_xml_values(xml: &str, tag: &str) -> Vec<String> {
    let open = format!("<{}>", tag);
    let close = format!("</{}>", tag);
    let mut values = Vec::new();
    let mut search_from = 0;

    while let Some(start) = xml[search_from..].find(&open) {
        let abs_start = search_from + start + open.len();
        if let Some(end) = xml[abs_start..].find(&close) {
            let val = xml[abs_start..abs_start + end].trim().to_string();
            if !val.is_empty() {
                values.push(val);
            }
            search_from = abs_start + end + close.len();
        } else {
            break;
        }
    }
    values
}

// ─── eSCL Scanning ──────────────────────────────────────────────

fn escl_scan(scanner: &EsclScanner, options: &ScanOptions) -> Result<ScanResult, ScannerError> {
    let base = format!("http://{}:{}{}", scanner.host, scanner.port, scanner.root);
    eprintln!("[eSCL] Starting scan on {}", base);

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|e| ScannerError::SystemError(format!("HTTP client: {}", e)))?;

    // Step 1: Build scan settings XML
    let xml = build_scan_xml(scanner, options);
    eprintln!("[eSCL] Scan settings:\n{}", xml);

    // Step 2: POST scan job
    let scan_url = format!("{}/ScanJobs", base);
    let resp = client
        .post(&scan_url)
        .header("Content-Type", "text/xml")
        .body(xml)
        .send()
        .map_err(|e| ScannerError::SystemError(format!("POST ScanJobs: {}", e)))?;

    let status = resp.status();
    eprintln!("[eSCL] ScanJobs response: {}", status);

    if status.as_u16() != 201 {
        let body = resp.text().unwrap_or_default();
        return Err(ScannerError::SystemError(format!(
            "Scanner a refusé le scan (HTTP {}): {}",
            status, body
        )));
    }

    // Step 3: Get job location from Location header
    let location = resp
        .headers()
        .get("location")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .ok_or_else(|| {
            ScannerError::SystemError("Pas de header Location dans la réponse".into())
        })?;

    eprintln!("[eSCL] Scan job location: {}", location);

    // The location may be relative or absolute
    let job_base = if location.starts_with("http") {
        location
    } else {
        format!("http://{}:{}{}", scanner.host, scanner.port, location)
    };

    // Step 4: Poll for scan completion, then download
    // Some scanners need time to physically scan. Poll NextDocument with retries.
    let next_doc_url = format!("{}/NextDocument", job_base);
    eprintln!("[eSCL] Downloading from: {}", next_doc_url);

    let mut last_error = String::new();
    let scan_deadline = std::time::Instant::now() + Duration::from_secs(90);

    loop {
        if std::time::Instant::now() >= scan_deadline {
            return Err(ScannerError::SystemError(format!(
                "Délai de numérisation dépassé. Dernière erreur: {}",
                last_error
            )));
        }

        let dl_resp = client.get(&next_doc_url).send();

        match dl_resp {
            Ok(r) if r.status().is_success() => {
                let content_type = r
                    .headers()
                    .get("content-type")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("unknown")
                    .to_string();
                eprintln!("[eSCL] Download OK, content-type: {}", content_type);

                let image_bytes = r.bytes().map_err(|e| {
                    ScannerError::SystemError(format!("Téléchargement image: {}", e))
                })?;

                if image_bytes.is_empty() {
                    return Err(ScannerError::SystemError(
                        "Image numérisée vide".into(),
                    ));
                }

                eprintln!("[eSCL] Downloaded {} bytes", image_bytes.len());

                // Decode image (JPEG or PNG or PDF)
                let img = image::load_from_memory(&image_bytes).map_err(|e| {
                    ScannerError::SystemError(format!("Décodage image: {}", e))
                })?;

                let width = img.width();
                let height = img.height();

                // Convert to PNG
                let mut png_bytes = Vec::new();
                let mut cursor = std::io::Cursor::new(&mut png_bytes);
                img.write_to(&mut cursor, image::ImageFormat::Png)
                    .map_err(|e| {
                        ScannerError::SystemError(format!("Encodage PNG: {}", e))
                    })?;

                eprintln!(
                    "[eSCL] Scan complete: {}x{} ({} bytes PNG)",
                    width,
                    height,
                    png_bytes.len()
                );

                return Ok(ScanResult {
                    image_data: png_bytes,
                    width,
                    height,
                });
            }
            Ok(r) if r.status().as_u16() == 503 => {
                // Scanner is still scanning, retry
                eprintln!("[eSCL] Scanner busy (503), retrying in 2s...");
                last_error = "Scanner occupé (503)".into();
                std::thread::sleep(Duration::from_secs(2));
            }
            Ok(r) if r.status().as_u16() == 404 => {
                // Job might not be ready yet
                eprintln!("[eSCL] Not found (404), retrying in 2s...");
                last_error = "Document pas encore prêt (404)".into();
                std::thread::sleep(Duration::from_secs(2));
            }
            Ok(r) => {
                let status = r.status();
                let body = r.text().unwrap_or_default();
                return Err(ScannerError::SystemError(format!(
                    "Erreur téléchargement (HTTP {}): {}",
                    status, body
                )));
            }
            Err(e) => {
                eprintln!("[eSCL] Download error: {}, retrying...", e);
                last_error = format!("{}", e);
                std::thread::sleep(Duration::from_secs(2));
            }
        }
    }
}

fn build_scan_xml(scanner: &EsclScanner, options: &ScanOptions) -> String {
    let (paper_w_mm, paper_h_mm) = paper_dimensions(&options.paper_format);

    // eSCL uses 300ths of an inch for scan region
    let width_300ths = (paper_w_mm / 25.4 * 300.0).round() as u32;
    let height_300ths = (paper_h_mm / 25.4 * 300.0).round() as u32;

    // Clamp to scanner max if known
    let (max_w, max_h) = if let Some(ref caps) = scanner.caps {
        (caps.max_width, caps.max_height)
    } else {
        (2550, 3507) // A4 at 300dpi defaults
    };
    let width_300ths = width_300ths.min(max_w);
    let height_300ths = height_300ths.min(max_h);

    // Map color mode
    let escl_color = match color_mode_id(&options.color_mode) {
        2 => "Grayscale8",
        4 => "BlackAndWhite1",
        _ => "RGB24",
    };

    // Input source
    let input_source = if options.duplex
        || options.paper_format.to_lowercase().contains("adf")
    {
        "Feeder"
    } else {
        "Platen"
    };

    let dpi = options.dpi;

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<scan:ScanSettings xmlns:scan="http://schemas.hp.com/imaging/escl/2011/05/03" xmlns:pwg="http://www.pwg.org/schemas/2010/12/sm">
  <pwg:Version>2.5</pwg:Version>
  <pwg:ScanRegions>
    <pwg:ScanRegion>
      <pwg:XOffset>0</pwg:XOffset>
      <pwg:YOffset>0</pwg:YOffset>
      <pwg:Width>{width_300ths}</pwg:Width>
      <pwg:Height>{height_300ths}</pwg:Height>
      <pwg:ContentRegionUnits>escl:ThreeHundredthsOfInches</pwg:ContentRegionUnits>
    </pwg:ScanRegion>
  </pwg:ScanRegions>
  <pwg:InputSource>{input_source}</pwg:InputSource>
  <scan:ColorMode>{escl_color}</scan:ColorMode>
  <scan:XResolution>{dpi}</scan:XResolution>
  <scan:YResolution>{dpi}</scan:YResolution>
  <pwg:DocumentFormat>image/jpeg</pwg:DocumentFormat>
  <scan:Intent>Document</scan:Intent>
</scan:ScanSettings>"#
    )
}

// ─── ScannerBackend implementation ──────────────────────────────

impl ScannerBackend for EsclBackend {
    fn list_devices(&self) -> Result<Vec<ScannerDevice>, ScannerError> {
        let scanners = discover_escl();

        let devices = scanners
            .iter()
            .map(|s| {
                let caps = if let Some(ref c) = s.caps {
                    // Convert 300ths-of-inch to mm
                    let max_w_mm = c.max_width as f64 / 300.0 * 25.4;
                    let max_h_mm = c.max_height as f64 / 300.0 * 25.4;

                    let color_modes = c
                        .color_modes
                        .iter()
                        .map(|m| match m.as_str() {
                            "RGB24" => "Couleur".to_string(),
                            "Grayscale8" => "Niveaux de gris".to_string(),
                            "BlackAndWhite1" => "Noir et blanc".to_string(),
                            other => other.to_string(),
                        })
                        .collect();

                    ScannerCapabilities {
                        resolutions: c.resolutions.clone(),
                        color_modes,
                        supports_duplex: c.supports_duplex,
                        supports_adf: c.supports_adf,
                        max_width_mm: max_w_mm,
                        max_height_mm: max_h_mm,
                    }
                } else {
                    ScannerCapabilities::default()
                };

                ScannerDevice {
                    // Encode eSCL connection info in the device ID
                    id: format!("escl://{}:{}{}", s.host, s.port, s.root),
                    name: s.name.clone(),
                    vendor: String::new(),
                    capabilities: caps,
                }
            })
            .collect();

        Ok(devices)
    }

    fn scan(&self, options: ScanOptions) -> Result<ScanResult, ScannerError> {
        // Parse eSCL URL from device_id: "escl://host:port/root"
        let scanner = parse_escl_device_id(&options.device_id)?;
        escl_scan(&scanner, &options)
    }
}

fn parse_escl_device_id(device_id: &str) -> Result<EsclScanner, ScannerError> {
    if let Some(rest) = device_id.strip_prefix("escl://") {
        // rest = "host:port/root"
        let (host_port, root) = if let Some(slash_idx) = rest.find('/') {
            (&rest[..slash_idx], &rest[slash_idx..])
        } else {
            (rest, "/eSCL")
        };

        let (host, port) = if let Some(colon_idx) = host_port.rfind(':') {
            let h = &host_port[..colon_idx];
            let p = host_port[colon_idx + 1..].parse::<u16>().unwrap_or(80);
            (h.to_string(), p)
        } else {
            (host_port.to_string(), 80)
        };

        Ok(EsclScanner {
            name: host.clone(),
            host,
            port,
            root: root.to_string(),
            caps: None,
        })
    } else {
        Err(ScannerError::SystemError(format!(
            "ID scanner invalide (attendu escl://...): {}",
            device_id
        )))
    }
}
