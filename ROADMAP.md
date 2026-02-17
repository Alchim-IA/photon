<div align="center">

# Roadmap

### Scanner de Documents — Fonctionnalites planifiees

<br/>

Ce document presente les evolutions envisagees pour l'application, organisees par priorite et categorie.

<br/>

</div>

---

## Legende

| Statut | Signification |
|:------:|:--------------|
| :white_check_mark: | Implemente |
| :construction: | En cours |
| :clipboard: | Planifie |
| :bulb: | Idee / A explorer |

<br/>

---

## v0.1.0 — Fondations *(actuelle)*

| Statut | Fonctionnalite |
|:------:|:---------------|
| :white_check_mark: | Detection et connexion aux scanners (WIA / ICA / SANE) |
| :white_check_mark: | Numerisation avec configuration (DPI, couleur, format, duplex, ADF) |
| :white_check_mark: | Previsualisation des documents numerises |
| :white_check_mark: | Export PDF et image (PNG, JPEG) |
| :white_check_mark: | Historique des documents avec miniatures |
| :white_check_mark: | Persistance des parametres et de l'historique |
| :white_check_mark: | Interface glassmorphisme "Frosted Touch" |
| :white_check_mark: | Theme sombre / clair / auto |
| :white_check_mark: | Recadrage automatique |
| :white_check_mark: | Impression directe |

<br/>

---

## v0.2.0 — OCR et recherche *(implementee)*

| Statut | Fonctionnalite | Description |
|:------:|:---------------|:------------|
| :white_check_mark: | **OCR integre** | Reconnaissance de texte via Tesseract embarque (pas de dependance externe). Extraction automatique du texte apres numerisation. |
| :white_check_mark: | **PDF searchable** | Generation de PDF avec couche texte invisible (PDF/A). Le texte OCR est integre dans le fichier pour permettre la recherche. |
| :white_check_mark: | **Recherche full-text** | Barre de recherche dans l'historique. Recherche par contenu OCR, nom de fichier, date. |
| :white_check_mark: | **Copier le texte** | Selection et copie du texte extrait directement depuis la previsualisation. |
| :clipboard: | **Detection de langue** | Identification automatique de la langue du document pour optimiser l'OCR. |

<br/>

---

## v0.3.0 — Edition et traitement avance *(implementee)*

| Statut | Fonctionnalite | Description |
|:------:|:---------------|:------------|
| :white_check_mark: | **Rotation et retournement** | Rotation 90/180/270 et miroir horizontal/vertical dans la previsualisation. |
| :white_check_mark: | **Ajustements manuels** | Luminosite, contraste, saturation, nettete — curseurs en temps reel. |
| :white_check_mark: | **Debruitage** | Filtre de reduction du bruit pour les documents anciens ou de mauvaise qualite. |
| :white_check_mark: | **Redressement automatique** | Detection de l'inclinaison (deskew) et correction automatique de l'angle. |
| :white_check_mark: | **Suppression de fond** | Blanchiment du fond pour les documents numerises sur surface coloree. |
| :white_check_mark: | **Fusion de pages** | Combiner plusieurs numerisations en un seul document PDF multi-pages. |
| :white_check_mark: | **Reordonner les pages** | Glisser-deposer pour reorganiser les pages d'un document multi-pages. |

<br/>

---

## v0.4.0 — Productivite et workflow *(implementee)*

| Statut | Fonctionnalite | Description |
|:------:|:---------------|:------------|
| :white_check_mark: | **Profils de numerisation** | Sauvegarder des presets nommes (ex: "Factures 300dpi N&B", "Photos 1200dpi couleur"). Changement en un clic. |
| :white_check_mark: | **Numerisation par lot** | Mode batch : numeriser N pages automatiquement avec le chargeur ADF, nommage sequentiel. |
| :white_check_mark: | **Raccourcis clavier** | `Ctrl+S` numeriser, `Ctrl+Shift+S` PDF, `Ctrl+E` exporter, `Ctrl+P` imprimer, `Ctrl+O` OCR. |
| :white_check_mark: | **Nommage automatique** | Templates de noms de fichiers : `{date}_{time}_{counter}.pdf`. Configurable dans les parametres. |
| :white_check_mark: | **Dossier de surveillance** | Export automatique vers un dossier specifique a chaque numerisation. |
| :white_check_mark: | **Actions rapides** | Menu contextuel (clic droit) sur un document : renommer, dupliquer, ajouter aux pages, supprimer. |

<br/>

---

## v0.5.0 — Cloud et partage

