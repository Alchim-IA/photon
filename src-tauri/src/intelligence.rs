use crate::ocr::OcrResult;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::LazyLock;

// ─── Pre-compiled Regex Patterns ────────────────────────────────

static RE_AMOUNT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(\d{1,3}(?:[.\s]\d{3})*(?:[,\.]\d{2})?)\s*(?:€|EUR|euros?)").unwrap()
});
static RE_DATE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(\d{1,2}[/\-\.]\d{1,2}[/\-\.]\d{2,4})\b").unwrap()
});
static RE_IBAN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b([A-Z]{2}\d{2}\s?(?:\d{4}\s?){4,7}\d{1,4})\b").unwrap()
});
static RE_EMAIL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b([a-zA-Z0-9._%+\-]+@[a-zA-Z0-9.\-]+\.[a-zA-Z]{2,})\b").unwrap()
});
static RE_PHONE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(?:\+33|0)\s?[1-9](?:[\s.\-]?\d{2}){4}\b").unwrap()
});
static RE_SIRET: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(\d{3}\s?\d{3}\s?\d{3}\s?\d{5})\b").unwrap()
});
static RE_SIREN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(\d{3}\s?\d{3}\s?\d{3})\b").unwrap()
});
static RE_TOTAL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:total|net\s+[àa]\s+payer|montant\s+ttc)\s*:?\s*(\d{1,3}(?:[.\s]\d{3})*(?:[,\.]\d{2})?)\s*(?:€|EUR)?").unwrap()
});
static RE_DOC_NUM: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:n[°o]\s*(?:facture|devis|commande|dossier|contrat))\s*:?\s*([A-Z0-9\-/]+)").unwrap()
});

// ─── Document Types ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum DocumentType {
    Facture,
    CarteIdentite,
    Contrat,
    Courrier,
    Recu,
    Formulaire,
    CV,
    Ordonnance,
    ReleveBancaire,
    BulletinPaie,
    Devis,
    Autre,
}

impl DocumentType {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Facture => "Facture",
            Self::CarteIdentite => "Carte d'identité",
            Self::Contrat => "Contrat",
            Self::Courrier => "Courrier",
            Self::Recu => "Reçu",
            Self::Formulaire => "Formulaire",
            Self::CV => "CV",
            Self::Ordonnance => "Ordonnance",
            Self::ReleveBancaire => "Relevé bancaire",
            Self::BulletinPaie => "Bulletin de paie",
            Self::Devis => "Devis",
            Self::Autre => "Autre",
        }
    }

    pub fn all() -> &'static [DocumentType] {
        &[
            Self::Facture,
            Self::CarteIdentite,
            Self::Contrat,
            Self::Courrier,
            Self::Recu,
            Self::Formulaire,
            Self::CV,
            Self::Ordonnance,
            Self::ReleveBancaire,
            Self::BulletinPaie,
            Self::Devis,
            Self::Autre,
        ]
    }

    pub fn suggested_folder(&self) -> &'static str {
        match self {
            Self::Facture => "Factures",
            Self::CarteIdentite => "Identité",
            Self::Contrat => "Contrats",
            Self::Courrier => "Courrier",
            Self::Recu => "Reçus",
            Self::Formulaire => "Formulaires",
            Self::CV => "CV",
            Self::Ordonnance => "Médical",
            Self::ReleveBancaire => "Banque",
            Self::BulletinPaie => "Paie",
            Self::Devis => "Devis",
            Self::Autre => "Divers",
        }
    }
}

// ─── Classification Result ──────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassificationResult {
    pub doc_type: DocumentType,
    pub confidence: f32,
    pub scores: Vec<(String, f32)>,
}

// ─── Extracted Data ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedData {
    pub fields: HashMap<String, Vec<String>>,
}

// ─── Smart Suggestion ───────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartSuggestion {
    pub suggested_name: String,
    pub suggested_folder: String,
    pub suggested_tags: Vec<String>,
    pub classification: ClassificationResult,
    pub extracted_data: ExtractedData,
}

// ─── Classification Engine ──────────────────────────────────────

