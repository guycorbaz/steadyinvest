# Story 7.0 — UX pass: entry dialogs, acknowledged refusals, panel cards (Portefeuille first)

Status: review (PR 1 #211 — shared components + Portefeuille; PR 2 — Liste de suivi, Études list, Réglages)

## Story

As Guy,
I want every data entry to happen in a titled dialog with labelled fields and one explanatory
sentence, every refusal to be a dialog I must acknowledge, and every screen to group its content
in titled cards with visible state bands,
so that entering a buy, a sell or a dividend is unambiguous, a refused write can never be
overlooked, and a screen reads at a glance — while the app keeps its neutral voice and its
colour budget.

Spec: `_bmad-output/planning-artifacts/ux-entry-dialogs-refusals-cards.md` (PR #206, validated
by Guy 2026-09-23). Umbrella issue #208. Origin: the on-display walk 2026-09-22/23.

## Acceptance Criteria

1. **AC1 — Routing rule.** A REFUSAL (a gesture whose write did not happen) opens the modal
   notice (« Action refusée » + the unchanged message + « Compris »), or — while one of the
   Dialog's own forms is open — lands inline in that form (`field-error`), the user's text intact.
   A STATE renders as a `StatusBand`, never a dialog. An OUTCOME stays the inline notice (F4).
2. **AC2 — Shared components.** `ModalDialog` (notice / confirm / form, on the confront-overlay
   pattern: scrim, centred card, Esc + scrim + Annuler cancel, Enter in a field validates, the
   first field / the safe button takes focus, a destructive verb never fires on a plain Enter),
   `PanelCard` (the Réglages card promoted, Réglages unchanged in look), `StatusBand`.
3. **AC3 — Portefeuille.** Cards: Portefeuilles · Candidats · Capital à risque · Dividendes nets ·
   Consolidation · Concentration et taille · Positions · Positions vendues. Form dialogs: add /
   edit a position, Achat, Vente, Dividende, Modifier la transaction, Seuil suiveur, the
   trigger-panel Vente, Ajouter / Renommer un portefeuille, Rachat. Confirm dialogs: delete a
   transaction, delete a portfolio, remove a position. The record callbacks are the pre-pass
   ones with the same arguments; a form closes only on a written result.
4. **AC4 — The currency trap.** The add-position dialog looks the study up as the symbol is
   typed: the currency chip defaults to the study's native currency and the study's company
   name is shown; the register's « Aucune étude liée » band names a same-ticker study in another
   currency when one exists.
5. **AC5 — Posture.** Every new string is French inside `@tr()`, neutral, no banned verb (the
   « Acheter / Vendre / Racheter » verbs became the nouns Achat / Vente / Rachat); the floor is
   re-based on the measured total (594); the two dialog glyphs are allow-listed.
6. **AC6 — Gates.** fmt, clippy `-D warnings`, `cargo test --all` green; headless walk of the
   Portefeuille dialogs (add position + lookup + Esc, delete-portfolio confirm → guard refusal
   notice, ledger Dividende form); Guy's on-display check of B1/B3/B4/B7 on the new dialogs.

## Tasks / Subtasks

- [x] **Task 1 — Components** (`app/ui/components/`): `panel_card.slint`, `status_band.slint`,
  `modal_dialog.slint` (+ `LabeledField`); `TextField.edited`; `ActionButton.autofocus` +
  `forward-focus` (a `focus()` inside `init` of a conditionally created element is dropped —
  deferred by a 30 ms `Timer`).
- [x] **Task 2 — State + wiring**: the `Dialog` global (`state.slint`, exported in `app.slint`,
  mounted above the shell); `wiring/dialog.rs::refuse`; `apply_holdings_result` and the
  refresh refusals route through it; `sell-holding`, `set-trailing-stop`, `add-portfolio`,
  `rename-portfolio` report `bool`; `study-currency-for` / `study-name-for` pure lookups;
  `HoldingRow.study-other-currency`; the ledger-delete confirm props retired.
- [x] **Task 3 — Portefeuille** (`screens/portfolio.slint` rewritten, 1 214 → 805 lines):
  cards, dialog openers (titles + sentences), bands (unstopped exposure, missing rates,
  unavailable, murmur, non classé, all-unclassified, the study-link cause), row actions with
  the « … » suffix.
- [x] **Task 4 — Réglages** keeps its look on `PanelCard` (`SettingsPanel` removed).
- [x] **Task 5 — Gates + headless verification** (see Dev Notes).
- [x] **Task 6 — PR 2**: Liste de suivi (« Valeurs suivies » card, « Ajouter une valeur… »
  dialog, the buy-zone summary as a band, `add-watch` reports bool); Études list (one card,
  « Créer une étude… » dialog with the reference currency prefilled, the 2.12 archive/delete
  prompt parked by Rust as a Dialog confirm — title/verb derived in the overlay —, the notice
  slot as a band); Réglages (the older-import and restore prompts as Dialog confirms, the
  manual FX rate as a dialog, every validation / key / dossier / FX refusal through
  `dialog::refuse`); `dialog::confirm` for the Rust-parked prompts; the overlay's `cancel()`
  tells the parking side (`cancel-study-action` / `cancel-import` / `cancel-restore`).

## Dev Notes

### Decisions taken (defaults of the spec's open questions, Guy did not object)
- Q1 outcomes stay inline. Q2 one story, two PRs. Q3 row actions stay on the row.
- The trigger-panel sell keeps the 4.7 semantics (quantity + rationale at the study's present
  price) in its own form (`sell-trigger`); the ledger sell keeps the explicit obtained price.
- « Retirer » a position now asks for confirmation (it was one click; it drops the ledger too).
- Focus return to the opening control is NOT done (no element-reference API in Slint); Esc and
  the scrim close, Tab order inside the card is the layout order.

### Headless lessons (added to `.claude/skills/verify/SKILL.md`)
- Resize the window to the Xvfb screen, else nothing overflows and nothing scrolls.
- **Give the window X input focus** (`xdotool windowfocus --sync $WID`) before typing or
  pressing Esc: with no window manager the app never has keyboard focus, and the symptom reads
  exactly like « the field ignores keystrokes » / « Esc does nothing ».

### Verification (2026-09-23, headless on a copy of Guy's test dossier)
- Portefeuille renders as eight cards; the NVDA row band states « l'étude NVDA.US est en USD et
  cette position en CHF ».
- « Ajouter une position… » → typing `NVDA.US` shows « Étude liée : Nvidia (USD) », the chip
  switches to USD, Enregistrer enables; Esc closes.
- « Supprimer le portefeuille » → confirm (Annuler focused) → « Supprimer » → the guard's
  refusal opens as « Action refusée » with the 6.1 message; « Compris » / Esc closes.
- « Transactions » → « Dividende… » → the labelled form with « Retenue à la source (vide = 35 %) ».
- Gates: fmt clean, clippy `-D warnings` clean, `cargo test --all` 821 passed.

### Verification PR 2 (2026-09-23, headless, same copy)
- Liste de suivi: the card; « Ajouter une valeur… » → typing `NVDA.US` + Enter writes the row
  (« NVDA.US · Aucune étude liée · Lier une étude »).
- Études: the card; « Créer une étude… » → Symbole + « Devise des chiffres » prefilled CHF;
  « Archiver » → « Confirmer cette action ? / Archiver l'étude NVDA.US ? … » (Annuler focused).
- Réglages: threshold `150` + Enregistrer → « Action refusée » with the 6.7 message;
  « Ajouter un taux… » → the three labelled fields. @tr total measured 612 (floor re-based).
- Kept on purpose: Réglages fields re-sync to the effective value after a refusal (#88/#93) —
  the refused value is in the dialog's message, the field shows what is in force. The key-test
  VERDICTS (« clé refusée », quota, inconclusive) stay a status line: a verdict is an outcome of
  the test, not a refused write.
- Pre-existing, not fixed here: at 1 600 px the Études row's five actions overflow the card
  (the fixed-width columns of #204 need ~1 700 px); Guy's display is wider.

### Walk (2026-09-24, headless on a copy of Guy's dossier — Guy did A.1 / A.2 on display, Claude the rest)
- Portefeuille: Achat (5 × 80), Vente partielle (3 × 82), Dividende (12 × 3, retenue 12.6, net
  23.4) through the forms, Enter validates, the outcome line names each write; « Modifier… » on a
  transaction prefills date / quantity / price / fees; « Supprimer » confirms with Annuler focused
  and a plain Enter deletes nothing; « Définir un seuil… » 10 % → « Stop 10 % : 69,39 CHF · à
  7,71 CHF au-dessus ».
- Liste de suivi: « Ajouter une valeur… » NESN.SW writes the row. Études: « Créer une étude… »
  with the currency `Dropdown` (CHF). Réglages: « Ajouter un taux… » USD → CHF 0.8 through the
  currency `Dropdown`, listed « USD → CHF 0.8 le 2026-09-24 (manuel) ».
- **Fixed in the same PR** (findings of the walk):
  1. after a ledger write the position's « Prix d'achat » showed the exact aggregate with
     28 decimals (`108.66666…`) — the row now carries a display spelling
     (`purchase-price-text`, locale + price scale); the raw value stays for the edit form;
  2. the open ledger's wider toggle squeezed the zone / price column to one glyph per line and
     the row grew tenfold — the column has a 170 px floor and the toggle reads « Masquer »;
  3. the same symbol could be added twice to the watchlist — refused (« Ce symbole est déjà
     dans la liste de suivi », `MSG_WATCH_DUPLICATE`, inventory 133 → 134, case-insensitive);
  4. two transactions on the same day listed in random order (`ORDER BY occurred_at, id` with
     v4 ids) — now `occurred_at, created_at, id`, so the ledger reads in entry order and the
     replayed basis matches what is listed;
  5. the « Seuil suiveur par défaut » field cut its placeholder (« Pourcentage (0–10 ») — widened;
  6. the thousands separator: the UI default font has no glyph for U+202F, so a `StatusBand`
     drew « 1540 CHF » beside a numeric line drawing « 1 540 » — the formatter now emits U+00A0
     (every font carries it) and the paste parser accepts both.
- Not defects, seen again: NVDA « non classé (chiffre d'affaires indisponible) » = the CHF
  position vs the USD study (#81 link rule); the sector « Grande » on NVDA is a value typed in
  the dossier (the sector field is free text on purpose).
- Not exercised headless: the native save / open pickers (PDF export, import, restore) — Guy
  exported PDFs on display during the 2026-09-22/23 walk.
