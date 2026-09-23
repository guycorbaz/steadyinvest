# UX pass — entry dialogs, acknowledged refusals, panel cards

**Status:** spec for review (Guy) — no code yet. **Origin:** the on-display walk of 2026-09-22/23
(`docs/on-display-walk-epics-5-6.md`, volets A–B). **Decision (Guy, 2026-09-23):** the direction and
the order below are validated; the spec is to be read before any code. **Placement:** one story,
before Epic 7's first UI stories (7.1 / 7.2), which would otherwise add two screens on the same
patterns. `app` crate only (`app/ui` + `app/src/wiring`); no contract, core, persistence or report
change.

## 1. What the walk showed

| Observation (Guy, on display) | Root cause in the current UI |
|---|---|
| « Nothing happens » on Achat / Vente / Dividende | Five same-looking fields on ONE row under the ledger; the buttons are disabled until quantity + price are typed; « Prix unitaire » means *gross per share* for a dividend. |
| The entry row sat below the window edge | No scroll container (fixed in PR #205) — but a row at the bottom of a history is still the wrong place for an action. |
| Refusals are easy to miss (delete a non-empty portfolio, invalid threshold, invalid size bound) | A refusal is a caption-sized grey line in the same slot as everything else. |
| « Rien ne s'affiche » in the size mix | Three `0 %` lines + a fourth « non classé (taux manquant USD → CHF) » line, all in the same ink; the cause is stated but not *seen*. |
| NVDA held in CHF while the study is in USD → « Aucune étude liée » | The add-holding form never suggests the study's currency; the link rule (issue #81, same ticker AND currency) is invisible at entry time. |
| Portefeuille is hard to read | A flat column of same-size text lines: CaR, consolidation, concentration, add form, register — no cards, no titled groups (Réglages has 15 cards and reads fine). |

## 2. Goals / non-goals

**Goals**
1. Every **refusal** of a user action is shown in a modal dialog the user must acknowledge.
2. Every **data entry or edit** (holding, buy, sell, dividend, trailing stop, portfolio, watchlist
   ticker, study creation, manual FX rate) happens in a modal **form dialog** with a title, one
   explanatory sentence, named fields and a single validating button.
3. Every screen groups its content in **titled cards**, the way Réglages already does.
4. Persistent **states** (missing rate, unclassified, no linked study, threshold murmur) become a
   visible **status band**, not a caption line — and are NOT modal.

**Non-goals**
- No new facts, no new computations, no contract or schema change.
- No OS-native dialogs beyond the existing `rfd` file pickers (Slint has none; all dialogs are
  in-window overlays, like the confront overlay of Story 5.1).
- No colour spent: the colour budget (UX spec, « Monastic Colour Budget ») stays untouched —
  dialogs and bands carry attention by **placement, scrim, icon and ink**, never by red/amber.
- Success notices (« Le dossier a été exporté. ») stay inline — see §3. (Open question Q1.)

## 3. Message vocabulary — one rule for routing

Today one `notice` string per global carries three different things. The pass separates them:

| Kind | Definition | Surface | Examples (existing `MSG_*`) |
|---|---|---|---|
| **Refusal** | The user acted; the write did not happen; the message names the cause. | **Modal notice dialog**, one button « Compris ». Focus returns to the field/control that triggered it; the user's text is kept. | « Le seuil de concentration doit être un pourcentage entre 0 et 100 ; rien n'a été enregistré. » · « La quantité dépasse la quantité détenue à cette date ; rien n'a été enregistré. » · portfolio delete guards · « Le fichier n'est pas un dossier valide ; rien n'a été restauré. » |
| **State** | A situation that exists regardless of the last gesture and lasts while it exists. | **Status band** (persistent, icon + surface-alt + border, `text-mid`), placed at the top of the card it concerns. Never modal. | « taux manquant USD → CHF » · « non classé (…) » · « Aucune étude liée » · « seuil 50 % approché ou atteint » · « Dossier en lecture seule (schéma plus récent) » · « 1 position sans seuil suiveur (2 000 CHF non couverts) » |
| **Outcome** | The gesture succeeded; a fact to know, nothing to do. | **Inline notice** in the card's notice slot (F4 rule unchanged: replaces only the in-progress banner or an empty slot). | « Le dossier a été exporté. /path » · « Le dividende a été enregistré. » · « Les taux ont été actualisés. » |
| **Confirmation** | A destructive or irreversible gesture needs an explicit yes. | **Modal confirm dialog**, two buttons (« Annuler » / the verb). | delete a transaction (6.3), restore a backup (5.4), import an older dossier (#65), delete a portfolio (6.1). |

**Routing rule (mechanical):** a message emitted *in response to a gesture* that ends in « ; rien
n'a été enregistré. », « ; l'écriture n'a pas eu lieu. », « n'a pas été … » or names a guard
(« … contient des positions », « dernier portefeuille ») is a refusal → dialog. A message set
during a *render* (rebuild of rows, screen activation) is a state → band. The F4 notice-slot rule
keeps applying to outcomes. The neutral-voice / banned-verb gate (2.14) applies to every dialog
string.

## 4. Shared components (`app/ui/components/`)

### 4.1 `ModalDialog` — one overlay, three variants
Built on the `ConfrontOverlay` pattern (scrim `Tokens.bg` @ 0.72, centred card ≤ 640 px, click
on the scrim = cancel, TouchArea inside the card swallows clicks). One `Dialog` global in
`state.slint`:

```
export global Dialog {
    in-out property <string> kind;      // "" (hidden) | "notice" | "confirm" | "form"
    in-out property <string> title;
    in-out property <string> body;      // one explanatory sentence (forms) or the message (notice)
    in-out property <string> form-id;   // which form to mount: "holding-add" | "holding-edit" |
                                        // "buy" | "sell" | "dividend" | "stop" | "portfolio-add" |
                                        // "portfolio-rename" | "watch-add" | "study-create" | "fx-rate"
    in-out property <string> field-error; // a refusal raised BY the form, shown inside it
    callback confirm();                 // confirm variant: the destructive verb
    callback cancel();                  // every variant: Esc, scrim, « Annuler », « Compris »
}
```
- **Notice**: title (« Action refusée »), the message, one button « Compris » (focused; Enter and
  Esc both close). An icon (⚠ in ink) on the left of the message — no hue.
- **Confirm**: title, the message (already worded by the existing `MSG_*_CONFIRM` constants), two
  buttons: « Annuler » (focused by default) and the verb (« Supprimer », « Restaurer »,
  « Importer »). Enter = the verb only after Tab to it (a destructive verb never fires on a reflex
  Enter).
- **Form**: title, one explanatory sentence, the fields (each with its own label ABOVE the field —
  never a placeholder as the only label), a `field-error` line under the fields for a refusal
  raised by the form itself (kept inline: the user must see it next to the offending field, with
  their text intact), buttons « Annuler » / « Enregistrer ». Enter in the last field validates.
- Keyboard: Esc cancels everywhere; focus is trapped in the card; first field focused on open;
  on close, focus returns to the control that opened the dialog.
- Rust side: `Dialog` is driven by the wiring, not by screens. A refusal today does
  `holdings.set_notice(msg)`; it becomes `dialog::refuse(&ui, msg)` (one helper). Outcomes keep
  `set_notice`. The three existing inline confirms (ledger delete, restore, older-import) move to
  the confirm variant; their `*-confirm-visible` / `*-confirm` properties disappear.

### 4.2 `PanelCard` — the Réglages card, promoted
`SettingsPanel` (title + `@children`, `Tokens.surface`, border `Tokens.separator`,
`Tokens.radius`, padding 16/24) moves to `components/panel_card.slint` as `PanelCard`; Réglages
keeps working unchanged (alias or direct rename). Optional `subtitle` (caption under the title) and
an optional `band` slot (§4.3) at the top of the card.

### 4.3 `StatusBand` — a state you cannot miss, without colour
A full-width row: surface `Tokens.surface-alt`, 1 px border `Tokens.separator`, left ink icon
(hollow dot for « stale/missing », ⚠ for a threshold murmur, ✕-in-circle for « indisponible »),
text `text-mid`, wrap. Placed as the first row of the card it concerns; several bands stack. It
is *the* rendering of every state in §3, replacing the caption lines.

## 5. Screen by screen

### 5.1 Portefeuille (first — the walk's pain point)
Cards, top to bottom:
1. **Portefeuilles** — the selector chips, then two buttons: « Ajouter un portefeuille… » (form
   dialog: name) and « Renommer… » (form dialog: name, prefilled); « Supprimer le portefeuille »
   (confirm dialog when it is empty; when it holds positions the guard is a *refusal* dialog that
   names the count). Bands: « Dossier en lecture seule » when applicable.
2. **Capital à risque** — the per-currency lines; band: « n position(s) sans seuil suiveur
   (… non couverts) ».
3. **Consolidation** — per-bank lines + total + the rates footnote; band: « Total global
   indisponible : taux manquant … ».
4. **Concentration et taille** — the per-security shares (murmur → band, not an inline suffix),
   the size mix, the « non classé » lines as bands. When every share is 0 % *because* nothing is
   classified, the three 0 % lines are still shown (honest) but the band explains it first.
5. **Positions** — « Ajouter une position… » button (form dialog, §5.1.1) above the register.
   Each row keeps its actions, but « Modifier », « Seuil % », « Transactions » open dialogs
   (§5.1.2). Row bands: « Aucune étude liée (l'étude NVDA.US est en USD) » — the currency
   mismatch is *named*, which is what today's text hides.
6. **Positions vendues** — unchanged, collapsible.

#### 5.1.1 « Ajouter une position »
Title « Ajouter une position ». Sentence: « La position est rattachée à l'étude du même symbole et
de la même devise. » Fields: Symbole (with the linked study's name shown once matched), Quantité,
Prix d'achat unitaire, Devise (chips; **defaults to the matched study's currency** when a study
exists, else the reference currency — the walk's CHF/USD trap), Secteur (facultatif). Refusals
from `record_holding` render in the form's `field-error`. Same dialog for « Modifier la
position » (prefilled, title changes).

#### 5.1.2 Ledger dialogs (replacing the one row under the history)
« Transactions » opens the history as today (read + edit/delete rows) **and** three buttons at
the top of the history, each opening its own form:
- **« Acheter »** — sentence: « Un achat augmente la position et recalcule le prix moyen. »
  Fields: Date (vide = aujourd'hui), Quantité, Prix unitaire payé, Frais (vide = 0), Raison
  (facultative).
- **« Vendre »** — sentence: « Une vente partielle garde la position ; une vente totale la retire
  (elle reste consultable dans « Positions vendues »). » Fields: Date, Quantité (vide = toute la
  position), Prix unitaire obtenu, Frais, Raison. Oversell refusal → `field-error`.
- **« Dividende »** — sentence: « Le brut par action est multiplié par les actions ; la retenue
  (vide = {taux Réglages} %) donne le net réinvestissable. » Fields: Date, Actions (vide = toute
  la position), Brut par action, Retenue (vide = {x} %), Raison. The Réglages rate is shown in the
  label, not only in a hint.
- **« Modifier la transaction »** — same fields as its kind, prefilled.
- **« Seuil suiveur »** — sentence: « Le seuil se calcule depuis le plus haut atteint ; vide =
  le seuil par défaut des Réglages. » One field.
- Delete a row → confirm dialog (replaces the inline confirm of 6.3 review P-86).

### 5.2 Liste de suivi
Cards: **Valeurs suivies** (« Ajouter une valeur… » form dialog: Symbole; the alert facts as
bands). Refusals (unknown/duplicate ticker) → notice dialog.

### 5.3 Études (list)
Cards: **Créer une étude** becomes a button + form dialog (Symbole, Devise, Nom de société);
**Rechercher** stays inline (it is a filter, not an entry); **Études** (the list) and the
« Importer une étude… » picker unchanged. Delete → confirm dialog.

### 5.4 Réglages
Already carded. Refusals (threshold, size table, FX rate/date, key errors) → notice dialog, the
text kept in the field. « Ajouter un taux » manual FX entry → form dialog (Devise, Taux vers
{ref}, Date (vide = aujourd'hui)). Restore / older-import confirms → confirm dialog.

### 5.5 Study screen (§1–§5 form)
**Out of scope** for this pass: the study form is the dense entry regime (Spike A) and has its own
validation rail (tri-state tags, soft lock). Its refusals already render next to the cell.

## 6. Acceptance criteria

**Given** any gesture whose write is refused (every `MSG_*` that names a non-write)
**When** the refusal is raised
**Then** a modal notice dialog shows the unchanged message, with one button « Compris »
**And** the user's typed text is still in the field after closing
**And** no notice slot of another card was replaced (F4 rule).

**Given** a persistent state (missing rate, unclassified, no linked study, murmur, read-only)
**When** the screen renders
**Then** it is a `StatusBand` at the top of the card it concerns, never a dialog
**And** it still names its cause (« taux manquant USD → CHF », « étude en USD »).

**Given** Portefeuille with an open ledger
**When** the user clicks « Acheter », « Vendre » or « Dividende »
**Then** a titled form dialog opens with labelled fields and one explanatory sentence
**And** the record callbacks (`record-buy` / `record-sell` / `record-dividend` / `update-transaction`)
are the SAME callbacks as today with the same arguments (no state-layer change).

**Given** « Ajouter une position » for a ticker whose study is in USD
**When** the dialog opens
**Then** the currency chip defaults to USD and the study name is shown next to the symbol.

**Given** every screen
**When** rendered
**Then** each functional group is inside a `PanelCard` with a title, and the posture test's exact
`@tr` counts are updated deliberately (documented delta), not silenced.

**Given** the keyboard
**When** a dialog is open
**Then** Esc cancels, focus is trapped, the first field is focused, and a destructive verb never
fires on a plain Enter.

## 7. Gates & verification
- `cargo fmt`, `clippy -D warnings`, `cargo test --all`, `cargo deny`; the posture / banned-verb
  gate over every new `@tr` string; the `@tr` count delta stated in the story.
- Headless walk (`.claude/skills/verify`) per screen, **window resized to the Xvfb screen**
  (`xdotool windowsize`), else nothing overflows and nothing scrolls — the 2026-09-23 lesson.
- Guy's on-display check: repeat B1, B3, B4, B7 of the walk on the new dialogs.

## 8. Open questions (defaults proposed)
- **Q1** Should *outcomes* (« … a été enregistré. ») also be dialogs? **Default: no** — an
  acknowledged dialog on every success would train reflex-clicking and devalue the refusals.
- **Q2** One story or two (components + Portefeuille first, then the other screens)? **Default:
  one story, two PRs** in the order of §5.
- **Q3** Keep the row actions « Modifier / Retirer / Transactions » on the register row, or move
  them behind a single « … » menu? **Default: keep** (discoverability beats density here).