struct KeywordSet {
    doc_type: DocumentType,
    keywords: Vec<(&'static str, f32)>,
}

fn build_keyword_sets() -> Vec<KeywordSet> {
    vec![
        KeywordSet {
            doc_type: DocumentType::Facture,
            keywords: vec![
                ("facture", 5.0), ("invoice", 4.0), ("n° facture", 6.0),
                ("numéro de facture", 6.0), ("montant ttc", 5.0), ("montant ht", 4.0),
                ("tva", 4.0), ("total à payer", 5.0), ("échéance", 3.0),
                ("bon de commande", 3.0), ("siret", 3.0), ("siren", 2.0),
                ("rib", 2.0), ("iban", 3.0), ("règlement", 3.0),
                ("date de facturation", 5.0), ("net à payer", 5.0),
            ],
        },
        KeywordSet {
            doc_type: DocumentType::CarteIdentite,
            keywords: vec![
                ("carte nationale d'identité", 8.0), ("carte d'identité", 7.0),
                ("identity card", 6.0), ("république française", 4.0),
                ("nationalité", 3.0), ("née le", 3.0), ("né le", 3.0),
                ("lieu de naissance", 4.0), ("passeport", 6.0), ("passport", 5.0),
                ("permis de conduire", 6.0), ("driving licence", 5.0),
                ("date d'expiration", 3.0), ("n° carte", 4.0),
            ],
        },
        KeywordSet {
            doc_type: DocumentType::Contrat,
            keywords: vec![
                ("contrat", 5.0), ("contract", 4.0), ("entre les soussignés", 6.0),
                ("parties", 3.0), ("article", 2.0), ("clause", 3.0),
                ("durée du contrat", 5.0), ("résiliation", 4.0), ("avenant", 4.0),
                ("signataire", 3.0), ("lu et approuvé", 5.0), ("fait à", 2.0),
                ("conditions générales", 4.0), ("objet du contrat", 5.0),
                ("bail", 4.0), ("location", 2.0),
            ],
        },
        KeywordSet {
            doc_type: DocumentType::Courrier,
            keywords: vec![
                ("madame", 2.0), ("monsieur", 2.0), ("cher", 1.5),
                ("cordialement", 3.0), ("salutations", 3.0), ("veuillez agréer", 4.0),
                ("objet :", 3.0), ("à l'attention de", 4.0),
                ("p.j.", 2.0), ("pièce jointe", 2.0), ("recommandé", 3.0),
                ("lettre", 2.0), ("courrier", 3.0), ("expéditeur", 2.0),
            ],
        },
        KeywordSet {
            doc_type: DocumentType::Recu,
            keywords: vec![
                ("reçu", 5.0), ("receipt", 4.0), ("ticket de caisse", 6.0),
                ("total", 2.0), ("cb", 2.0), ("carte bancaire", 3.0),
                ("paiement", 3.0), ("merci de votre visite", 4.0),
                ("encaissé", 3.0), ("espèces", 2.0), ("rendu", 2.0),
            ],
        },
        KeywordSet {
            doc_type: DocumentType::Formulaire,
            keywords: vec![
                ("formulaire", 5.0), ("form", 3.0), ("à remplir", 4.0),
                ("nom :", 1.5), ("prénom :", 1.5), ("adresse :", 1.5),
                ("signature", 2.0), ("date :", 1.0), ("cochez", 3.0),
                ("case à cocher", 3.0), ("cerfa", 5.0), ("n°", 1.0),
                ("déclaration", 3.0),
            ],
        },
        KeywordSet {
            doc_type: DocumentType::CV,
            keywords: vec![
                ("curriculum vitae", 8.0), ("cv", 3.0), ("expérience professionnelle", 6.0),
                ("formation", 2.0), ("compétences", 4.0), ("langues", 2.0),
                ("centres d'intérêt", 3.0), ("diplôme", 3.0), ("stage", 2.0),
                ("poste actuel", 4.0), ("profil", 2.0), ("références", 2.0),
            ],
        },
        KeywordSet {
            doc_type: DocumentType::Ordonnance,
            keywords: vec![
                ("ordonnance", 6.0), ("prescription", 5.0), ("dr ", 3.0),
                ("docteur", 3.0), ("médecin", 4.0), ("patient", 3.0),
                ("posologie", 5.0), ("comprimé", 4.0), ("mg", 2.0),
                ("pharmacie", 3.0), ("rpps", 4.0), ("adeli", 4.0),
                ("sécurité sociale", 3.0), ("matin", 1.0), ("soir", 1.0),
            ],
        },
        KeywordSet {
            doc_type: DocumentType::ReleveBancaire,
            keywords: vec![
                ("relevé de compte", 7.0), ("relevé bancaire", 7.0),
                ("solde", 3.0), ("débit", 3.0), ("crédit", 3.0),
                ("banque", 3.0), ("compte courant", 5.0), ("bic", 3.0),
                ("iban", 3.0), ("opérations", 2.0), ("virement", 3.0),
                ("prélèvement", 3.0), ("solde précédent", 5.0),
                ("nouveau solde", 5.0), ("agence", 2.0),
            ],
        },
        KeywordSet {
            doc_type: DocumentType::BulletinPaie,
            keywords: vec![
                ("bulletin de paie", 8.0), ("bulletin de salaire", 8.0),
                ("fiche de paie", 7.0), ("salaire brut", 6.0),
                ("salaire net", 6.0), ("net à payer", 4.0),
                ("cotisations", 4.0), ("urssaf", 5.0), ("csg", 3.0),
                ("convention collective", 5.0), ("employeur", 3.0),
                ("salarié", 3.0), ("période", 2.0), ("congés", 2.0),
            ],
        },
        KeywordSet {
            doc_type: DocumentType::Devis,
            keywords: vec![
                ("devis", 6.0), ("estimate", 4.0), ("quote", 3.0),
                ("proposition commerciale", 5.0), ("offre de prix", 5.0),
                ("validité", 3.0), ("durée de validité", 4.0),
                ("bon pour accord", 5.0), ("prix unitaire", 4.0),
                ("remise", 2.0), ("acompte", 3.0), ("conditions de paiement", 3.0),
            ],
        },
    ]
}

/// Classifies a document based on its OCR text using weighted keyword scoring.
pub fn classify_document(ocr_text: &str) -> ClassificationResult {
    let text_lower = ocr_text.to_lowercase();
    let keyword_sets = build_keyword_sets();

    let mut scores: Vec<(String, f32)> = Vec::new();
    let mut best_type = DocumentType::Autre;
    let mut best_score: f32 = 0.0;

    for ks in &keyword_sets {
        let mut score: f32 = 0.0;
        for &(keyword, weight) in &ks.keywords {
            let kw_lower = keyword.to_lowercase();
            // Count occurrences (capped at 3 to avoid gaming)
            let count = text_lower.matches(&kw_lower).count().min(3) as f32;
            if count > 0.0 {
                score += weight * (1.0 + (count - 1.0) * 0.3); // diminishing returns
            }
        }
        scores.push((ks.doc_type.label().to_string(), score));
        if score > best_score {
            best_score = score;
            best_type = ks.doc_type.clone();
        }
    }

    // Compute confidence as normalized score (0.0-1.0)
    let total_score: f32 = scores.iter().map(|(_, s)| s).sum();
    let confidence = if total_score > 0.0 {
        (best_score / total_score).min(1.0)
    } else {
        0.0
    };

    // Minimum threshold: if best score is too low, classify as Autre
    if best_score < 3.0 {
        return ClassificationResult {
            doc_type: DocumentType::Autre,
            confidence: 0.0,
            scores,
        };
    }

    // Sort scores descending
    scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    ClassificationResult {
        doc_type: best_type,
        confidence,
        scores,
    }
}

// ─── Data Extraction Engine ─────────────────────────────────────

/// Extracts structured data from OCR text using regex patterns.
pub fn extract_data(ocr_text: &str) -> ExtractedData {
    let mut fields: HashMap<String, Vec<String>> = HashMap::new();

    // Amounts (€)
    let amounts: Vec<String> = RE_AMOUNT.captures_iter(ocr_text)
        .map(|c| c[1].trim().to_string()).collect();
    if !amounts.is_empty() { fields.insert("montants".into(), amounts); }

    // Dates (DD/MM/YYYY, DD-MM-YYYY, DD.MM.YYYY)
    let dates: Vec<String> = RE_DATE.captures_iter(ocr_text)
        .map(|c| c[1].to_string()).collect();
    if !dates.is_empty() { fields.insert("dates".into(), dates); }

    // IBAN
    let ibans: Vec<String> = RE_IBAN.captures_iter(ocr_text)
        .map(|c| c[1].to_string()).collect();
    if !ibans.is_empty() { fields.insert("iban".into(), ibans); }

    // Emails
    let emails: Vec<String> = RE_EMAIL.captures_iter(ocr_text)
        .map(|c| c[1].to_string()).collect();
    if !emails.is_empty() { fields.insert("emails".into(), emails); }

    // Phone numbers (French format)
    let phones: Vec<String> = RE_PHONE.find_iter(ocr_text)
        .map(|m| m.as_str().to_string()).collect();
    if !phones.is_empty() { fields.insert("telephones".into(), phones); }

    // SIRET/SIREN
    let sirets: Vec<String> = RE_SIRET.captures_iter(ocr_text)
        .map(|c| c[1].to_string()).collect();
    if !sirets.is_empty() { fields.insert("siret".into(), sirets); }

    // Only add SIREN if no SIRET found (SIRET contains SIREN)
    if !fields.contains_key("siret") {
        let sirens: Vec<String> = RE_SIREN.captures_iter(ocr_text)
            .map(|c| c[1].to_string()).collect();
        if !sirens.is_empty() { fields.insert("siren".into(), sirens); }
    }

    // "Total" line amounts (specific pattern for invoices)
    let totals: Vec<String> = RE_TOTAL.captures_iter(ocr_text)
        .map(|c| c[1].trim().to_string()).collect();
    if !totals.is_empty() { fields.insert("total".into(), totals); }

    // Invoice/document number
    let numbers: Vec<String> = RE_DOC_NUM.captures_iter(ocr_text)
        .map(|c| c[1].to_string()).collect();
    if !numbers.is_empty() { fields.insert("numero_document".into(), numbers); }

    ExtractedData { fields }
}

// ─── Smart Suggestions ──────────────────────────────────────────

/// Generates smart suggestions based on classification and extracted data.
pub fn generate_suggestions(
    classification: &ClassificationResult,
    extracted_data: &ExtractedData,
) -> SmartSuggestion {
    let doc_type = &classification.doc_type;
    let now = chrono::Local::now();

    // Build suggested name
    let base = doc_type.label();
    let date_part = extracted_data
        .fields
        .get("dates")
        .and_then(|d| d.first())
        .cloned()
        .unwrap_or_else(|| now.format("%Y-%m-%d").to_string());

    let extra = match doc_type {
        DocumentType::Facture | DocumentType::Devis => {
            extracted_data
                .fields
                .get("total")
                .or_else(|| extracted_data.fields.get("montants"))
                .and_then(|a| a.first())
                .map(|a| format!("_{}€", a))
                .unwrap_or_default()
        }
        DocumentType::BulletinPaie => {
            // Try to get month from date
            String::new()
        }
        _ => String::new(),
    };

    let doc_num = extracted_data
        .fields
        .get("numero_document")
        .and_then(|n| n.first())
        .map(|n| format!("_{}", n))
        .unwrap_or_default();

    let suggested_name = format!("{}_{}{}{}", base, date_part, doc_num, extra);

    // Build suggested tags
    let mut suggested_tags = vec![doc_type.label().to_string()];

    // Add year tag from extracted date
    if let Some(dates) = extracted_data.fields.get("dates") {
        if let Some(date) = dates.first() {
            // Try to extract year from DD/MM/YYYY or similar
            let parts: Vec<&str> = date.split(|c: char| c == '/' || c == '-' || c == '.').collect();
            if let Some(year) = parts.last() {
                if year.len() == 4 {
                    suggested_tags.push(year.to_string());
                } else if year.len() == 2 {
                    suggested_tags.push(format!("20{}", year));
                }
            }
        }
    }

    // Add amount-based tags
    if extracted_data.fields.contains_key("montants") || extracted_data.fields.contains_key("total") {
        suggested_tags.push("Financier".to_string());
    }

    // Type-specific tags
    match doc_type {
        DocumentType::Ordonnance => suggested_tags.push("Santé".to_string()),
        DocumentType::CarteIdentite => suggested_tags.push("Identité".to_string()),
        DocumentType::BulletinPaie | DocumentType::Contrat => suggested_tags.push("Emploi".to_string()),
        DocumentType::ReleveBancaire => suggested_tags.push("Banque".to_string()),
        _ => {}
    }

    SmartSuggestion {
        suggested_name,
        suggested_folder: doc_type.suggested_folder().to_string(),
        suggested_tags,
        classification: classification.clone(),
        extracted_data: extracted_data.clone(),
    }
}

// ─── Automation Rules Engine ────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomationRule {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub condition_logic: ConditionLogic,
    pub conditions: Vec<RuleCondition>,
    pub actions: Vec<RuleAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConditionLogic {
    And,
    Or,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleCondition {
    pub field: ConditionField,
    pub operator: ConditionOperator,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConditionField {
    DocumentType,
    Tag,
    TextContains,
    AmountAbove,
    AmountBelow,
    HasField,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConditionOperator {
    Equals,
    NotEquals,
    Contains,
    Regex,
    GreaterThan,
    LessThan,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleAction {
    pub action_type: ActionType,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActionType {
    Rename,
    MoveToFolder,
    AddTag,
    ApplyProfile,
}

/// Context passed to rule evaluation.
pub struct RuleContext<'a> {
    pub classification: &'a ClassificationResult,
    pub extracted_data: &'a ExtractedData,
    pub tags: &'a [String],
    pub ocr_text: &'a str,
}

/// Result of rule execution: a list of actions to perform.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleExecutionResult {
    pub rule_name: String,
    pub actions: Vec<RuleAction>,
}

/// Evaluates a single condition against the context.
fn evaluate_condition(condition: &RuleCondition, ctx: &RuleContext) -> bool {
    match &condition.field {
        ConditionField::DocumentType => {
            let type_label = ctx.classification.doc_type.label().to_lowercase();
            match &condition.operator {
                ConditionOperator::Equals => type_label == condition.value.to_lowercase(),
                ConditionOperator::NotEquals => type_label != condition.value.to_lowercase(),
                ConditionOperator::Contains => type_label.contains(&condition.value.to_lowercase()),
                _ => false,
            }
        }
        ConditionField::Tag => {
            let value_lower = condition.value.to_lowercase();
            match &condition.operator {
                ConditionOperator::Equals => ctx.tags.iter().any(|t| t.to_lowercase() == value_lower),
                ConditionOperator::NotEquals => !ctx.tags.iter().any(|t| t.to_lowercase() == value_lower),
                ConditionOperator::Contains => ctx.tags.iter().any(|t| t.to_lowercase().contains(&value_lower)),
                _ => false,
            }
        }
        ConditionField::TextContains => {
            let text_lower = ctx.ocr_text.to_lowercase();
            match &condition.operator {
                ConditionOperator::Contains => text_lower.contains(&condition.value.to_lowercase()),
                ConditionOperator::Regex => {
                    Regex::new(&condition.value)
                        .map(|re| re.is_match(ctx.ocr_text))
                        .unwrap_or(false)
                }
                ConditionOperator::Equals => text_lower == condition.value.to_lowercase(),
                _ => false,
            }
        }
        ConditionField::AmountAbove | ConditionField::AmountBelow => {
            let threshold: f64 = condition.value.replace(',', ".").parse().unwrap_or(0.0);
            let amounts = ctx.extracted_data.fields.get("total")
                .or_else(|| ctx.extracted_data.fields.get("montants"));

            if let Some(amounts) = amounts {
                let parsed: Vec<f64> = amounts
                    .iter()
                    .filter_map(|a| a.replace(',', ".").replace(' ', "").parse::<f64>().ok())
                    .collect();

                if parsed.is_empty() {
                    return false;
                }

                match &condition.field {
                    ConditionField::AmountAbove => parsed.iter().cloned().fold(f64::NEG_INFINITY, f64::max) > threshold,
                    ConditionField::AmountBelow => parsed.iter().cloned().fold(f64::INFINITY, f64::min) < threshold,
                    _ => false,
                }
            } else {
                false
            }
        }
        ConditionField::HasField => {
            ctx.extracted_data.fields.contains_key(&condition.value)
        }
    }
}

/// Evaluates all rules against the given context and returns matching actions.
pub fn evaluate_rules(rules: &[AutomationRule], ctx: &RuleContext) -> Vec<RuleExecutionResult> {
    let mut results = Vec::new();

    for rule in rules {
        if !rule.enabled || rule.conditions.is_empty() {
            continue;
        }

        let matches = match rule.condition_logic {
            ConditionLogic::And => rule.conditions.iter().all(|c| evaluate_condition(c, ctx)),
            ConditionLogic::Or => rule.conditions.iter().any(|c| evaluate_condition(c, ctx)),
        };

        if matches {
            results.push(RuleExecutionResult {
                rule_name: rule.name.clone(),
                actions: rule.actions.clone(),
            });
        }
    }

    results
}

/// Returns the pre-compiled sensitive-data regex patterns for redaction.
/// Each entry is (label, &Regex).
pub fn get_sensitive_patterns() -> Vec<(&'static str, &'static Regex)> {
    vec![
        ("IBAN", &*RE_IBAN),
        ("Email", &*RE_EMAIL),
        ("Phone", &*RE_PHONE),
        ("SIRET", &*RE_SIRET),
    ]
}
