use lopdf::{Document, Object, ObjectId, Stream, Dictionary};
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use flate2::write::ZlibEncoder;
use flate2::Compression;
use std::io::Write;

// ─── Data Structures ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PdfALevel {
    A1b,
    A2b,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WatermarkPosition {
    Center,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Diagonal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatermarkConfig {
    pub text: String,
    pub opacity: f64,
    pub rotation: f64,
    pub font_size: f64,
    pub color: String,
    pub position: WatermarkPosition,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignatureConfig {
    pub image_base64: String,
    pub page_index: usize,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PdfExportOptions {
    pub pdfa: Option<PdfALevel>,
    pub user_password: Option<String>,
    pub owner_password: Option<String>,
    #[serde(default)]
    pub watermark: Option<WatermarkConfig>,
    #[serde(default)]
    pub signature: Option<SignatureConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AnnotationType {
    Highlight,
    Ellipse,
    TextNote,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Annotation {
    pub annotation_type: AnnotationType,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub color: String,
    pub text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageAnnotations {
    pub page_index: usize,
    pub annotations: Vec<Annotation>,
}

// ─── PDF/A Conformance ───────────────────────────────────────────

static SRGB_ICC: &[u8] = include_bytes!("../srgb.icc");

pub fn apply_pdfa_conformance(
    path: &str,
    level: &PdfALevel,
) -> Result<(), String> {
    let mut doc = Document::load(path)
        .map_err(|e| format!("Failed to load PDF: {}", e))?;

    let (part, conformance) = match level {
        PdfALevel::A1b => (1, "B"),
        PdfALevel::A2b => (2, "B"),
    };

    // 1. Embed sRGB ICC profile as a stream
    let icc_stream = Stream::new(
        Dictionary::from_iter(vec![
            ("N", Object::Integer(3)),
            ("Alternate", Object::Name(b"DeviceRGB".to_vec())),
            ("Length", Object::Integer(SRGB_ICC.len() as i64)),
            ("Filter", Object::Name(b"FlateDecode".to_vec())),
        ]),
        SRGB_ICC.to_vec(),
    ).with_compression(true);
    let icc_id = doc.add_object(Object::Stream(icc_stream));

    // 2. Create OutputIntent dictionary
    let output_intent = Dictionary::from_iter(vec![
        ("Type", Object::Name(b"OutputIntent".to_vec())),
        ("S", Object::Name(b"GTS_PDFA1".to_vec())),
        ("OutputConditionIdentifier", Object::String(
            b"sRGB IEC61966-2.1".to_vec(),
            lopdf::StringFormat::Literal,
        )),
        ("RegistryName", Object::String(
            b"http://www.color.org".to_vec(),
            lopdf::StringFormat::Literal,
        )),
        ("DestOutputProfile", Object::Reference(icc_id)),
    ]);
    let intent_id = doc.add_object(Object::Dictionary(output_intent));

    // 3. Add OutputIntents to catalog
    let catalog_id = doc.catalog()
        .map_err(|e| format!("Failed to get catalog: {}", e))?
        .clone();
    if let Ok(catalog_obj) = doc.get_object_mut(find_catalog_id(&doc)?) {
        if let Object::Dictionary(ref mut cat_dict) = catalog_obj {
            cat_dict.set(
                "OutputIntents",
                Object::Array(vec![Object::Reference(intent_id)]),
            );
        }
    }

    // 4. Add XMP metadata
    let xmp = build_pdfa_xmp(part, conformance);
    let xmp_stream = Stream::new(
        Dictionary::from_iter(vec![
            ("Type", Object::Name(b"Metadata".to_vec())),
            ("Subtype", Object::Name(b"XML".to_vec())),
            ("Length", Object::Integer(xmp.len() as i64)),
        ]),
        xmp.into_bytes(),
    );
    let xmp_id = doc.add_object(Object::Stream(xmp_stream));

    if let Ok(catalog_obj) = doc.get_object_mut(find_catalog_id(&doc)?) {
        if let Object::Dictionary(ref mut cat_dict) = catalog_obj {
            cat_dict.set("Metadata", Object::Reference(xmp_id));
        }
    }

    doc.save(path)
        .map_err(|e| format!("Failed to save PDF/A: {}", e))?;

    Ok(())
}

fn find_catalog_id(doc: &Document) -> Result<ObjectId, String> {
    if let Ok(Object::Reference(id)) = doc.trailer.get(b"Root") {
        Ok(*id)
    } else {
        Err("No catalog found in trailer".into())
    }
}

fn build_pdfa_xmp(part: u32, conformance: &str) -> String {
    format!(
        r#"<?xpacket begin="" id="W5M0MpCehiHzreSzNTczkc9d"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/">
  <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
    <rdf:Description rdf:about=""
      xmlns:dc="http://purl.org/dc/elements/1.1/"
      xmlns:pdfaid="http://www.aiim.org/pdfa/ns/id/">
      <pdfaid:part>{}</pdfaid:part>
      <pdfaid:conformance>{}</pdfaid:conformance>
      <dc:title>
        <rdf:Alt><rdf:li xml:lang="x-default">Document</rdf:li></rdf:Alt>
      </dc:title>
    </rdf:Description>
  </rdf:RDF>
</x:xmpmeta>
<?xpacket end="w"?>"#,
        part, conformance
    )
}

// ─── PDF Encryption (AES-256, PDF 2.0 V=5 R=6) ─────────────────

pub fn encrypt_pdf(
    path: &str,
    user_password: &str,
    owner_password: &str,
) -> Result<(), String> {
    use aes::cipher::{BlockEncryptMut, KeyIvInit, block_padding::Pkcs7};
    use sha2::{Sha256, Sha384, Digest as Sha2Digest};

    type Aes256CbcEnc = cbc::Encryptor<aes::Aes256>;

    let mut doc = Document::load(path)
        .map_err(|e| format!("Failed to load PDF for encryption: {}", e))?;

    // Generate random values
    let mut rng = rand::thread_rng();
    let mut file_encryption_key = [0u8; 32]; // 256-bit file encryption key
    rand::RngCore::fill_bytes(&mut rng, &mut file_encryption_key);

    let mut u_validation_salt = [0u8; 8];
    let mut u_key_salt = [0u8; 8];
    let mut o_validation_salt = [0u8; 8];
    let mut o_key_salt = [0u8; 8];
    let mut file_id = [0u8; 16];
    rand::RngCore::fill_bytes(&mut rng, &mut u_validation_salt);
    rand::RngCore::fill_bytes(&mut rng, &mut u_key_salt);
    rand::RngCore::fill_bytes(&mut rng, &mut o_validation_salt);
    rand::RngCore::fill_bytes(&mut rng, &mut o_key_salt);
    rand::RngCore::fill_bytes(&mut rng, &mut file_id);

    let permissions: i64 = -4; // 0xFFFFFFFC — allow everything except first 2 bits

    // PDF 2.0 password hash algorithm (simplified Algorithm 2.B)
    // For V=5, R=6: use SHA-256 based password validation
    let compute_hash = |password: &[u8], salt: &[u8], u_bytes: &[u8]| -> [u8; 32] {
        let mut input = Vec::new();
        input.extend_from_slice(password);
        input.extend_from_slice(salt);
        input.extend_from_slice(u_bytes);

        let mut k = <Sha256 as Sha2Digest>::digest(&input);

        // 64 rounds of iterative hashing (Algorithm 2.B from ISO 32000-2)
        for round in 0..64u32 {
            let mut k1 = Vec::new();
            // Repeat (password + K + user_key) 64 times
            for _ in 0..64 {
                k1.extend_from_slice(password);
                k1.extend_from_slice(&k);
                k1.extend_from_slice(u_bytes);
            }

            // AES-128-CBC encrypt k1 with first 16 bytes of K as key and next 16 as IV
            let aes_key = &k[..16];
            let aes_iv = &k[16..32];
            let encrypted = aes_cbc_encrypt_128(aes_key, aes_iv, &k1);

            // Sum of first 16 bytes mod 3 determines hash function
            let sum: u32 = encrypted.iter().take(16).map(|&b| b as u32).sum();
            k = match sum % 3 {
                0 => {
                    let d = <Sha256 as Sha2Digest>::digest(&encrypted);
                    let mut arr = [0u8; 32];
                    arr.copy_from_slice(&d);
                    arr.into()
                },
                1 => {
                    let d = <Sha384 as Sha2Digest>::digest(&encrypted);
                    let mut arr = [0u8; 32];
                    arr.copy_from_slice(&d[..32]);
                    arr.into()
                },
                _ => {
                    let d = sha2::Sha512::digest(&encrypted);
                    let mut arr = [0u8; 32];
                    arr.copy_from_slice(&d[..32]);
                    arr.into()
                },
            };

            // Check if we can stop early (last byte of encrypted determines)
            if round >= 63 {
                let last_byte = *encrypted.last().unwrap_or(&0);
                if last_byte <= (round - 32) as u8 {
                    break;
                }
            }
        }

        let mut result = [0u8; 32];
        result.copy_from_slice(&k[..32]);
        result
    };

    // Compute U value (48 bytes: 32-byte hash + 8-byte validation salt + 8-byte key salt)
    let user_pw = if user_password.len() > 127 { &user_password.as_bytes()[..127] } else { user_password.as_bytes() };
    let owner_pw = if owner_password.len() > 127 { &owner_password.as_bytes()[..127] } else { owner_password.as_bytes() };

    let u_hash = compute_hash(user_pw, &u_validation_salt, &[]);
    let mut u_value = Vec::with_capacity(48);
    u_value.extend_from_slice(&u_hash);
    u_value.extend_from_slice(&u_validation_salt);
    u_value.extend_from_slice(&u_key_salt);

    // Compute UE (32 bytes: encrypted file encryption key with user key)
    let u_key_hash = compute_hash(user_pw, &u_key_salt, &[]);
    let ue_iv = [0u8; 16];
    let ue_value = aes_cbc_encrypt_256(&u_key_hash, &ue_iv, &file_encryption_key);

    // Compute O value (48 bytes: 32-byte hash + 8-byte validation salt + 8-byte key salt)
    let o_hash = compute_hash(owner_pw, &o_validation_salt, &u_value);
    let mut o_value = Vec::with_capacity(48);
    o_value.extend_from_slice(&o_hash);
    o_value.extend_from_slice(&o_validation_salt);
    o_value.extend_from_slice(&o_key_salt);

    // Compute OE (32 bytes: encrypted file encryption key with owner key)
    let o_key_hash = compute_hash(owner_pw, &o_key_salt, &u_value);
    let oe_iv = [0u8; 16];
    let oe_value = aes_cbc_encrypt_256(&o_key_hash, &oe_iv, &file_encryption_key);

    // Compute Perms (16 bytes: encrypted permissions)
    let mut perms_input = [0u8; 16];
    let perm_bytes = (permissions as u32).to_le_bytes();
    perms_input[..4].copy_from_slice(&perm_bytes);
    perms_input[4..8].copy_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF]);
    perms_input[8] = b'T'; // EncryptMetadata = true
    perms_input[9] = b'a';
    perms_input[10] = b'd';
    perms_input[11] = b'b';
    rand::RngCore::fill_bytes(&mut rng, &mut perms_input[12..16]);

    let perms_value = aes_ecb_encrypt_256(&file_encryption_key, &perms_input);

    // Encrypt all string and stream objects with AES-256-CBC
    let object_ids: Vec<ObjectId> = doc.objects.keys().cloned().collect();
    for obj_id in object_ids {
        if let Ok(obj) = doc.get_object_mut(obj_id) {
            encrypt_object_aes256(obj, &file_encryption_key);
        }
    }

    // Add Encrypt dictionary to trailer
    let encrypt_dict = Dictionary::from_iter(vec![
        ("Filter", Object::Name(b"Standard".to_vec())),
        ("V", Object::Integer(5)),
        ("R", Object::Integer(6)),
        ("Length", Object::Integer(256)),
        ("CF", Object::Dictionary(Dictionary::from_iter(vec![
            ("StdCF", Object::Dictionary(Dictionary::from_iter(vec![
                ("AuthEvent", Object::Name(b"DocOpen".to_vec())),
                ("CFM", Object::Name(b"AESV3".to_vec())),
                ("Length", Object::Integer(32)),
                ("Type", Object::Name(b"CryptFilter".to_vec())),
            ]))),
        ]))),
        ("StmF", Object::Name(b"StdCF".to_vec())),
        ("StrF", Object::Name(b"StdCF".to_vec())),
        ("P", Object::Integer(permissions)),
        ("O", Object::String(o_value, lopdf::StringFormat::Literal)),
        ("U", Object::String(u_value, lopdf::StringFormat::Literal)),
        ("OE", Object::String(oe_value[..32].to_vec(), lopdf::StringFormat::Literal)),
        ("UE", Object::String(ue_value[..32].to_vec(), lopdf::StringFormat::Literal)),
        ("Perms", Object::String(perms_value[..16].to_vec(), lopdf::StringFormat::Literal)),
        ("EncryptMetadata", Object::Boolean(true)),
    ]);
    let encrypt_id = doc.add_object(Object::Dictionary(encrypt_dict));
    doc.trailer.set("Encrypt", Object::Reference(encrypt_id));

    // Set file ID
    doc.trailer.set("ID", Object::Array(vec![
        Object::String(file_id.to_vec(), lopdf::StringFormat::Literal),
        Object::String(file_id.to_vec(), lopdf::StringFormat::Literal),
    ]));

    doc.save(path)
        .map_err(|e| format!("Failed to save encrypted PDF: {}", e))?;

    Ok(())
}

/// AES-128-CBC encrypt (for password hash algorithm internals)
fn aes_cbc_encrypt_128(key: &[u8], iv: &[u8], data: &[u8]) -> Vec<u8> {
    use aes::cipher::{BlockEncryptMut, KeyIvInit, block_padding::NoPadding};
    type Aes128CbcEnc = cbc::Encryptor<aes::Aes128>;

    // Pad data to 16-byte boundary
    let pad_len = if data.len() % 16 == 0 { data.len() } else { data.len() + (16 - data.len() % 16) };
    let mut buf = vec![0u8; pad_len];
    buf[..data.len()].copy_from_slice(data);

    let enc = Aes128CbcEnc::new_from_slices(key, iv).unwrap();
    enc.encrypt_padded_mut::<NoPadding>(&mut buf, pad_len).unwrap();
    buf
}

/// AES-256-CBC encrypt (for key encryption)
fn aes_cbc_encrypt_256(key: &[u8], iv: &[u8], data: &[u8]) -> Vec<u8> {
    use aes::cipher::{BlockEncryptMut, KeyIvInit, block_padding::NoPadding};
    type Aes256CbcEnc = cbc::Encryptor<aes::Aes256>;

    let pad_len = if data.len() % 16 == 0 { data.len() } else { data.len() + (16 - data.len() % 16) };
    let mut buf = vec![0u8; pad_len];
    buf[..data.len()].copy_from_slice(data);

    let enc = Aes256CbcEnc::new_from_slices(key, iv).unwrap();
    enc.encrypt_padded_mut::<NoPadding>(&mut buf, pad_len).unwrap();
    buf
}

/// AES-256-ECB encrypt a single 16-byte block (for Perms value)
fn aes_ecb_encrypt_256(key: &[u8], data: &[u8; 16]) -> Vec<u8> {
    use aes::cipher::{BlockEncrypt, KeyInit};
    use aes::Aes256;

    let cipher = Aes256::new_from_slice(key).unwrap();
    let mut block = aes::Block::clone_from_slice(data);
    cipher.encrypt_block(&mut block);
    block.to_vec()
}

/// Encrypt a PDF object's strings and streams with AES-256-CBC
fn encrypt_object_aes256(obj: &mut Object, key: &[u8]) {
    match obj {
        Object::String(ref mut data, _) => {
            // AES-256-CBC with random IV prepended
            let mut rng = rand::thread_rng();
            let mut iv = [0u8; 16];
            rand::RngCore::fill_bytes(&mut rng, &mut iv);
            let encrypted = aes_cbc_encrypt_256(key, &iv, data);
            let mut result = Vec::with_capacity(16 + encrypted.len());
            result.extend_from_slice(&iv);
            result.extend_from_slice(&encrypted);
            *data = result;
        }
        Object::Stream(ref mut stream) => {
            let mut rng = rand::thread_rng();
            let mut iv = [0u8; 16];
            rand::RngCore::fill_bytes(&mut rng, &mut iv);
            let encrypted = aes_cbc_encrypt_256(key, &iv, &stream.content);
            let mut result = Vec::with_capacity(16 + encrypted.len());
            result.extend_from_slice(&iv);
            result.extend_from_slice(&encrypted);
            stream.content = result;
        }
        Object::Array(ref mut arr) => {
            for item in arr.iter_mut() {
                encrypt_object_aes256(item, key);
            }
        }
        Object::Dictionary(ref mut dict) => {
            for (_, val) in dict.iter_mut() {
                encrypt_object_aes256(val, key);
            }
        }
        _ => {}
    }
}

// ─── Annotation Rendering ────────────────────────────────────────

pub fn render_annotations_to_pdf(
    path: &str,
    page_annotations: &[PageAnnotations],
) -> Result<(), String> {
    let mut doc = Document::load(path)
        .map_err(|e| format!("Failed to load PDF for annotations: {}", e))?;

    let page_ids: Vec<(u32, ObjectId)> = {
        let pages = doc.get_pages();
        let mut sorted: Vec<(u32, ObjectId)> = pages.into_iter().collect();
        sorted.sort_by_key(|(num, _)| *num);
        sorted
    };

    for pa in page_annotations {
        if pa.page_index >= page_ids.len() {
            continue;
        }
        let (_, page_id) = page_ids[pa.page_index];

        // Get MediaBox dimensions
        let (page_w, page_h) = get_page_dimensions(&doc, page_id);

        // Build annotation drawing operators
        let mut ops = String::new();
        ops.push_str("q\n"); // Save graphics state

        for ann in &pa.annotations {
            let x = ann.x * page_w;
            let y = (1.0 - ann.y - ann.height) * page_h; // PDF Y is bottom-up
            let w = ann.width * page_w;
            let h = ann.height * page_h;

            let (r, g, b) = parse_color(&ann.color);

            match ann.annotation_type {
                AnnotationType::Highlight => {
                    // Yellow highlight with 30% opacity
                    ops.push_str(&format!(
                        "/GS1 gs\n{} {} {} rg\n{} {} {} {} re f\n",
                        r, g, b, x, y, w, h
                    ));
                }
                AnnotationType::Ellipse => {
                    // Red stroke ellipse using 4 Bezier curves
                    let cx = x + w / 2.0;
                    let cy = y + h / 2.0;
                    let rx = w / 2.0;
                    let ry = h / 2.0;
                    let k: f64 = 0.5522847498; // Bezier approximation of circle

                    ops.push_str(&format!(
                        "{} {} {} RG\n2 w\n",
                        r, g, b
                    ));
                    ops.push_str(&format!("{} {} m\n", cx + rx, cy));
                    ops.push_str(&format!(
                        "{} {} {} {} {} {} c\n",
                        cx + rx, cy + ry * k,
                        cx + rx * k, cy + ry,
                        cx, cy + ry
                    ));
                    ops.push_str(&format!(
                        "{} {} {} {} {} {} c\n",
                        cx - rx * k, cy + ry,
                        cx - rx, cy + ry * k,
                        cx - rx, cy
                    ));
                    ops.push_str(&format!(
                        "{} {} {} {} {} {} c\n",
                        cx - rx, cy - ry * k,
                        cx - rx * k, cy - ry,
                        cx, cy - ry
                    ));
                    ops.push_str(&format!(
                        "{} {} {} {} {} {} c\n",
                        cx + rx * k, cy - ry,
                        cx + rx, cy - ry * k,
                        cx + rx, cy
                    ));
                    ops.push_str("S\n");
                }
                AnnotationType::TextNote => {
                    // Small icon + text
                    let text = ann.text.as_deref().unwrap_or("Note");
                    let safe_text = pdf_escape_string(text);

                    // Draw note icon (small filled rect)
                    ops.push_str(&format!(
                        "{} {} {} rg\n{} {} 8 8 re f\n",
                        r, g, b, x, y + h - 8.0
                    ));

                    // Draw text
                    ops.push_str(&format!(
                        "BT\n/F1 9 Tf\n0 0 0 rg\n{} {} Td\n({}) Tj\nET\n",
                        x + 12.0, y + h - 9.0, safe_text
                    ));
                }
            }
        }

        ops.push_str("Q\n"); // Restore graphics state

        // Ensure the page has a Helvetica font resource
        ensure_font_resource(&mut doc, page_id);

        // Ensure ExtGState for transparency
        ensure_ext_gstate(&mut doc, page_id);

        // Append operations to page content stream
        append_to_page_content(&mut doc, page_id, &ops)?;
    }

    doc.save(path)
        .map_err(|e| format!("Failed to save annotated PDF: {}", e))?;

    Ok(())
}

fn get_page_dimensions(doc: &Document, page_id: ObjectId) -> (f64, f64) {
    let default = (595.0, 842.0); // A4

    let page_obj = match doc.get_object(page_id) {
        Ok(obj) => obj,
        Err(_) => return default,
    };
    let dict = match page_obj.as_dict() {
        Ok(d) => d,
        Err(_) => return default,
    };

    // Try MediaBox on this page, then walk up
    if let Ok(mb) = dict.get(b"MediaBox") {
        if let Some(dims) = parse_media_box(doc, mb) {
            return dims;
        }
    }

    // Walk up page tree
    if let Ok(Object::Reference(parent_id)) = dict.get(b"Parent") {
        return get_page_dimensions(doc, *parent_id);
    }

    default
}

fn parse_media_box(doc: &Document, obj: &Object) -> Option<(f64, f64)> {
    let resolved = match obj {
        Object::Reference(r) => doc.get_object(*r).ok()?,
        other => other,
    };
    let arr = resolved.as_array().ok()?;
    if arr.len() < 4 {
        return None;
    }
    let x1 = obj_to_f64(&arr[0]).unwrap_or(0.0);
    let y1 = obj_to_f64(&arr[1]).unwrap_or(0.0);
    let x2 = obj_to_f64(&arr[2]).unwrap_or(595.0);
    let y2 = obj_to_f64(&arr[3]).unwrap_or(842.0);
    Some(((x2 - x1).abs(), (y2 - y1).abs()))
}

fn obj_to_f64(obj: &Object) -> Option<f64> {
    match obj {
        Object::Integer(i) => Some(*i as f64),
        Object::Real(f) => Some(*f as f64),
        _ => None,
    }
}

fn parse_color(color: &str) -> (f64, f64, f64) {
    let hex = color.trim_start_matches('#');
    if hex.len() >= 6 {
        let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(255) as f64 / 255.0;
        let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(255) as f64 / 255.0;
        let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0) as f64 / 255.0;
        (r, g, b)
    } else {
        (1.0, 1.0, 0.0) // Default yellow
    }
}

fn pdf_escape_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('(', "\\(")
        .replace(')', "\\)")
}

fn ensure_font_resource(doc: &mut Document, page_id: ObjectId) {
    // Add Helvetica as /F1 if not present
    let font_dict = Dictionary::from_iter(vec![
        ("Type", Object::Name(b"Font".to_vec())),
        ("Subtype", Object::Name(b"Type1".to_vec())),
        ("BaseFont", Object::Name(b"Helvetica".to_vec())),
    ]);
    let font_id = doc.add_object(Object::Dictionary(font_dict));

    if let Ok(page_obj) = doc.get_object_mut(page_id) {
        if let Object::Dictionary(ref mut page_dict) = page_obj {
            let resources = page_dict
                .get_mut(b"Resources")
                .ok()
                .and_then(|r| {
                    if let Object::Dictionary(ref mut d) = r {
                        Some(d)
                    } else {
                        None
                    }
                });

            if let Some(res_dict) = resources {
                let font_res = res_dict
                    .get_mut(b"Font")
                    .ok()
                    .and_then(|f| {
                        if let Object::Dictionary(ref mut d) = f {
                            Some(d)
                        } else {
                            None
                        }
                    });

                if let Some(fonts) = font_res {
                    if fonts.get(b"F1").is_err() {
                        fonts.set("F1", Object::Reference(font_id));
                    }
                } else {
                    let mut fonts = Dictionary::new();
                    fonts.set("F1", Object::Reference(font_id));
                    res_dict.set("Font", Object::Dictionary(fonts));
                }
            } else {
                let mut fonts = Dictionary::new();
                fonts.set("F1", Object::Reference(font_id));
                let mut res = Dictionary::new();
                res.set("Font", Object::Dictionary(fonts));
                page_dict.set("Resources", Object::Dictionary(res));
            }
        }
    }
}

fn ensure_ext_gstate(doc: &mut Document, page_id: ObjectId) {
    // Add graphics state for 30% opacity highlights
    let gs_dict = Dictionary::from_iter(vec![
        ("Type", Object::Name(b"ExtGState".to_vec())),
        ("ca", Object::Real(0.3)),
        ("CA", Object::Real(0.3)),
    ]);
    let gs_id = doc.add_object(Object::Dictionary(gs_dict));

    if let Ok(page_obj) = doc.get_object_mut(page_id) {
        if let Object::Dictionary(ref mut page_dict) = page_obj {
            let resources = page_dict
                .get_mut(b"Resources")
                .ok()
                .and_then(|r| {
                    if let Object::Dictionary(ref mut d) = r {
                        Some(d)
                    } else {
                        None
                    }
                });

            if let Some(res_dict) = resources {
                let ext_gstate = res_dict
                    .get_mut(b"ExtGState")
                    .ok()
                    .and_then(|g| {
                        if let Object::Dictionary(ref mut d) = g {
                            Some(d)
                        } else {
                            None
                        }
                    });

                if let Some(gs) = ext_gstate {
                    if gs.get(b"GS1").is_err() {
                        gs.set("GS1", Object::Reference(gs_id));
                    }
                } else {
                    let mut gs = Dictionary::new();
                    gs.set("GS1", Object::Reference(gs_id));
                    res_dict.set("ExtGState", Object::Dictionary(gs));
                }
            } else {
                let mut gs = Dictionary::new();
                gs.set("GS1", Object::Reference(gs_id));
                let mut res = Dictionary::new();
                res.set("ExtGState", Object::Dictionary(gs));
                page_dict.set("Resources", Object::Dictionary(res));
            }
        }
    }
}

fn append_to_page_content(
    doc: &mut Document,
    page_id: ObjectId,
    ops: &str,
) -> Result<(), String> {
    let new_stream = Stream::new(Dictionary::new(), ops.as_bytes().to_vec());
    let new_content_id = doc.add_object(Object::Stream(new_stream));

    if let Ok(page_obj) = doc.get_object_mut(page_id) {
        if let Object::Dictionary(ref mut page_dict) = page_obj {
            match page_dict.get(b"Contents") {
                Ok(Object::Reference(existing_id)) => {
                    let existing_id = *existing_id;
                    page_dict.set(
                        "Contents",
                        Object::Array(vec![
                            Object::Reference(existing_id),
                            Object::Reference(new_content_id),
                        ]),
                    );
                }
                Ok(Object::Array(arr)) => {
                    let mut new_arr = arr.clone();
                    new_arr.push(Object::Reference(new_content_id));
                    page_dict.set("Contents", Object::Array(new_arr));
                }
                _ => {
                    page_dict.set("Contents", Object::Reference(new_content_id));
                }
            }
        }
    }

    Ok(())
}

// ─── Watermark Rendering ─────────────────────────────────────────

pub fn render_watermark_to_pdf(
    path: &str,
    config: &WatermarkConfig,
) -> Result<(), String> {
    let mut doc = Document::load(path)
        .map_err(|e| format!("Failed to load PDF for watermark: {}", e))?;

    let page_ids: Vec<(u32, ObjectId)> = {
        let pages = doc.get_pages();
        let mut sorted: Vec<(u32, ObjectId)> = pages.into_iter().collect();
        sorted.sort_by_key(|(num, _)| *num);
        sorted
    };

    let (r, g, b) = parse_color(&config.color);
    let safe_text = pdf_escape_string(&config.text);

    for (_, page_id) in &page_ids {
        let (page_w, page_h) = get_page_dimensions(&doc, *page_id);

        // Create ExtGState for watermark opacity
        let gs_dict = Dictionary::from_iter(vec![
            ("Type", Object::Name(b"ExtGState".to_vec())),
            ("ca", Object::Real(config.opacity as f32)),
            ("CA", Object::Real(config.opacity as f32)),
        ]);
        let gs_id = doc.add_object(Object::Dictionary(gs_dict));

        // Add GS2 to page resources
        add_ext_gstate_named(&mut doc, *page_id, "GS2", gs_id);

        // Ensure font resource
        ensure_font_resource(&mut doc, *page_id);

        // Compute position and rotation
        let (tx, ty, angle_rad) = match config.position {
            WatermarkPosition::Center => (page_w / 2.0, page_h / 2.0, 0.0_f64),
            WatermarkPosition::TopLeft => (page_w * 0.15, page_h * 0.9, 0.0),
            WatermarkPosition::TopRight => (page_w * 0.85, page_h * 0.9, 0.0),
            WatermarkPosition::BottomLeft => (page_w * 0.15, page_h * 0.1, 0.0),
            WatermarkPosition::BottomRight => (page_w * 0.85, page_h * 0.1, 0.0),
            WatermarkPosition::Diagonal => (page_w / 2.0, page_h / 2.0, config.rotation.to_radians()),
        };

        let angle_rad = if !matches!(config.position, WatermarkPosition::Diagonal) && config.rotation.abs() > 0.01 {
            config.rotation.to_radians()
        } else {
            angle_rad
        };

        let cos_a = angle_rad.cos();
        let sin_a = angle_rad.sin();

        // Approximate text width for centering (Helvetica ~0.5 * font_size per char)
        let approx_text_width = config.text.len() as f64 * config.font_size * 0.5;
        let text_offset_x = -approx_text_width / 2.0;
        let text_offset_y = -config.font_size / 3.0; // Approximate vertical centering

        let mut ops = String::new();
        ops.push_str("q\n");
        ops.push_str("/GS2 gs\n");
        // Transform: translate to position, then rotate
        ops.push_str(&format!(
            "{} {} {} {} {} {} cm\n",
            cos_a, sin_a, -sin_a, cos_a, tx, ty
        ));
        ops.push_str(&format!(
            "BT\n/F1 {} Tf\n{} {} {} rg\n{} {} Td\n({}) Tj\nET\n",
            config.font_size, r, g, b, text_offset_x, text_offset_y, safe_text
        ));
        ops.push_str("Q\n");

        append_to_page_content(&mut doc, *page_id, &ops)?;
    }

    doc.save(path)
        .map_err(|e| format!("Failed to save watermarked PDF: {}", e))?;

    Ok(())
}

fn add_ext_gstate_named(doc: &mut Document, page_id: ObjectId, name: &str, gs_id: ObjectId) {
    if let Ok(page_obj) = doc.get_object_mut(page_id) {
        if let Object::Dictionary(ref mut page_dict) = page_obj {
            let resources = page_dict
                .get_mut(b"Resources")
                .ok()
                .and_then(|r| {
                    if let Object::Dictionary(ref mut d) = r {
                        Some(d)
                    } else {
                        None
                    }
                });

            if let Some(res_dict) = resources {
                let ext_gstate = res_dict
                    .get_mut(b"ExtGState")
                    .ok()
                    .and_then(|g| {
                        if let Object::Dictionary(ref mut d) = g {
                            Some(d)
                        } else {
                            None
                        }
                    });

                if let Some(gs) = ext_gstate {
                    gs.set(name, Object::Reference(gs_id));
                } else {
                    let mut gs = Dictionary::new();
                    gs.set(name, Object::Reference(gs_id));
                    res_dict.set("ExtGState", Object::Dictionary(gs));
                }
            } else {
                let mut gs = Dictionary::new();
                gs.set(name, Object::Reference(gs_id));
                let mut res = Dictionary::new();
                res.set("ExtGState", Object::Dictionary(gs));
                page_dict.set("Resources", Object::Dictionary(res));
            }
        }
    }
}

// ─── Signature Rendering ─────────────────────────────────────────

pub fn render_signature_to_pdf(
    path: &str,
    config: &SignatureConfig,
) -> Result<(), String> {
    use base64::{engine::general_purpose::STANDARD as BASE64, Engine};

    let png_bytes = BASE64.decode(&config.image_base64)
        .map_err(|e| format!("Failed to decode signature base64: {}", e))?;

    let img = image::load_from_memory(&png_bytes)
        .map_err(|e| format!("Failed to decode signature image: {}", e))?;

    let rgba = img.to_rgba8();
    let img_w = rgba.width();
    let img_h = rgba.height();

    // Separate RGB and alpha channels
    let pixel_count = (img_w * img_h) as usize;
    let mut rgb_data = Vec::with_capacity(pixel_count * 3);
    let mut alpha_data = Vec::with_capacity(pixel_count);

    for pixel in rgba.pixels() {
        rgb_data.push(pixel[0]);
        rgb_data.push(pixel[1]);
        rgb_data.push(pixel[2]);
        alpha_data.push(pixel[3]);
    }

    // Compress RGB data with zlib
    let compressed_rgb = zlib_compress(&rgb_data)?;
    let compressed_alpha = zlib_compress(&alpha_data)?;

    let mut doc = Document::load(path)
        .map_err(|e| format!("Failed to load PDF for signature: {}", e))?;

    let page_ids: Vec<(u32, ObjectId)> = {
        let pages = doc.get_pages();
        let mut sorted: Vec<(u32, ObjectId)> = pages.into_iter().collect();
        sorted.sort_by_key(|(num, _)| *num);
        sorted
    };

    if config.page_index >= page_ids.len() {
        return Err(format!("Signature page index {} out of range", config.page_index));
    }

    let (_, page_id) = page_ids[config.page_index];
    let (page_w, page_h) = get_page_dimensions(&doc, page_id);

    // Create soft mask (alpha channel) XObject
    let smask_stream = Stream::new(
        Dictionary::from_iter(vec![
            ("Type", Object::Name(b"XObject".to_vec())),
            ("Subtype", Object::Name(b"Image".to_vec())),
            ("Width", Object::Integer(img_w as i64)),
            ("Height", Object::Integer(img_h as i64)),
            ("ColorSpace", Object::Name(b"DeviceGray".to_vec())),
            ("BitsPerComponent", Object::Integer(8)),
            ("Filter", Object::Name(b"FlateDecode".to_vec())),
            ("Length", Object::Integer(compressed_alpha.len() as i64)),
        ]),
        compressed_alpha,
    );
    let smask_id = doc.add_object(Object::Stream(smask_stream));

    // Create image XObject with soft mask
    let img_stream = Stream::new(
        Dictionary::from_iter(vec![
            ("Type", Object::Name(b"XObject".to_vec())),
            ("Subtype", Object::Name(b"Image".to_vec())),
            ("Width", Object::Integer(img_w as i64)),
            ("Height", Object::Integer(img_h as i64)),
            ("ColorSpace", Object::Name(b"DeviceRGB".to_vec())),
            ("BitsPerComponent", Object::Integer(8)),
            ("Filter", Object::Name(b"FlateDecode".to_vec())),
            ("Length", Object::Integer(compressed_rgb.len() as i64)),
            ("SMask", Object::Reference(smask_id)),
        ]),
        compressed_rgb,
    );
    let img_id = doc.add_object(Object::Stream(img_stream));

    // Add image as XObject resource /Sig1
    add_xobject_resource(&mut doc, page_id, "Sig1", img_id);

    // Compute placement in PDF coordinates
    let sig_x = config.x * page_w;
    let sig_y = (1.0 - config.y - config.height) * page_h; // PDF Y is bottom-up
    let sig_w = config.width * page_w;
    let sig_h = config.height * page_h;

    let ops = format!(
        "q\n{} 0 0 {} {} {} cm\n/Sig1 Do\nQ\n",
        sig_w, sig_h, sig_x, sig_y
    );

    append_to_page_content(&mut doc, page_id, &ops)?;

    doc.save(path)
        .map_err(|e| format!("Failed to save signed PDF: {}", e))?;

    Ok(())
}

fn zlib_compress(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data)
        .map_err(|e| format!("Compression error: {}", e))?;
    encoder.finish()
        .map_err(|e| format!("Compression finish error: {}", e))
}

fn add_xobject_resource(doc: &mut Document, page_id: ObjectId, name: &str, xobj_id: ObjectId) {
    if let Ok(page_obj) = doc.get_object_mut(page_id) {
        if let Object::Dictionary(ref mut page_dict) = page_obj {
            let resources = page_dict
                .get_mut(b"Resources")
                .ok()
                .and_then(|r| {
                    if let Object::Dictionary(ref mut d) = r {
                        Some(d)
                    } else {
                        None
                    }
                });

            if let Some(res_dict) = resources {
                let xobjects = res_dict
                    .get_mut(b"XObject")
                    .ok()
                    .and_then(|x| {
                        if let Object::Dictionary(ref mut d) = x {
                            Some(d)
                        } else {
                            None
                        }
                    });

                if let Some(xobjs) = xobjects {
                    xobjs.set(name, Object::Reference(xobj_id));
                } else {
                    let mut xobjs = Dictionary::new();
                    xobjs.set(name, Object::Reference(xobj_id));
                    res_dict.set("XObject", Object::Dictionary(xobjs));
                }
            } else {
                let mut xobjs = Dictionary::new();
                xobjs.set(name, Object::Reference(xobj_id));
                let mut res = Dictionary::new();
                res.set("XObject", Object::Dictionary(xobjs));
                page_dict.set("Resources", Object::Dictionary(res));
            }
        }
    }
}

// ─── SHA-256 Integrity ───────────────────────────────────────────

pub fn compute_sha256(path: &str) -> Result<String, String> {
    let data = std::fs::read(path)
        .map_err(|e| format!("Failed to read file for SHA-256: {}", e))?;
    let mut hasher = Sha256::new();
    hasher.update(&data);
    let result = hasher.finalize();
    Ok(format!("{:x}", result))
}

// ─── Post-Processing Pipeline ────────────────────────────────────

pub fn postprocess_pdf(
    path: &str,
    export_options: Option<&PdfExportOptions>,
    annotations: Option<&[PageAnnotations]>,
) -> Result<Option<String>, String> {
    // 1. Render annotations (if any)
    if let Some(page_anns) = annotations {
        if !page_anns.is_empty() {
            render_annotations_to_pdf(path, page_anns)?;
        }
    }

    if let Some(opts) = export_options {
        // 2. Render signature (if any)
        if let Some(ref sig) = opts.signature {
            render_signature_to_pdf(path, sig)?;
        }

        // 3. Render watermark (if any)
        if let Some(ref wm) = opts.watermark {
            render_watermark_to_pdf(path, wm)?;
        }

        // 4. Apply PDF/A conformance (if requested)
        if let Some(ref level) = opts.pdfa {
            apply_pdfa_conformance(path, level)?;
        }

        // 5. Apply encryption (must be last before hash)
        if opts.user_password.is_some() || opts.owner_password.is_some() {
            let user_pw = opts.user_password.as_deref().unwrap_or("");
            let owner_pw = opts.owner_password.as_deref().unwrap_or(user_pw);
            encrypt_pdf(path, user_pw, owner_pw)?;
        }
    }

    // 6. Compute SHA-256 hash
    let sha256 = compute_sha256(path)?;
    Ok(Some(sha256))
}
