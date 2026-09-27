# UX pass — AI-assistance surfaces (Story 8.0)

**Status:** validated by Guy 2026-09-27 (« défauts partout »: every default of §9, Q1–Q16, adopted) — no code yet. **Origin:** G2 (PR #255, merged 2026-09-27):
PRD Phase 4 (FR13, FR17, FR33, FR64, FR68, FR69–FR78), architecture §Phase 4 (A1–A13), owner
decisions O1–O7 and D1–D11. **Placement:** Story 8.0, before any Epic 8 UI story (8.1, 8.5a, 8.5b,
8.6, 8.7, 8.8) — the same role the 7.0 pass (PR #206) played for Epic 7. `app` crate
(`app/ui` + `app/src/wiring`) and `report` (study PDF) only; the data model is 8.2a/8.2b.
**Wording:** §3 is the one French list the UI stories copy verbatim; every genuinely open choice
is in §9 with a default — **Guy decides the wording and the open questions**.
**Revision 2 (2026-09-27):** G3 review of the first draft applied (glyph, AI-text containment,
stale wording, frozen/current table, PDF encoding, chart cue, keyboard, bands, strings, record
structure, pinned-area order); Q12–Q15 added.
**Revision 3 (2026-09-27):** re-review applied — AI glyph « ★ » (bundled in Inter; « ✦ » is in no
bundled font), WinAnsi substitutes for « → » / « ≥ », AiFrame in the draft-study create form,
missing strings, « Verdict » row, stale confirmation names its target; Q16 added.

## 1. What has to fit

| New thing (Epic 8) | Why it needs a design before code |
|---|---|
| Drafts arrive from outside the app (MCP), while it is open or closed | Nothing in the UI announces an external event; it must be a *state* (seen, not modal). |
| AI text (comments, proposed notes, names, client/model) is third-party content | FR13/FR64: it must never read as an app signal — one frame, labelled, with the disclaimer. |
| A pending judgment draft may be a chart line or not | The charts are the signature surface (UX-DR10) and never auto-suggest (FR33); the proposal must be visibly *not* the owner's. |
| A validated AI value stays marked until the owner edits it (FR17) | The §3 cell already carries markers in two columns (G1 J / M4); a new one must pass the confusability gate (UX-DR15). |
| Study notes (FR78) | The study has ONE free-text field, « Justification de la décision » (`RationaleNote`); notes are a dated list, not a second free box. |
| « Valider l'étude » freezes the verdict (FR68, D11) | Two verdicts on one screen must not spend a second zone hue, and the difference must be seen. |

## 2. Goals / non-goals

**Goals**
1. One **Propositions** destination (nav rail): the pending drafts to act on, and the record of all
   drafts, with a pending count.
2. One **decision dialog** used from everywhere (inbox, study band, chart chips, judgment chips):
   current vs proposed side by side, the AI comment framed.
3. One **`AiFrame`** component — the only place AI-written text is rendered (structural test, §4.1).
4. One **AI glyph « ★ »** for every AI-related mark, ink only, used nowhere else in the app.
5. **Notes** as a titled card of dated entries under the rationale, entered through dialogs.
6. **« Valider l'étude »** in the study action row; the frozen verdict under the verdict bar, a
   side-by-side comparison only when it differs.

**Non-goals**
- No hue spent (UX spec « Monastic Colour Budget »): AI marks, the frozen verdict and the
  differences carry attention by glyph, placement, weight and ink — never a colour. The only
  coloured verdict on screen stays the *current* one.
- No toast/pop-up when a draft arrives (a draft is a *state*).
- No bulk action anywhere (FR74). No ghost preview of a pending value in the grid (§9 Q6).
- No change to the comparison screen (7.1) — see §7.

## 3. Vocabulary and the French wording list

### 3.1 Routing (the 7.0 rule, unchanged)

| Kind | Surface | Epic 8 examples |
|---|---|---|
| **Refusal** | modal notice « Action refusée » + « Compris » (or `field-error` inside an open form) | a decision on a read-only dossier; a write failure; target study deleted or archived since listing; a duplicate at draft-study validation; the target changed after the stale confirmation |
| **State** | `StatusBand` at the top of the card / pinned area it concerns | pending drafts on this study; pending draft studies; drafts unreadable; frozen verdict differs; stale / target gone inside the decision dialog |
| **Outcome** | inline notice (F4 slot) | « Proposition validée. », « Étude validée ; verdict figé le 27/09. » |
| **Confirmation** | modal confirm | delete a note; validate a stale draft (O4); replace an existing frozen verdict |

### 3.2 Terms (defaults — alternatives in §9)

| Concept | French (UI) |
|---|---|
| AI (label, frame, glyph caption) | **IA** |
| a draft | **proposition** (« proposition de l'IA ») |
| the destination | **Propositions** |
| the view of pending drafts to act on | **À traiter** |
| the record of all drafts | **Registre** |
| pending | **en attente** |
| stale | **périmée** |
| target gone | **cible disparue** |
| validated / rejected | **validée** / **rejetée** |
| validated then undone | **validée puis annulée** |
| edited before validation | **modifiée avant validation** |
| AI origin (inside AiFrame only) | « Proposée par {client} ({modèle}) le {JJ/MM/AAAA} » |
| after validation (app text, no AI string) | « proposée par l'IA, validée le {JJ/MM/AAAA} » |
| judgment after validation (app text) | « placée par l'IA · validée le {JJ/MM/AAAA} » |
| frozen / current verdict (FR68 labels) | « figé ({méthode}, {JJ/MM}) » / « actuel ({méthode}, aujourd'hui) » / « validée le {JJ/MM} » |

### 3.3 Strings (copied verbatim by the stories)

Placeholders `{…}` are filled by the app. A placeholder filled with **AI-written
text** is marked `{IA:…}` and may only appear inside an `AiFrame` (§4.1).

**Nav rail (8.5a)**
- Rail label: « Propositions » — « Propositions · {n} » while n > 0 pending; « Propositions · ⊘ »
  when the last poll or read failed (§5.1). The top-bar title stays « Propositions » (no count).

**Propositions screen (8.5a, 8.7)**
- Card title: « Propositions ». View chips: « À traiter » · « Registre ».
- Subtitle: « Proposées par une IA via le serveur MCP ; aucune n'est appliquée sans votre validation. »
- Kind chips (both views): « Toutes » · « Valeurs » · « Jugements » · « Notes » · « Études ».
- Registre filters: « Étude : » (drop-down, « Toutes les études ») · outcome chips « En attente » ·
  « Validées » · « Validées puis annulées » · « Rejetées » (the view chip is « À traiter », never
  « En attente », so the two cannot be confused).
- Empty (À traiter): « Aucune proposition à traiter. Une IA enregistrée comme client MCP peut en déposer ici ; rien n'est appliqué sans votre validation. »
- Empty (Registre): « Aucune proposition dans ce dossier. »
- Unreadable (band ⊘): « Les propositions n'ont pas pu être lues ; la liste est indisponible ({cause}). »
- Read-only (band ◦): « Dossier en lecture seule : les propositions sont consultables, aucune décision n'est possible. »
- Row targets (app text): « {TICKER} · {champ} · {année} » (cell) · « {TICKER} · {champ} » (judgment) · « {TICKER} · nouvelle note » · « Nouvelle étude : {TICKER} ({DEV}) ».
- Row parts (app text): group header « {TICKER} ({DEV}) » · draft-study group « Nouvelles études » · « Actuel {x} → Proposé {y} » · for a note « Nouvelle note » · row button « Détail » / « Masquer le détail » (Registre) · record flag « périmée à la décision ».
- Draft-study row (8.5a): « Nouvelle étude : {TICKER} ({DEV}) » then, inside the compact `AiFrame`, the proposed company name and the comment: « {IA:nom} — {IA:commentaire} » (no name: the comment alone).
- Draft-study row until 8.7 ships (8.5a): « La validation d'une proposition d'étude n'est pas encore disponible ; la proposition reste en attente. »
- Draft-study row button (8.7): « Valider… » (opens the prefilled create dialog directly — Q14).
- Create form opened from a draft study (8.7): sentence « Étude proposée par une IA ; vérifiez le symbole, la devise et le nom avant de créer. » — the form-id `study-create` gains a draft variant (§4.2).

**Decision dialog (8.5b, 8.6, 8.7)**
- Title: « Proposition de l'IA ». Columns: « Actuel » · « Proposé ».
- A note draft always creates a NEW note: « Actuel » shows « — »; « Proposé » shows the note text inside an `AiFrame`.
- Buttons: « Valider » · « Rejeter » · « Modifier avant de valider… » · « Annuler ».
- Edit form: title « Modifier la proposition » · sentence « La valeur enregistrée sera la vôtre ; la proposition sera notée modifiée avant validation. » · button « Enregistrer ».
- Other study (context line): « Valider ou rejeter ouvre l'étude {TICKER} ({DEV}). »
- Stale band (◦): « La cible {cible} a changé depuis la proposition. Valeur actuelle : {maintenant}. »
- Stale confirm: title « Valider une proposition périmée ? » · body « La cible {cible} a changé depuis la proposition (valeur actuelle : {maintenant}). La valeur proposée la remplacera. » · verb « Valider ».
- Target gone band (⊘): « Cible disparue : {raison} ; la proposition ne peut qu'être rejetée. » (raisons : « l'année {AAAA} n'existe plus dans l'étude » · « l'étude a été supprimée »).
- Changed after confirmation (refusal): « La cible a encore changé depuis votre confirmation ; rien n'a été enregistré. La proposition est affichée de nouveau. »
- Draft study, duplicate at validation (refusal): « Une étude {TICKER} en {DEV} existe déjà ; rien n'a été créé. »
- Refusals: « Le dossier est en lecture seule ; aucune décision n'a été enregistrée. » · « L'étude {TICKER} n'existe plus ; aucune décision n'a été enregistrée. » · « L'étude {TICKER} est archivée ; aucune décision n'a été enregistrée. » · « La décision n'a pas pu être enregistrée ({cause}) ; rien n'a été modifié. »
- Outcomes: « Proposition validée. » · « Proposition rejetée. » · « Proposition validée (modifiée avant validation). » · undo: « Validation annulée ; la proposition est notée validée puis annulée. » · redo: « Validation rétablie. »

**AiFrame (everywhere AI text shows)**
- Header: « ★ IA — Proposée par {IA:client} ({IA:modèle}) le {JJ/MM/AAAA} ».
- Disclaimer (always, last line): « Texte rédigé par une IA, non vérifié — ne constitue pas un conseil financier. »

**Bands (8.5a, 8.8)** — each band's button uses the new StatusBand action slot (§4.5)
- Study reminder (★): « {n} proposition(s) de l'IA en attente sur cette étude. » + « Voir les propositions ».
- Study reminder, read failed (⊘): « Les propositions de cette étude n'ont pas pu être lues ; état indisponible. » + « Voir les propositions ».
- Études card (★): « {n} proposition(s) d'étude de l'IA en attente. » + « Voir les propositions » (tickers are listed on the Propositions screen, not in the band).
- Frozen verdict differs (•): see 8.8 below.

**Cell and judgment marks (8.5b, 8.6)**
- Cell trace line (traceability overlay « Entrées & provenance »): « {valeur} — proposée par l'IA, validée le {JJ/MM/AAAA} » (client/model shown only in the AiFrame of the history Détail).
- Pending-judgment action chip (§5.5): « ★ IA · {champ} : {valeur} ».
- Chart endpoint label (pending AI line): « IA {valeur} ».
- Chart action-chip row label: « Propositions de l'IA : ».
- Études list row marker (study with pending drafts): « ★ {n} ».
- After validation (caption under the field / at the line end): « placée par l'IA · validée le {JJ/MM/AAAA} ».

**Notes (8.1)**
- Card: « Notes » · button « Ajouter une note… » · row buttons « Modifier… » · « Supprimer ».
- Empty: « Aucune note pour cette étude. »
- Form: title « Ajouter une note » / « Modifier la note » · sentence « Une note reste dans l'historique de l'étude, même supprimée. Ctrl+Entrée enregistre. » · field « Note » · button « Enregistrer ».
- Delete confirm: title « Supprimer la note ? » · body « La note quitte l'étude ; elle reste lisible dans l'historique. » · verb « Supprimer ».
- Refusal: « La note est vide ; rien n'a été enregistré. »
- Row meta: « {JJ/MM/AAAA} » + « · modifiée le {JJ/MM/AAAA} » when edited.
- Long note: « Afficher tout » / « Réduire ».

**History (8.1, 8.7)**
- Toggle chip: « Masquer les notes » / « Afficher les notes ».
- Summaries (app text only): « Note ajoutée » · « Note modifiée » · « Note supprimée » · « ★ Proposition validée : {cible} » · « ★ Proposition validée (modifiée) : {cible} » · « ★ Proposition rejetée : {cible} » · « ★ Proposition validée puis annulée : {cible} » · « Étude validée ; verdict figé ».

**« Valider l'étude » and the frozen verdict (8.8)** — see Q12 for the enabled/disabled choice
- Button: « Valider l'étude ».
- Reason beside the disabled button (Q12 default): « Verdict incomplet — entrées ouvertes : {liste} ».
- Refusal (Q12 alternative): « L'étude ne peut pas être validée : le verdict n'est pas complet (entrées ouvertes : {liste}) ; rien n'a été enregistré. »
- Replace confirm: title « Remplacer le verdict figé ? » · body « Le verdict figé le {JJ/MM} sera remplacé ; il reste lisible dans l'historique. » · verb « Valider l'étude ».
- Outcome: « Étude validée ; verdict figé le {JJ/MM}. » · undo (Ctrl+Z): « Validation de l'étude annulée. »
- Strip (same): « Étude validée le {JJ/MM} ; le verdict actuel est identique au verdict figé. »
- Band (differs, •): « Le verdict actuel diffère du verdict figé le {JJ/MM}. » + « Voir la comparaison » / « Masquer la comparaison ».
- Table columns: « figé ({méthode}, {JJ/MM}) » · « actuel ({méthode}, aujourd'hui) » — « actuel (…, provisoire) » / « actuel (…, retenu) » when the current verdict is not full.
- Table rows: « Verdict » · « Zone du prix » · « Ratio hausse/baisse » · « Valeur relative » · « Appréciation projetée » · « Potentiel à 5 ans » · « Entrées » · « Méthode ».
- Withheld cell: « retenu — entrées ouvertes : {liste} ».
- Entrées row: « identiques » or « {n} modifiée(s) : {champ} {figé} → {actuel}, … ».
- Cause line: « Cause : {causes} » with « rafraîchissement du {JJ/MM} » · « modification de votre part » · « proposition de l'IA validée » · « changement de méthode ({vA} → {vB}) » · « cause inconnue ».

**MCP refusal reasons (8.3, 8.4) — decided here (Q11)**
A stable English `snake_case` code (never shown in the UI) + a French message:
- `dossier_mismatch` — « Le dossier a changé depuis la lecture ({chemin lu} ≠ {chemin actuel}) ; rien n'a été enregistré. »
- `dossier_replaced` (A11) — « Le fichier du dossier a été remplacé pendant l'écriture (restauration) ; rien n'a été enregistré. »
- `schema_mismatch` — « Le dossier est au schéma v{a}, ce serveur MCP en v{b} ; ouvrez-le d'abord dans l'application. » / « … ce serveur MCP est plus ancien que le dossier. »
- `no_dossier` — « Aucun dossier n'a pu être déterminé ; rien n'a été lu. »
- `study_not_found` — « L'étude {id} n'existe pas dans ce dossier ; rien n'a été enregistré. »
- `empty_comment` — « Le commentaire est obligatoire ; rien n'a été enregistré. »
- `missing_origin` — « Le client et le modèle sont obligatoires ; rien n'a été enregistré. »
- `field_not_draftable` — « Le champ {champ} ne peut pas être proposé (voir la liste des champs du schéma de l'outil) ; rien n'a été enregistré. »
- `year_not_in_study` — « L'année {AAAA} n'existe pas dans l'étude {TICKER} ; une proposition n'ajoute jamais d'année. »
- `value_unparsable` — « La valeur « {texte} » n'est pas un nombre dans l'unité de {champ} ({unité}) ; rien n'a été enregistré. »
- `value_not_an_option` — « La valeur « {texte} » n'est pas une option de {champ} ({options}) ; rien n'a été enregistré. »
- `identifier_invalid` — « Le symbole ou la devise proposés ne sont pas valides ({règle}) ; rien n'a été enregistré. »
- `target_has_pending` — « {cible} a déjà une proposition en attente ; rien n'a été enregistré. »
- `study_exists` — « L'étude {TICKER} en {DEV} existe déjà ; rien n'a été enregistré. »
- `draft_study_pending` — « Une proposition d'étude {TICKER} en {DEV} est déjà en attente ; rien n'a été enregistré. »
- `write_denied` — « Écriture refusée : seule la création de propositions est permise. »
- *Added by Story 8.3 (G3 review, 2026-09-28):*
- `study_archived` — « L'étude {TICKER} est archivée ; la proposition n'a pas été enregistrée. »
- `value_out_of_range` — « La valeur « {texte} » de {champ} est hors des bornes d'une proposition (moins de 10¹⁵ en valeur absolue, au plus 10 décimales) ; rien n'a été enregistré. »
- `empty_note_text` — « Le texte de la note est vide ; rien n'a été enregistré. »
- `text_too_long` — « Le texte {champ} dépasse {max} caractères ({n}) ; rien n'a été enregistré. » ({champ} : le commentaire / le texte de la note / le nom de la société / le client / le modèle)
- `draft_id_conflict` — « Une autre proposition porte déjà l'identifiant {id} ; rien n'a été enregistré. » (the same proposition sent again is accepted once, without a second write)
- `dossier_busy` — « Une restauration du dossier est en cours (ou a été interrompue) ; rien n'a été lu ni enregistré. »
- `dossier_needs_recovery` — « Le dossier doit d'abord être ouvert dans l'application (reprise après une interruption) ; rien n'a été lu. »
- `dossier_protected` — « Le fichier du dossier est protégé en écriture » / « Le dossier qui contient le fichier est protégé en écriture » + « ; il ne peut pas être lu sans y créer de fichiers, ou rien ne peut y être enregistré. »
- `not_a_dossier` — « Le fichier {chemin} n'est pas un dossier SteadyInvest ; rien n'a été lu. »
- `dossier_identity_unreadable` — « L'identité du fichier du dossier n'a pas pu être lue ; rien n'a été enregistré. »
- `invalid_call` — « L'appel au serveur MCP est mal formé ({détail}) ; rien n'a été enregistré. » (a server defect, logged — never an AI proposal's fault)

## 4. Shared components (`app/ui/components/`)

### 4.1 `AiFrame` — the only home of AI-written text
A `Rectangle` in `surface-alt` with a `separator` border and a **3 px left rule in `text-mid`**
(its non-colour signature, distinct from `PanelCard` and `StatusBand`). Rows: header (§3.3, caption,
`text-mid`, semibold « IA »), the AI text (body, `text-high`, word-wrap, plain text, never
posture-scanned), then `@children` (for a prefilled edit field, §4.2), then the disclaimer (caption,
`text-low`). Properties: `origin-client`, `origin-model`, `submitted`, `text`, `compact` (list rows:
header + one elided text line; the disclaimer stays).

**AI-written strings** (from the draft, never produced by the app): the comment, the proposed note
text, the proposed company name, `client`, `model`. **Structural test (A12):** these reach Slint only
through properties whose names start with `ai-`; a scan of `app/ui/**/*.slint` fails if an `ai-*`
property is read outside `ai_frame.slint`, or if a model struct field carrying one of them is bound
elsewhere. Covered places: inbox and record rows (compact), the decision dialog (comment; note text
in « Proposé »; company name in the draft-study context), the edit form (§4.2), notes with an AI
origin (§5.6), history Détail of AI items (§5.7).

**Exemptions (listed in the test, each justified):**
- *Proposed values* (numbers, enum options) — parsed and validated at submission (A3), then
  formatted by the app's own number code; what is shown is app output, not AI text.
- *Ticker and currency of a draft study* — shown outside the frame in row targets and bands; they
  pass the `identifier_invalid` check at submission (ticker `[A-Z0-9.\-]{1,20}`, currency ISO 4217
  three letters — **8.3 must add this check**; it is the condition of the exemption).
- *The prefilled edit field* — it lives inside the AiFrame as `@children`; once the owner edits and
  saves, the text is the owner's (FR74).

### 4.2 The decision dialog — a new `ModalDialog` kind `"decision"`
`Dialog.kind == "decision"` (beside notice / confirm / form), mounted in the same overlay (scrim,
centred card, focus trap, Esc cancels). Layout: title « Proposition de l'IA »; context line (study,
target, « Valider ou rejeter ouvre l'étude … » when needed); state band (périmée / cible disparue /
lecture seule); a two-column grid « Actuel » | « Proposé » (numeric font; the proposed side in
semibold; for a note, « — » | the text inside an AiFrame); the `AiFrame` with the comment; buttons.
« Modifier avant de valider… » switches to the form variant `form-id = "draft-edit"`: the AiFrame
keeps the comment, the « Proposé » cell becomes a `LabeledField` (value) or a `LabeledTextArea`
(note, as `@children` of the AiFrame), prefilled. Initial focus: see Q5 (default « Annuler » when
the target is a ✓ value, « Valider » otherwise).

**Draft-study variant of the create form (8.7).** « Valider… » on a draft-study row opens the
ordinary `study-create` form (`modal_dialog.slint`) prefilled with ticker, currency and name, with
the draft sentence (§3.3). In this variant the **name field and the AI comment are wrapped in an
`AiFrame`** (`@children`): the proposed name is AI-written until the owner confirms « Créer », at
which point it becomes the owner's entry (FR74). Ticker and currency fields stay plain (exempt,
§4.1). The duplicate check runs on « Créer ».

### 4.3 `LabeledTextArea` (in `modal_dialog.slint`)
A multi-line field for notes, the `RationaleNote` box behaviour (3–8 rows, scroll beyond). **Enter
inserts a newline; Ctrl+Enter submits** (the form's validating key); Esc cancels.

### 4.4 The AI glyph « ★ »
U+2605 (black star), text presentation, `text-high`, as `Tokens.ai-glyph` next to
`gap-glyph` / `warn-glyph`. **Not used anywhere in `app/ui` today** (inventory 2026-09-27: « ◆ » is
the zone/stop alert glyph on Liste de suivi and Portefeuille, « ◦ » the stale/absent dot, « ⊘ »
indisponible, « ⚠ » murmur/refusal, « △ » plausibility, « ▦ » gap, « ⦸ » lock, « ✓ » / « ? »
review). **Font coverage checked with fontTools (2026-09-27):** « ★ » is in Inter Regular and
SemiBold (the UI font), so it renders from the bundled font, no system fallback. The four-pointed
« ✦ » (U+2726) of revision 2 is in none of the bundled fonts (Inter, IBM Plex Sans) and was
dropped. A star can read as a rating; next to a figure it must never suggest « good » — hence
Q16.

### 4.5 `StatusBand` — optional action slot (component change, 8.5a; reused by 8.8)
Today `StatusBand` has `text` + `icon` only (`status_band.slint:9-13`). 8.5a adds `in property
<string> action-label` (empty = no button) and `callback action()`, rendering a trailing
`ActionButton` (focusable, Tab order after the band text). All existing bands are unchanged (empty
label).

### 4.6 `FrozenVerdictStrip` (8.8)
Pinned under the verdict bar (§5.9). Two forms: the one-line caption when frozen = current; a
`StatusBand` (glyph « • », action « Voir la comparaison ») when they differ, expandable to the
comparison table. Not rendered when the study has no frozen verdict.

## 5. Screen by screen

### 5.1 Nav rail — the « Propositions » destination (8.5a)
A sixth destination between « Revue » and « Réglages ». The rail shows a **separate label
property** (« Propositions · {n} », « Propositions · ⊘ » on a failed poll or read — never vanishing,
the last good count is not kept), while the top bar keeps reading `destinations[current-screen]`
(`app.slint:87`), so the title is plain « Propositions ». No badge, no hue. **Re-index:** in
`app.slint` the screen `if` block (line 151) gains `current-screen == 4` for Propositions and
Réglages moves to 5; in `wiring/mod.rs` the `screen-activated` match gets a new arm `4` (re-read the
drafts), and 5 falls through the existing `_ => {}` like Réglages today.

### 5.2 The Propositions screen (8.5a « À traiter », 8.7 « Registre »)
**One `PanelCard` « Propositions », two views switched by a chip:**
- **« À traiter »** — the pending drafts, the actionable view. Kind chips, then screen bands
  (unreadable ⊘ / read-only ◦), then rows grouped by study (group header « {TICKER} ({DEV}) »,
  draft studies last under « Nouvelles études »), newest first. Row: target (app text), « Actuel {x}
  → Proposé {y} » (for a note: « Nouvelle note »), the state word (« périmée », « cible disparue »),
  the date, and the compact `AiFrame` (comment, one line; the disclaimer stays). A draft-study row
  shows « Nouvelle étude : {TICKER} ({DEV}) » — ticker and currency as app text (exempt, §4.1) —
  and the proposed company name before the comment, inside the compact `AiFrame`. Click or
  Enter/Space opens the decision dialog. Draft-study rows carry « Valider… » (Q14) and, until 8.7,
  the « pas encore disponible » line instead.
- **« Registre »** — **all** drafts, pending included, read-only. Filters: « Étude : » + outcome
  chips (« En attente · Validées · Validées puis annulées · Rejetées »). Rows: submitted date,
  target, outcome word (+ « périmée à la décision », « modifiée avant validation »), decided date;
  « Détail » expands to the comment and proposed value/text in an `AiFrame`.
States: empty texts §3.3; no « loading » state (a local read); a failed read shows the ⊘ band and
the rail « · ⊘ » — never an empty list. Keyboard: Tab through view chips → kind/outcome chips →
rows; Enter/Space opens; « Détail » toggles.

### 5.3 Pinned area of the study screen — order (8.5a, 8.8)
Top to bottom, outside the scroll (the G1 J reason: seen wherever the form is scrolled):
1. verdict bar; 2. `FrozenVerdictStrip` (8.8); 3. the ★ reminder band (8.5a); 4. the notice slot.
The expanded comparison (§5.9) is **capped at 40 % of the study area's height** and scrolls
internally (its own `Flickable`); « Masquer la comparaison » collapses it; it starts collapsed on
every open.

### 5.4 Study reminder and Études signal (8.5a)
- Study: the ★ band « {n} proposition(s) … » + « Voir les propositions » (opens Propositions,
  « À traiter », filtered on this study). On a failed poll: the ⊘ variant (§3.3), not removed.
  Absent when n = 0 and the last read succeeded.
- Études list card: the ★ band « {n} proposition(s) d'étude … » at the top of the card while draft
  studies are pending; list rows of studies with pending drafts carry « ★ {n} » after the name.

### 5.5 Pending judgment drafts on the study (8.6) — see Q13
- **Chart lines (default of Q13: every judgment the owner can drag on a chart gets an AI line
  there):** §1 growth chart — estimated high EPS and estimated low EPS (and a projected EPS-growth
  draft, drawn as the est-high line it implies); §3 P/E chart (#115) — judged high P/E and judged
  low P/E. The AI line is **dotted** (round dots, `chart-stroke-thin`, ink `text-mid` — on §1 the
  owner's lines and seeds are solid (thin / dimmed), and on §3 the judged-low P/E level is already
  dashed (`pe_history_chart.slint`), so dots are the one free pattern on both charts), ends in a **hollow circle** endpoint
  marker, carries the label « IA {valeur} », and has **no drag handle** (inert). Three non-colour
  cues: dot pattern + hollow marker + label.
- **Action chips:** under each chart's legend, a row « Propositions de l'IA : » with **one chip
  per pending field**, naming it — « ★ IA · BPA estimé haut : 12,40 », « ★ IA · BPA estimé bas :
  9,10 » — rendered as `ActionButton`s (focusable `FocusScope`, neutral ink), **outside** the
  « Ajuster : » selector and **not** a series-hued `ContextChip`. Enter/click opens the decision
  dialog.
- **Other judgments** (projected sales growth, severe low, current dividend…): the same
  « ★ IA · {champ} : {valeur} » action chip under the `JudgmentField`, after any « hist. » chip.
  `forecast_low_option` (a `ChoiceChip` group): the action chip names the proposed option by its
  label, « ★ IA · Option du bas prévisionnel : {libellé} », placed after the group; the group itself
  is untouched while pending.
- **After validation:** the value is the owner's; the caption « placée par l'IA · validée le … »
  sits at the line end (chart) or under the field, `text-low`, until any write to that field (A6).
  Zone bar and verdict bar are identical with and without pending drafts (FR72).

### 5.6 Notes (8.1)
A `PanelCard` « Notes » directly below « Justification de la décision » (`RationaleNote` unchanged).
« Ajouter une note… » → form `note-add` (`LabeledTextArea`, Ctrl+Enter submits). Rows, newest
first: date meta, the text (3 lines, then « Afficher tout »), « Modifier… » (form `note-edit`),
« Supprimer » (confirm). A note carrying an AI origin renders its text inside an `AiFrame` plus the
app caption « proposée par l'IA, validée le … »; an owner edit clears the AI origin. Undo/redo cover
add/edit/delete (Ctrl+Z / Ctrl+Y).

### 5.7 History panel (8.1, 8.7)
The « Historique de l'étude » panel gains the chip « Masquer les notes » (default: shown). Note-only
entries (A12) summarise as « Note ajoutée / modifiée / supprimée ». Processed drafts are merged at
`decided_at` with ★ summaries (app text only), rejected ones included. Their « Détail » shows the
AI-written parts (comment, proposed note text, before/after of an AI note) inside an `AiFrame`;
app-formatted value changes (« {avant} → {après} » of a number) stay plain history lines.

### 5.8 The study grid — the AI-origin cell mark (8.5b)
A validated AI value (Source::Manual, `?`, `ai_origin` set) shows « ★ » in the **trailing column,
bottom half** — the slot of the stale « ◦ ». They should not coincide (a validated AI value is a
manual `Current` value); 8.5b verifies it against `contract/src/cell.rs`, else « ★ » takes the
leading-bottom slot when no lock shows. Cleared by the owner's next edit (A6). **Confusability gate
(UX-DR15) entry:** « ★ » joins { ?, ✓, ⦸, △, ◦, ▦, n/a, ⋯ } at 14 px; the pairs measured are
**★ vs ◦** (same slot) and **★ vs ◆** (the alert glyph elsewhere in the app), plus ★ vs △ (same
column); 8.5b's visual verification shows them side by side at `mark-font`.

### 5.9 « Valider l'étude » and the frozen verdict (8.8) — see Q12
- **Button:** in the study action row, after « Historique ». Q12 default: **disabled while the
  verdict is not full, with the reason shown beside it** (« Verdict incomplet — entrées ouvertes :
  … », caption, `text-mid`), as FR68 / Story 8.8 / the 8.0 AC say. With an existing frozen verdict
  it asks the replace confirmation. The freeze is **undoable (Ctrl+Z)** in the session.
- **Strip (§4.6):** equal → the caption line; different → the « • » band + « Voir la comparaison ».
- **Comparison table:** columns « figé (…) » | « actuel (…) »; when the current verdict is
  provisional the header says « actuel (…, provisoire) » and its figures show in `text-mid`; when
  withheld the column reads « retenu — entrées ouvertes : … » in every row. Rows: « Verdict »
  (`VerdictFacts.quality_value_candidate` — the verdict word the app already shows), « Zone du prix »
  (`VerdictFacts.present_price_zone`, written with the runtime `Labels.zone-buy / zone-hold /
  zone-sell`, **in ink**, never a zone hue — the current verdict keeps the only coloured badge in
  the verdict bar), « Ratio hausse/baisse » (value + the ≥ 3 criterion), « Valeur relative » (the
  < 100 % criterion), « Appréciation projetée » (the ≥ doubling criterion), « Potentiel à 5 ans »,
  « Entrées » (« identiques » or the changed load-bearing inputs, frozen → current — an
  inputs-only difference therefore always shows a row), « Méthode ». A changed row is marked by a
  leading « • » and semibold figures (two non-colour channels); unchanged rows in `text-mid`. Then
  the cause line.

## 6. Keyboard operation (NFR-U2)
Nav: « Propositions » is a `NavItem` (Tab, Enter/Space). Propositions screen: view chips → filter
chips → rows (Tab), Enter/Space opens a row, « Valider… » on a draft-study row, « Détail » toggles.
Decision dialog: focus trapped, initial focus per Q5, Esc cancels, Enter activates the focused
button; the edit form validates with Enter (value) or Ctrl+Enter (note). Study: the band action
buttons (§4.5), the chart action chips and the judgment action chips (`ActionButton`s), the notes
buttons, « Valider l'étude », « Voir la comparaison » — all Tab-reachable with the visible focus
ring. Ctrl+Z / Ctrl+Y undo/redo a validation (and a freeze) while the study stays open (A8).

## 7. Report impact (decided here)
The study PDF writes Helvetica with **WinAnsi encoding only** (`report/src/pdf.rs`,
`winansi_byte`): « ★ », « ≠ », « → » and « ≥ » are outside it (`winansi("→")` gives « ? », a
tested fact in `pdf.rs`), so the PDF uses WinAnsi substitutes: « † » (0x86) for the AI mark, « • »
(0x95) for a changed row, « -> » for « → » (Entrées row « {champ} {figé} -> {actuel} », history
and cause line), « >= » for « ≥ » (criteria « ratio >= 3 », « appréciation >= doublement »).
- **Notes: not printed** (unchanged, like the rationale; the PDF is the NAIC form — tiebreaker).
- **AI-origin values and AI-placed judgments: marked « † »** (0x86) after the figure, with one
  legend line under the table: « † valeur proposée par une IA et validée par l'utilisateur ». A
  judged value that is also AI-placed prints « * † ». A report must not launder an AI value.
- **Frozen verdict (8.8):** a block after the verdict: « Verdict figé le {JJ/MM/AAAA} ({méthode}) »
  with its figures and, when it differs from the current one, the comparison table in greyscale,
  changed rows marked « • » (0x95), and the cause line.
- **Comparison (7.1) and review (7.2) — screens and PDFs unchanged, by decision** (Q9).

## 8. Acceptance criteria

**Given** pending drafts **When** the app renders **Then** the rail reads « Propositions · n » (the
top-bar title does not), each concerned study shows the ★ band, pending draft studies show the ★
band on the Études card, and no dialog opens by itself. **Given** a failed poll **Then** the rail
reads « · ⊘ » and the bands switch to their ⊘ variant — nothing vanishes.

**Given** a pending value or note **When** its row is opened and « Valider » pressed **Then** it
took 2 actions; **Given** a draft study **When** « Valider… » then « Créer » **Then** 2 actions
(Q14).

**Given** any AI-written string **When** rendered **Then** it is inside `AiFrame` with the
disclaimer, save the listed exemptions — enforced by the structural test.

**Given** a stale draft **When** « Valider » is pressed **Then** the confirm shows the current value
without any stored base value; a target-gone draft offers only « Rejeter ».

**Given** a pending draft on a draggable chart judgment **When** the chart renders **Then** a dotted,
thin, handle-less line with a hollow endpoint and « IA {valeur} » shows, one action chip per pending
field names it, and the zone bar and verdict are identical with and without it.

**Given** a validated AI value in a §3 cell **When** rendered **Then** « ★ » shows in the trailing
bottom slot, passes the gate against ◦, ◆ and △ at 14 px, and disappears on the next owner edit.

**Given** a study **When** a note is added, edited, deleted **Then** each goes through a titled
dialog (Ctrl+Enter submits, delete confirmed), and history can hide note-only entries.

**Given** a frozen verdict that differs from the current one **Then** the • band shows under the
verdict bar (above the ★ band and the notice), the capped comparison marks each changed row with •
+ semibold, lists changed inputs under « Entrées », writes zones with the runtime labels in ink, and
names the cause.

**Given** the study PDF **Then** notes are absent, AI-origin figures carry « † » with the legend, a
frozen verdict prints with its comparison (• markers) when it differs; the comparison PDF is unchanged.

**Given** the keyboard only **Then** every surface above is reachable and operable (§6).

## 9. Open questions — decided 2026-09-27: every default below is adopted (Guy: « défauts partout »)
- **Q1** « IA » or « AI »? **Default: « IA »** (French UI, UX-DR29). Alt: « AI ».
- **Q2** « proposition » or « brouillon »? **Default: « proposition »** (from the owner's side it is
  a proposal; « brouillon » reads as the owner's own unfinished work). Alt: « brouillon ».
- **Q3** « périmée » for stale? **Default: « périmée »**. Alt: « dépassée » or « obsolète ».
- **Q4** « validée puis annulée »? **Default: yes**. Alt: « annulée après validation ».
- **Q5** Initial focus in the decision dialog? **Default: « Annuler » when the target is a ✓ value
  (O5 moves it to ?), « Valider » otherwise.** Alt: « Annuler » always (safer, one more keystroke).
- **Q6** Ghost preview of a pending value in the grid? **Default: no.**
- **Q7** A sixth nav destination, or a badge on « Études »? **Default: sixth destination.**
- **Q8** Frozen/current labels: **Default: FR68's « figé (vNN, JJ/MM) » / « actuel (vMM,
  aujourd'hui) » / « validée le JJ/MM »** (adopted in §3.3). Alt: « Au moment de la décision » /
  « Aujourd'hui ».
- **Q9** AI-origin marks in the comparison (7.1)? **Default: no** (unchanged by decision).
- **Q10** Notes in the study PDF? **Default: no.** Alt: a « Notes » page appended.
- **Q11** MCP messages in French, codes in English? **Default: yes.** Alt: English messages.
- **Q12** « Valider l'étude » on a non-full verdict: **disabled with the reason shown beside it**
  (as FR68, Story 8.8 and the 8.0 AC say; the app already disables buttons, e.g. Achat before its
  fields) **or** always enabled + « Action refusée » (reason in the dialog, « ; rien n'a été
  enregistré. »). **Default: disabled + reason beside it.**
- **Q13** Which judgment drafts are chart lines? **Default: every judgment the owner can drag on a
  chart** — §1 est-high / est-low EPS (a projected EPS-growth draft shown as the est-high line it
  implies) and §3 judged high / low P/E (#115); the rest get an action chip; `forecast_low_option`
  gets a chip naming the option. Note: Story 8.6 calls a forecast P/E « not a chart line » — this
  default contradicts it and 8.6 would be amended.
- **Q14** Action count for a draft study (open → Valider → Créer = 3 vs FR74 ≤ 2)? **Default: a
  « Valider… » button directly on the draft-study row** (opens the prefilled create dialog; +
  « Créer » = 2). For other drafts, opening the row counts as action 1.
- **Q15** FR64 on compact marks (chart label « IA … », action chips, ★ cell mark, trace line)?
  **Default:** these carry **no AI-written text** (only app-formatted values and the ★/« IA »
  label); the disclaimer is satisfied by the always-visible footer (FR64) plus the `AiFrame` +
  disclaimer in the decision dialog each of them opens. Alt: a hover/focus popover with the
  AiFrame on each mark.

- **Q16** The AI glyph: « ★ » (bundled in Inter, but a star can read as a rating) **or** bundle a
  symbol font covering « ✦ » (U+2726, a « spark », no rating connotation, +1 font file and its
  licence) **or** accept « ✦ » from the system fallback (unmeasurable gate across machines).
  **Default: « ★ »**, measured in the gate against ◆ ◦ △ at 14 px.

## 10. Gates & verification
- Every new `@tr` string from §3.3 verbatim; the posture/banned-verb gate over them; `@tr` and
  `MSG_*` count deltas stated per UI story (review checklist §6).
- The `AiFrame` structural test with its exemption list; the « ★ » font-coverage check and its
  confusability entry (★/◦, ★/◆, ★/△) (UX-DR15).
- Headless walk per UI story on a temp dossier seeded with `just mcp-seed` (from 8.5a; 8.1 on a
  plain temp copy), window resized to the Xvfb screen; Guy's on-display check of the Propositions
  screen, the decision dialog, the dotted chart line, the ★ cell mark and the frozen-verdict strip.