| Statut | Fonctionnalite | Description |
|:------:|:---------------|:------------|
| :bulb: | **Envoi par email** | Envoyer directement un document numerise par email (SMTP ou client par defaut). |
| :bulb: | **Integration cloud** | Upload vers Google Drive, OneDrive, Dropbox. OAuth2 integre. |
| :bulb: | **Partage reseau** | Enregistrement direct vers un partage SMB/NFS. |
| :bulb: | **QR Code / lien** | Generer un lien ou QR code temporaire pour partager un document. |
| :bulb: | **WebDAV / Nextcloud** | Synchronisation avec un serveur WebDAV pour les solutions auto-hebergees. |

<br/>

---

## v0.6.0 — Intelligence et automatisation *(implementee)*

| Statut | Fonctionnalite | Description |
|:------:|:---------------|:------------|
| :white_check_mark: | **Classification automatique** | Detection du type de document (12 types : facture, carte d'identite, contrat, courrier, recu, formulaire, CV, ordonnance, releve bancaire, bulletin de paie, devis, autre) via heuristique avancee par mots-cles ponderes. |
| :white_check_mark: | **Extraction de donnees** | Extraction structuree des champs (montants, dates, IBAN, emails, telephones, SIRET/SIREN, numeros de document) via regex. |
| :white_check_mark: | **Tags et categories** | Systeme de tags manuels et automatiques avec definitions personnalisables (nom + couleur). Stockage separe dans tags.json. |
| :white_check_mark: | **Regles d'automatisation** | Moteur de regles avance avec conditions multiples (ET/OU), 6 types de conditions, 4 types d'actions (renommer, deplacer, ajouter tag, appliquer profil). |
| :white_check_mark: | **Suggestions intelligentes** | Suggestions de nom de fichier, dossier de destination et tags bases sur la classification et l'extraction de donnees. |

<br/>

---

## v1.0.0 — Polish et stabilite *(implementee)*

| Statut | Fonctionnalite | Description |
|:------:|:---------------|:------------|
| :white_check_mark: | **Internationalisation (i18n)** | Support bilingue francais / anglais. Contexte React custom avec fichiers JSON, interpolation `{{param}}`, persistance localStorage. |
| :white_check_mark: | **Accessibilite (a11y)** | Navigation clavier complete, roles ARIA (radio, tab, dialog, menu, status, progressbar), focus trap modales, skip link, focus-visible, `prefers-reduced-motion`, `prefers-contrast: more`. |
| :white_check_mark: | **Mise a jour automatique** | Verification et installation via `tauri-plugin-updater` + GitHub Releases. Toast de notification avec installation en un clic. |
| :white_check_mark: | **Onboarding** | Assistant 5 etapes (langue, scanner, dossier, test, fin) + tour guide 7 etapes avec spotlight overlay. Migration automatique des utilisateurs existants. |
| :bulb: | **Systeme de plugins** | API pour etendre l'application (export vers ERP, integration metier, formats specifiques). |
| :white_check_mark: | **Mode portable** | Detection `portable.marker` a cote de l'executable, configuration stockee dans `data/` adjacent. OnceLock pour thread-safety. |
| :white_check_mark: | **Journalisation** | `tauri-plugin-log` avec rotation 5 Mo et sortie stdout en debug. |

<br/>

---

## Idees supplementaires

Ces idees ne sont pas encore planifiees mais meritent d'etre explorees :

- **Signature electronique** — Apposer une signature manuscrite (dessin ou image) sur un document PDF
- **Filigrane (watermark)** — Ajouter un texte ou logo en filigrane sur les documents exportes
- **Comparaison de documents** — Comparer visuellement deux versions d'un meme document (diff visuel)
- **Mode kiosque** — Interface simplifiee pour utilisation partagee (bureau d'accueil, bibliotheque)
- **Scanner a distance** — Piloter un scanner connecte a un autre poste sur le reseau local
- **Archivage long terme (PDF/A)** — Export au format PDF/A pour archivage conforme
- **Chiffrement** — Protection par mot de passe des documents PDF exportes
- **Annotations** — Surligner, entourer, ajouter des notes sur les documents numerises
- **Integration CLI** — Commande `scanner-cli scan --dpi 300 --output facture.pdf` pour scripting et automatisation
- **API REST locale** — Serveur HTTP local pour piloter l'application depuis d'autres logiciels

<br/>

---

<div align="center">

*Les priorites et le contenu de cette roadmap peuvent evoluer.
Les contributions et suggestions sont les bienvenues via les [Issues](https://github.com/votre-utilisateur/pdf-scanner/issues).*

<br/>
</div>
