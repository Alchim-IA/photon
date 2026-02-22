<div align="center">

# Roadmap

### Photon — Fonctionnalites planifiees

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

## v0.5.0 — Cloud et partage

| Statut | Fonctionnalite | Description |
|:------:|:---------------|:------------|
| :clipboard: | **Envoi par email** | Envoyer directement un document numerise par email (SMTP ou client par defaut). |
| :clipboard: | **Integration cloud** | Upload vers Google Drive, OneDrive, Dropbox. OAuth2 integre. |
| :bulb: | **Partage reseau** | Enregistrement direct vers un partage SMB/NFS. |
| :bulb: | **QR Code / lien** | Generer un lien ou QR code temporaire pour partager un document. |
| :bulb: | **WebDAV / Nextcloud** | Synchronisation avec un serveur WebDAV pour les solutions auto-hebergees. |

<br/>

---

## v1.1.0 — Signature et securite

| Statut | Fonctionnalite | Description |
|:------:|:---------------|:------------|
| :clipboard: | **Signature electronique** | Apposer une signature manuscrite (dessin ou image) sur un document PDF. Pad de dessin integre + import image. |
| :clipboard: | **Filigrane (watermark)** | Ajouter un texte ou logo en filigrane sur les documents exportes. Position, opacite, rotation configurables. |
| :clipboard: | **Verification d'integrite** | Hash SHA-256 des documents exportes, affiche dans les metadonnees. Permet de verifier qu'un document n'a pas ete altere. |
| :bulb: | **Chiffrement AES-256** | Remplacer RC4-128 par AES-256 pour le chiffrement PDF, conformite PDF 2.0. |
| :bulb: | **Coffre-fort local** | Espace de stockage chiffre pour les documents sensibles, deverrouillage par mot de passe maitre. |

<br/>

---

## v1.2.0 — Productivite avancee

| Statut | Fonctionnalite | Description |
|:------:|:---------------|:------------|
| :clipboard: | **Comparaison de documents** | Comparer visuellement deux versions d'un meme document (diff visuel cote a cote avec surbrillance des differences). |
| :clipboard: | **Detection de langue** | Identification automatique de la langue du document pour optimiser l'OCR. Basee sur l'analyse de frequence des caracteres. |
| :clipboard: | **Formulaires PDF** | Remplissage de champs de formulaire PDF existants. Detection automatique des zones de saisie. |
| :bulb: | **Modeles de documents** | Definir des zones sur un type de document (ex: facture) pour extraire toujours les memes champs automatiquement. |
| :bulb: | **Historique de versions** | Conserver un historique des modifications d'un document avec possibilite de revenir a une version precedente. |

<br/>

---

## v1.3.0 — Integration et automatisation

| Statut | Fonctionnalite | Description |
|:------:|:---------------|:------------|
| :clipboard: | **Integration CLI** | Commande `photon scan --dpi 300 --output facture.pdf` pour scripting et automatisation. |
| :clipboard: | **API REST locale** | Serveur HTTP local pour piloter l'application depuis d'autres logiciels. Endpoints scan, export, OCR. |
| :clipboard: | **Systeme de plugins** | API pour etendre l'application (export vers ERP, integration metier, formats specifiques). |
| :bulb: | **Webhooks** | Notifications HTTP a chaque numerisation ou export. Integration avec Zapier, n8n, IFTTT. |
| :bulb: | **Dossiers surveilles (import)** | Surveiller un dossier pour importer automatiquement tout nouveau fichier depose et appliquer les regles d'automatisation. |

<br/>

---

## v1.4.0 — Multi-utilisateurs et collaboration

| Statut | Fonctionnalite | Description |
|:------:|:---------------|:------------|
| :bulb: | **Mode kiosque** | Interface simplifiee pour utilisation partagee (bureau d'accueil, bibliotheque). Ecran de selection utilisateur. |
| :bulb: | **Scanner a distance** | Piloter un scanner connecte a un autre poste sur le reseau local via un mode serveur/client. |
| :bulb: | **Profils utilisateurs** | Chaque utilisateur a ses propres parametres, profils de scan, regles et tags. |
| :bulb: | **Journal d'activite** | Tracer qui a numerise quoi et quand. Export CSV du journal. |

<br/>

---

## v2.0.0 — IA et analyse avancee

| Statut | Fonctionnalite | Description |
|:------:|:---------------|:------------|
| :bulb: | **OCR par IA (LLM)** | Utiliser un modele de vision (local ou API) pour extraire le texte de documents complexes (tableaux, manuscrits). |
| :bulb: | **Resume automatique** | Generer un resume d'une page pour les documents longs via LLM local. |
| :bulb: | **Traduction integree** | Traduire le texte OCR dans une autre langue directement dans l'application. |
| :bulb: | **Recherche semantique** | Rechercher par sens plutot que par mots-cles exacts grace a des embeddings locaux. |
| :bulb: | **Redaction automatique** | Detecter et masquer automatiquement les informations sensibles (numeros de secu, IBAN) avant partage. |
| :bulb: | **Extraction de tableaux** | Detecter et extraire les tableaux en CSV/Excel depuis les documents numerises. |

<br/>

---

## Idees supplementaires

Ces idees ne sont pas encore planifiees mais meritent d'etre explorees :

- **Export TIFF multi-pages** — Exporter un document multi-pages au format TIFF en plus du PDF
- **Support des codes-barres** — Detection et decodage automatique des codes-barres et QR codes dans les documents
- **Mode sombre adaptatif** — Ajuster automatiquement le contraste de la previsualisation selon l'heure de la journee
- **Raccourcis personnalisables** — Permettre a l'utilisateur de redefinir les raccourcis clavier
- **Statistiques de numerisation** — Dashboard avec nombre de pages numerisees, espace utilise, types de documents
- **Export vers Notion / Obsidian** — Integration directe avec les outils de prise de notes
- **Compression intelligente** — Reduire la taille des PDF en optimisant les images selon le contenu (texte vs photo)
- **Import depuis camera** — Utiliser la webcam ou un appareil photo connecte comme source de numerisation
- **Synchronisation multi-postes** — Synchroniser la configuration et l'historique entre plusieurs installations via un fichier partage

<br/>

---

<div align="center">

*Les priorites et le contenu de cette roadmap peuvent evoluer.
Les contributions et suggestions sont les bienvenues via les [Issues](https://github.com/votre-utilisateur/pdf-scanner/issues).*

<br/>
</div>
