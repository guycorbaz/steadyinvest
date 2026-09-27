# UX pass — AI-assistance surfaces (Story 8.0)

**Status:** spec for review (Guy) — no code yet. **Origin:** G2 (PR #255, merged 2026-09-27):
PRD Phase 4 (FR13, FR17, FR33, FR64, FR68, FR69–FR78), architecture §Phase 4 (A1–A13), owner
decisions O1–O7 and D1–D11. **Placement:** Story 8.0, before any Epic 8 UI story (8.1, 8.5a, 8.5b,
8.6, 8.7, 8.8) — the same role the 7.0 pass (PR #206) played for Epic 7. `app` crate
(`app/ui` + `app/src/wiring`) and `report` (study PDF) only; the data model is 8.2a/8.2b.
**Wording:** §3 is the one French list the UI stories copy verbatim; every genuinely open wording
choice is in §9 with a default — **Guy decides the wording**.

## 1. What has to fit

| New thing (Epic 8) | Why it needs a design before code |
|---|---|
| Drafts arrive from outside the app (MCP), while it is open or closed | Nothing in the UI today announces an external event; it must be a *state* (seen, not modal). |
| AI text (comments, proposed notes) is third-party content | FR13/FR64: it must never read as an app signal — one frame, labelled, with the disclaimer. |
| A pending judgment draft may be a chart line (EPS forecast) or not (P/E, growth %, severe low) | The chart is the signature surface (UX-DR10) and never auto-suggests (FR33); the proposal must be visibly *not* the owner's. |
| A validated AI value stays marked until the owner edits it (FR17) | The §3 cell already carries five markers in two columns (G1 J / M4); a sixth must pass the confusability gate (UX-DR15). |
| Study notes (FR78) | Today the study has ONE free-text field, « Justification de la décision » (`RationaleNote`); notes are a dated list, not a second free box. |
| « Valider l'étude » freezes the verdict (FR68, D11) | Two verdicts on one screen (frozen vs current) must not spend a second zone hue, and the difference must be seen. |

## 2. Goals / non-goals

**Goals**
1. One **Propositions** destination: the dossier-level inbox (pending) and the drafts record
   (processed), reachable from the nav rail, with a pending count.
2. One **decision dialog** used from everywhere (inbox, study band, chart, judgment chip): current
   vs proposed side by side, the AI comment framed, ≤ 2 actions from the inbox (open → Valider).
3. One **`AiFrame`** component — the only place AI-authored text is rendered (A12 structural test).
4. One **AI glyph** « ◆ » for every AI-related mark (bands, cell mark, chart chip, history, list),
   ink only.
5. **Notes** as a titled card of dated entries under the rationale, entered through dialogs.
6. **« Valider l'étude »** in the study action row; the frozen verdict shown under the verdict bar,
   a side-by-side comparison only when it differs.

**Non-goals**
- No hue spent (UX spec « Monastic Colour Budget »): AI marks, the frozen verdict and the
  differences carry attention by glyph, placement, weight and ink — never a colour. The only
  coloured verdict on screen stays the *current* one.
- No toast/pop-up when a draft arrives (a draft is a *state*, not an event to acknowledge).
- No bulk action anywhere (FR74). No in-grid preview of a pending value (the grid shows what is
  true; §9 Q6).
- No change to the comparison screen (7.1) — see §7.

## 3. Vocabulary and the French wording list

### 3.1 Routing (the 7.0 rule, unchanged)

| Kind | Surface | Epic 8 examples |
|---|---|---|
| **Refusal** | modal notice « Action refusée » + « Compris » (or `field-error` inside an open form) | a decision on a read-only dossier; a write failure; target study deleted since listing; « Valider l'étude » on a non-full verdict; a duplicate at draft-study validation |
| **State** | `StatusBand` at the top of the card / screen it concerns | pending drafts on this study; pending draft studies; inbox unreadable; frozen verdict differs; stale / target gone inside the decision dialog |
| **Outcome** | inline notice (F4 slot) | « Proposition validée. », « Étude validée ; verdict figé le 27/09/2026. » |
| **Confirmation** | modal confirm | delete a note; validate a stale draft (O4); replace an existing frozen verdict |

### 3.2 Terms (defaults — alternatives in §9)

| Concept | French (UI) |
|---|---|
| AI (label, frame, glyph caption) | **IA** |
| a draft | **proposition** (« proposition de l'IA ») |
| the inbox destination | **Propositions** |
| the drafts record | **Registre des propositions** |
| pending | **en attente** |
| stale | **périmée** |
| target gone | **cible disparue** |
| validated | **validée** |
| validated then undone | **validée puis annulée** |
| rejected | **rejetée** |
| edited before validation | **modifiée avant validation** |
| AI origin line | « Proposée par {client} ({modèle}) le {JJ/MM/AAAA} » |
| after validation | « proposée par l'IA, validée le {JJ/MM/AAAA} » |
| on a judgment after validation | « placée par l'IA · validée le {JJ/MM/AAAA} » |
| frozen / current verdict | « **Figé** ({méthode}, {JJ/MM/AAAA}) » / « **Actuel** ({méthode}, aujourd'hui) » |

### 3.3 Strings (copied verbatim by the stories)

**Nav rail, inbox, record (8.5a, 8.7)**
- Destination: « Propositions » — with a count when > 0: « Propositions · {n} ».
- Card titles: « Propositions en attente » · « Registre des propositions ».
- Subtitle (pending): « Proposées par une IA via le serveur MCP ; aucune n'est appliquée sans votre validation. »
- Chips: « En attente » · « Registre » · « Toutes » · « Valeurs » · « Jugements » · « Notes » · « Études ».
- Record filter: « Étude : » (drop-down, « Toutes les études ») · outcome chips « En attente » · « Validées » · « Validées puis annulées » · « Rejetées ».
- Empty (pending): « Aucune proposition en attente. Une IA enregistrée comme client MCP peut en déposer ici ; rien n'est appliqué sans votre validation. »
- Empty (record): « Aucune proposition traitée pour le moment. »
- Unreadable (band ⊘): « Les propositions n'ont pas pu être lues ; la liste est indisponible ({cause}). »
- Read-only (band ◦): « Dossier en lecture seule : les propositions sont consultables, aucune décision n'est possible. »
- Row target forms: « {TICKER} · {champ} · {année} » (cell) · « {TICKER} · {champ} » (judgment) · « {TICKER} · note » · « Nouvelle étude : {TICKER} ({DEV}) ».

**Decision dialog (8.5b, 8.6, 8.7)**
- Title: « Proposition de l'IA ».
- Columns: « Actuel » · « Proposé ».
- Buttons: « Valider » · « Rejeter » · « Modifier avant de valider… » · « Annuler ».
- Edit form: title « Modifier la proposition » · sentence « La valeur enregistrée sera la vôtre ; la proposition sera notée modifiée avant validation. » · button « Enregistrer ».
- Other study: context « Valider ouvre l'étude {TICKER} ({DEV}). »
- Stale band (◦): « Proposition périmée : {champ} a changé depuis la proposition ({avant} → {maintenant}). »
- Stale confirm: title « Valider une proposition périmée ? » · body « {champ} de {TICKER} a changé depuis la proposition. La valeur proposée remplacera la valeur actuelle. » · verb « Valider ».
- Target gone band (⊘): « Cible disparue : {raison} ; la proposition ne peut qu'être rejetée. » (raisons : « l'année {AAAA} n'existe plus dans l'étude » · « l'étude a été supprimée »).
- Changed since confirmation (refusal): « La cible a encore changé depuis votre confirmation ; rien n'a été enregistré. La proposition est affichée de nouveau. »
- Draft study: context « Valider ouvre la création de l'étude, préremplie ; l'étude n'est créée qu'à « Créer ». » Duplicate (refusal): « Une étude {TICKER} en {DEV} existe déjà ; rien n'a été créé. »
- Refusals: « Le dossier est en lecture seule ; aucune décision n'a été enregistrée. » · « L'étude {TICKER} n'existe plus ; aucune décision n'a été enregistrée. » · « La décision n'a pas pu être enregistrée ({cause}) ; rien n'a été modifié. »
- Outcomes: « Proposition validée. » · « Proposition rejetée. » · « Proposition validée (modifiée avant validation). » · undo: « Validation annulée ; la proposition est notée validée puis annulée. »

**AiFrame (everywhere)**
- Header: « ◆ IA — Proposée par {client} ({modèle}) le {JJ/MM/AAAA} ».
- Disclaimer (always, last line): « Texte rédigé par une IA, non vérifié — ne constitue pas un conseil financier. »

**Study reminders (8.5a)**
- Band on the study (◆): « {n} proposition(s) de l'IA en attente sur cette étude. » + button « Voir les propositions ».
- Band on the Études card (◆): « {n} proposition(s) d'étude de l'IA en attente : {TICKER} ({DEV}), … » + button « Voir les propositions ».
- List row marker: « ◆ {n} ».

**Cell and judgment marks (8.5b, 8.6)**
- Cell trace line (traceability overlay « Entrées & provenance »): « {valeur} — proposée par l'IA ({client}, {modèle}), validée le {JJ/MM/AAAA} ».
- Judgment chip under a `JudgmentField` (pending): « ◆ IA : {valeur} » (opens the decision dialog).
- Chart chip (issue #121 selector): « ◆ Proposition IA ».
- Chart endpoint label (pending line): « IA {valeur} ».
- After validation (caption under the field / at the line end): « placée par l'IA · validée le {JJ/MM/AAAA} ».

**Notes (8.1)**
- Card: « Notes » · button « Ajouter une note… » · row buttons « Modifier… » · « Supprimer ».
- Empty: « Aucune note pour cette étude. »
- Form: title « Ajouter une note » / « Modifier la note » · sentence « Une note reste dans l'historique de l'étude, même supprimée. » · field « Note » · button « Enregistrer ».
- Delete confirm: title « Supprimer la note ? » · body « La note quitte l'étude ; elle reste lisible dans l'historique. » · verb « Supprimer ».
- Refusal: « La note est vide ; rien n'a été enregistré. »
- Row meta: « {JJ/MM/AAAA} » (created) + « · modifiée le {JJ/MM/AAAA} » when edited.

**History (8.1, 8.7)**
- Toggle chip: « Masquer les notes » / « Afficher les notes ».
- Entry summaries: « Note ajoutée » · « Note modifiée » · « Note supprimée » · « ◆ Proposition validée : {cible} {avant} → {après} » · « ◆ Proposition validée (modifiée) : … » · « ◆ Proposition rejetée : {cible} » · « ◆ Proposition validée puis annulée : {cible} » · « Étude validée ; verdict figé ».

**« Valider l'étude » and the frozen verdict (8.8)**
- Button: « Valider l'étude ».
- Refusal (not full): « L'étude ne peut pas être validée : le verdict n'est pas complet. Entrées encore ouvertes : {liste}. »
- Replace confirm: title « Remplacer le verdict figé ? » · body « Le verdict figé le {JJ/MM/AAAA} sera remplacé ; il reste lisible dans l'historique. » · verb « Valider l'étude ».
- Outcome: « Étude validée ; verdict figé le {JJ/MM/AAAA}. »
- Strip (same): « Étude validée le {JJ/MM/AAAA} ; le verdict actuel est identique au verdict figé. »
- Band (differs, ≠): « Le verdict actuel diffère du verdict figé le {JJ/MM/AAAA}. » + « Voir la comparaison » / « Masquer la comparaison ».
- Comparison rows: « Verdict » · « Zone du prix » · « Ratio hausse/baisse » · « Potentiel à 5 ans » · « Méthode ».
- Cause line: « Cause : {causes} » with causes « rafraîchissement du {JJ/MM/AAAA} » · « modification de votre part » · « proposition de l'IA validée » · « changement de méthode ({vA} → {vB}) » · « cause inconnue » (none recorded).

**MCP refusal reasons (8.3, 8.4) — decided here**
The MCP server returns a **stable machine code** plus a **French message** (the app's one voice;
the owner reads them through the AI client). Codes are English `snake_case`, never shown in the UI:
- `dossier_mismatch` — « Le dossier a changé depuis la lecture ({chemin lu} ≠ {chemin actuel}) ; rien n'a été enregistré. »
- `schema_mismatch` — « Le dossier est au schéma v{a}, ce serveur MCP en v{b} ; ouvrez-le d'abord dans l'application. » / « … ce serveur MCP est plus ancien que le dossier. »
- `no_dossier` — « Aucun dossier n'a pu être déterminé ; rien n'a été lu. »
- `study_not_found` — « L'étude {id} n'existe pas dans ce dossier ; rien n'a été enregistré. »
- `empty_comment` — « Le commentaire est obligatoire ; rien n'a été enregistré. »
- `missing_origin` — « Le client et le modèle sont obligatoires ; rien n'a été enregistré. »
- `field_not_draftable` — « Le champ {champ} ne peut pas être proposé (liste des champs : voir le schéma de l'outil) ; rien n'a été enregistré. »
- `year_not_in_study` — « L'année {AAAA} n'existe pas dans l'étude {TICKER} ; une proposition n'ajoute jamais d'année. »
- `value_unparsable` — « La valeur « {texte} » n'est pas un nombre dans l'unité de {champ} ({unité}) ; rien n'a été enregistré. »
- `target_has_pending` — « {cible} a déjà une proposition en attente ; rien n'a été enregistré. »
- `study_exists` — « L'étude {TICKER} en {DEV} existe déjà ; rien n'a été enregistré. »
- `draft_study_pending` — « Une proposition d'étude {TICKER} en {DEV} est déjà en attente ; rien n'a été enregistré. »
- `write_denied` — « Écriture refusée : seule la création de propositions est permise. »

## 4. Shared components (`app/ui/components/`)

### 4.1 `AiFrame` — the only home of AI text
A `Rectangle` in `surface-alt` with a `separator` border and a **3 px left rule in `text-mid`**
(the frame's non-colour signature, distinct from `PanelCard` and `StatusBand`). Rows: header
(« ◆ IA — Proposée par … », caption, `text-mid`, semibold « IA »), the AI text (body, `text-high`,
word-wrap, plain text — never rich text, never posture-scanned), the disclaimer (caption,
`text-low`). Properties: `origin` (string), `text` (string), `compact` (bool: header + text on one
line, used in list rows; the disclaimer stays). **Structural test (A12):** a scan of `app/ui/**/*.slint`
fails if a property named `ai-*`/bound to an AI-origin model field is read outside `ai_frame.slint`.

### 4.2 `DecisionDialog` — a fourth `ModalDialog` form (`form-id = "draft-decide"`)
Mounted in the existing `ModalDialog` (scrim, centred card, focus trap, Esc cancels). Layout,
top to bottom: title « Proposition de l'IA »; context line (study, target, « Valider ouvre
l'étude … » when needed); state band (périmée / cible disparue / lecture seule) when it applies;
a two-column grid « Actuel » | « Proposé » (figures in the numeric font; the changed side in
semibold; a note shows its text in each column); the `AiFrame` with the comment; the buttons.
The edit variant (`form-id = "draft-edit"`) swaps the « Proposé » column for a `LabeledField`
(value) or a multi-line field (note), prefilled. A multi-line `LabeledTextArea` is added to
`modal_dialog.slint` for notes (the `RationaleNote` box behaviour, 3–8 rows).

### 4.3 The AI glyph « ◆ »
One monochrome glyph (U+25C6, text presentation — never an emoji), `text-high`, used on bands
(`StatusBand.icon`), the list-row marker, the chart chip, the judgment chip, history entries, and
the cell mark (§5.4). `Tokens.ai-glyph: "◆"` next to `gap-glyph` / `warn-glyph`.

### 4.4 `FrozenVerdictStrip`
Pinned under the verdict bar (above the study notice slot). Two forms: the one-line caption when
frozen = current; a `StatusBand` with « ≠ » when they differ, expandable to the comparison table
(§5.8). Not rendered when the study has no frozen verdict.

## 5. Screen by screen

### 5.1 Nav rail — the « Propositions » destination (8.5a)
A sixth destination, between « Revue » and « Réglages »: `Études · Liste de suivi · Portefeuille ·
Revue · Propositions · Réglages`. Label « Propositions · {n} » while n > 0 (n = pending, all kinds);
plain « Propositions » at 0. No badge, no hue: the count is text in the label (the `NavItem`
already carries active state by bar + weight). The count updates on the A9 poll. **Wiring note:**
« Réglages » moves from index 4 to 5; every `screen-activated(i)` handler and `current-screen`
literal in Rust is re-indexed (a test asserts the destinations list).

### 5.2 The Propositions screen (8.5a pending, 8.7 record)
A scrollable column of `PanelCard`s, like Revue:
1. **« Propositions en attente »** (subtitle §3.3). Chips row: « En attente » | « Registre » (the
   two views of the screen) — then the kind chips « Toutes · Valeurs · Jugements · Notes · Études ».
   Screen-level bands first (unreadable ⊘ / read-only ◦). Then one row per pending draft, grouped
   by study (group header « {TICKER} ({DEV}) · {nom} »; draft studies grouped last under
   « Nouvelles études »), newest first inside a group. Row: target (§3.3 row forms), « Actuel {x} →
   Proposé {y} » (numeric font; for a note the first line of the text, elided), the state word
   when it applies (« périmée », « cible disparue »), the origin date, and the compact `AiFrame`
   comment (one line, elided; the disclaimer stays). The whole row is the hit target: click or
   Enter/Space opens the decision dialog (**action 1**); « Valider » in the dialog is **action 2**.
2. **« Registre des propositions »** (the « Registre » chip — 8.7). Filters: « Étude : » drop-down
   + the outcome chips. Rows: submitted date, target, outcome word (+ « périmée à la décision »,
   « modifiée avant validation » when true), decided date; « Détail » expands the row (the
   history-panel pattern) to the full comment and proposed value/text in an `AiFrame`. Read-only.

States: empty texts §3.3; no « loading » state (a local read; a poll that fails shows the ⊘ band,
never an empty list). Keyboard: Tab through chips → rows; Enter/Space opens; the dialog traps focus.

### 5.3 The study screen — reminder band (8.5a)
When the open study has pending drafts: a `StatusBand` « ◆ {n} proposition(s) de l'IA en attente
sur cette étude. » with the button « Voir les propositions » (opens the Propositions screen
filtered on this study), **pinned** under the verdict bar together with the notice slot (same
reason as G1 J: seen wherever the form is scrolled). It disappears when n = 0.

### 5.4 The study grid — the AI-origin cell mark (8.5b)
A validated AI value (Source::Manual, `?`, `ai_origin` set) shows « ◆ » in the **trailing column,
bottom half** — the slot of the stale dot « ◦ ». They cannot coincide: a validated AI value is a
manual value (freshness `Current`, `Cell::edited`), and the stale dot marks aged provider values
only. *(8.5b verifies this against `contract/src/cell.rs`; if a manual cell can be stale, « ◆ »
takes the leading-bottom slot when no lock shows.)* The mark is cleared by the owner's next edit of
the value (A6). The origin reads in the traceability overlay (§3.3 trace line). **Confusability
gate (UX-DR15):** « ◆ » (filled diamond) joins the set { ? ring, ✓, ⦸, △, ◦, ▦, n/a, overflow } at
14 px; its distinct channels are shape + fill (the only *filled* glyph in the trailing column).
The gate entry is added to the marker spec and to the 8.5b visual verification (side-by-side
◦ / ◆ at `mark-font`).

### 5.5 Pending judgment drafts on the study (8.6)
- **EPS forecast lines (§1 chart):** the proposed line is drawn from the same fixed origin as the
  owner's line, **dashed** (segments built in `viewmodel/chart.rs` — Slint `Path` has no dash
  attribute; the UX-DR10 projected-dash technique), stroke `chart-stroke-thin`, ink `text-mid`,
  **no drag handle** (inert), with the endpoint label « IA {valeur} » (caption, numeric font).
  Non-colour cues: dash + thinner stroke + label. The issue #121 selector gains a chip « ◆
  Proposition IA » while a line draft is pending: Enter/click opens the decision dialog.
- **Non-line judgments** (P/E jugés, croissances projetées, plus bas sévère, option du bas
  prévisionnel, dividende): under the `JudgmentField`, in the slot of the « hist. » chip, a chip
  « ◆ IA : {valeur} » (the `AiFrame` is in the dialog it opens). If a « hist. » chip also shows,
  the IA chip comes second.
- **After validation:** the value is the owner's; the caption « placée par l'IA · validée le
  {date} » sits at the line end (chart) or under the field (non-line), `text-low`, until any write
  to that field (A6). The zone bar and verdict bar are untouched by pending drafts (FR72).

### 5.6 Notes (8.1)
A `PanelCard` « Notes » **directly below** « Justification de la décision » (`RationaleNote`
stays the single decision-rationale field, unchanged). Header button « Ajouter une note… » → form
dialog (`form-id = "note-add"`, `LabeledTextArea`). Rows, newest first: date meta (§3.3), the text
(3 lines, then « Afficher tout »), « Modifier… » (form `note-edit`) and « Supprimer » (confirm). A
note carrying an AI origin renders its text inside an `AiFrame` (header « ◆ IA — Proposée par … »,
+ « validée le … »); an owner edit of such a note clears its AI origin (the text becomes the
owner's). Empty: « Aucune note pour cette étude. » Undo/redo cover add/edit/delete (Ctrl+Z).

### 5.7 History panel (8.1, 8.7)
The « Historique de l'étude » panel gains the chip « Masquer les notes » (default: notes shown).
Note-only entries (consecutive snapshots equal once `notes` are ignored — A12) summarise as
« Note ajoutée / modifiée / supprimée ». Processed drafts are merged in the timeline at
`decided_at` with « ◆ » summaries (§3.3), including rejected ones (no snapshot of their own);
their « Détail » shows the comment in an `AiFrame`.

### 5.8 « Valider l'étude » and the frozen verdict (8.8)
- **Button:** in the study action row, after « Historique », label « Valider l'étude »,
  always enabled (except on the demo). On a non-full verdict it raises the refusal (§3.3) naming the
  open inputs — a disabled button would hide the reason (the 7.0 routing rule). With an existing
  frozen verdict it asks the replace confirmation first. Outcome in the notice slot.
- **Strip** (`FrozenVerdictStrip`, §4.4) under the verdict bar:
  - equal → one caption line « Étude validée le … ; le verdict actuel est identique au verdict figé. »;
  - different → a `StatusBand` with the glyph « ≠ »: « Le verdict actuel diffère du verdict figé le … »
    + « Voir la comparaison ». Expanded, a 3-column table: row label · « Figé (ssg-1.2.0, 12/03/2026) »
    · « Actuel (ssg-1.3.0, aujourd'hui) », rows §3.3. A changed row is marked by a leading « ≠ »
    and semibold figures (two non-colour channels); unchanged rows in `text-mid`. The frozen verdict
    is written as its word (« Zone d'achat », …) **in ink**, never with a zone hue — the current
    verdict keeps the only coloured badge (the verdict bar above). Then the cause line (§3.3).
- Keyboard: the strip's button is in the tab order right after the verdict bar's « Traçabilité ».

## 6. Keyboard operation (NFR-U2) — summary
Nav: « Propositions » is a `NavItem` (Tab, Enter/Space). Screen: chips → rows (Tab), Enter/Space
opens a row, « Détail » toggles. Dialog: focus trapped; initial focus on « Valider » (Q5); Esc
cancels; Enter on a focused button; « Rejeter » and « Modifier avant de valider… » reachable by Tab.
Study: the reminder button, the #121 chip « ◆ Proposition IA », the « ◆ IA » judgment chip, the
notes buttons, « Valider l'étude », « Voir la comparaison » — all Tab-reachable FocusScopes with
the visible focus ring. Ctrl+Z / Ctrl+Y undo/redo a validation while its study stays open (A8).

## 7. Report impact (decided here)
- **Study PDF (5.6):**
  - **Notes: not printed** (unchanged by decision — the rationale is not printed either; the PDF is
    the NAIC form, and the tiebreaker is the form).
  - **AI-origin values: marked.** A printed figure whose cell carries an AI origin gets the sigil
    « † » (WinAnsi; « ◆ » is not in the standard PDF fonts), with one legend line under the table:
    « † valeur proposée par une IA et validée par l'utilisateur ». A judged value that is also
    AI-placed prints « * † ». Honesty over silence: a report must not launder an AI value.
  - **« placée par l'IA »:** the same « † » on the judgment figure (§3/§4) and the legend line.
  - **Frozen verdict (8.8):** a block after the verdict: « Verdict figé le {date} ({méthode}) :
    {verdict} » and, when it differs from the current one, the comparison table (§5.8) in greyscale
    with « ≠ » markers and the cause line.
- **Comparison (7.1) and review (7.2) PDFs and screens: unchanged, by decision** (they compare
  figures across studies; the origin is read on the study). Revisit if Guy asks (Q9).

## 8. Acceptance criteria

**Given** pending drafts **When** the app renders **Then** the nav rail reads « Propositions · n »,
each concerned study shows the ◆ band, a pending draft study shows the ◆ band on the Études card,
and no dialog opens by itself.

**Given** the Propositions screen **When** a row is opened and « Valider » pressed **Then** it took
2 actions, the value is applied as `?` with its AI origin, and the outcome shows inline.

**Given** any AI-authored text **When** it is rendered anywhere **Then** it is inside `AiFrame`
with the disclaimer — enforced by the structural test.

**Given** a stale draft **When** « Valider » is pressed **Then** the confirm dialog names the
changed target; a target-gone draft offers only « Rejeter ».

**Given** a pending EPS-forecast draft **When** the chart renders **Then** a dashed, thinner,
handle-less line labelled « IA {valeur} » shows beside the owner's line, and the zone bar and
verdict are identical with and without it.

**Given** a validated AI value in a §3 cell **When** rendered **Then** « ◆ » shows in the trailing
bottom slot, passes the confusability gate against ◦ at 14 px, and disappears on the next owner edit.

**Given** a study **When** a note is added, edited, deleted **Then** each goes through a titled
dialog (delete confirmed), the list shows dated rows, and history can hide note-only entries.

**Given** a non-full verdict **When** « Valider l'étude » is pressed **Then** « Action refusée »
names the open inputs. **Given** a frozen verdict that differs from the current one **Then** the ≠
band shows, the comparison marks each changed row with ≠ + semibold, the frozen verdict is in ink,
and the cause is named.

**Given** the study PDF **Then** notes are absent, AI-origin figures carry « † » with the legend,
and a frozen verdict prints with its comparison when it differs; the comparison PDF is unchanged.

**Given** the keyboard only **Then** every surface above is reachable and operable (§6).

## 9. Open questions (defaults proposed — Guy decides the wording)
- **Q1** « IA » or « AI »? **Default: « IA »** (French UI, UX-DR29). Alt: « AI » (the PRD's word).
- **Q2** « proposition » or « brouillon »? **Default: « proposition »** — from the owner's side it is
  a proposal to accept or refuse; « brouillon » reads as the owner's own unfinished work. Alt:
  « brouillon ».
- **Q3** « périmée » for stale? **Default: « périmée »**. Alt: « dépassée » (softer) or « obsolète ».
- **Q4** « validée puis annulée »? **Default: yes**. Alt: « annulée après validation ».
- **Q5** Initial focus in the decision dialog: **Default: « Valider »** (a validation is undoable,
  not destructive; two Enters = the ≤ 2-action path). Alt: « Annuler » (safer, 3 keystrokes).
- **Q6** Show a pending value in the grid cell (ghost figure)? **Default: no** — the grid shows only
  what is true; the ◆ band and the inbox carry the proposal.
- **Q7** A sixth nav destination, or a badge on « Études »? **Default: sixth destination
  « Propositions »** (drafts span studies and draft studies; a badge on Études would hide them).
- **Q8** Frozen/current labels: **Default: « Figé » / « Actuel »**. Alt: « Au moment de la décision »
  / « Aujourd'hui ».
- **Q9** Mark AI-origin figures in the comparison (7.1)? **Default: no** (unchanged by decision).
- **Q10** Notes in the study PDF? **Default: no**. Alt: a « Notes » page appended.
- **Q11** MCP refusal messages in French (codes in English)? **Default: yes**. Alt: English messages.

## 10. Gates & verification
- Every new `@tr` string from §3.3 verbatim; the posture/banned-verb gate over them; `@tr` and
  `MSG_*` count deltas stated per UI story (review checklist §6).
- The `AiFrame` structural test (A12); the confusability gate entry for « ◆ » (UX-DR15).
- Headless walk per UI story on a temp dossier seeded with `just mcp-seed` (from 8.5a; 8.1 on a
  plain temp copy), window resized to the Xvfb screen; Guy's on-display check of the inbox, the
  decision dialog, the chart line, the ◆ cell mark and the frozen-verdict strip.
