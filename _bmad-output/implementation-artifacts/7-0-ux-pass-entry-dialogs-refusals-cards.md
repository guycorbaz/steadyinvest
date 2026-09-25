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

### Review Findings — G1 catch-up review (2026-09-25, #237)

3-layer adversarial review (Blind Hunter, Edge Case Hunter, Acceptance Auditor) of the merged code
of PRs #211, #215, #219, #220/#221, #223, #230 (the UX pass, #213/#214, #217, #218, walk fixes),
checked against main 73a7b19. Decisions are Guy's (2026-09-25); every item is fixed through a
reviewed PR (G3) or listed under Defer.

- [x] [Review][Decision] Trigger sell: empty quantity + autofocus + Enter sells the whole position — **Enter requires a quantity; the whole-position sale stays a button click (pre-7.0 behaviour)** [app/ui/components/modal_dialog.slint:97,431]
- [x] [Review][Decision] Several studies per ticker (#81) collapse to « the latest study » in the position dropdown — **the dropdown lists studies (id carried), « TICKER · CUR » when a ticker has several** [app/src/wiring/holdings.rs:211-221, app/src/state/holdings.rs:211]
- [x] [Review][Decision] « Modifier » forces the study's currency (no conversion), refuses sector-only edits on ledger-backed rows, disables the form without a study — **Modifier never changes a holding's currency; same ticker → always editable; new ticker → a study in the holding's currency, else a named refusal** [app/src/state/holdings.rs:236-247]
- [x] [Review][Decision] Confirm verb disabled on a read-only dossier also blocks restore — **keep the block, name the reason in the dialog, add the guard in `confirm_restore`** [app/ui/components/modal_dialog.slint:277, app/src/state/restore.rs]
- [x] [Review][Decision] « Créer une étude » prefills the reference currency; no company-name field (spec §5.3) — **no prefill (« choisir », submit disabled until chosen), allow-list kept, optional « Nom de société » added** [app/ui/screens/dashboard.slint:259, modal_dialog.slint:488-506]
- [x] [Review][Decision] Réglages re-syncs a field after a refusal (spec AC1: keep the typed text) — **fix: keep the typed text** [app/src/wiring/prefs.rs:188,201]
- [x] [Review][Decision] #217 `/splits`: hard dependency with an unnamed error; a 200 non-array body skips rebasing silently; decimal ratios dropped; dividend / book per share no longer rebased on an unverified assumption — **failure stays hard but named; unreadable body = failure; decimal ratios accepted; one real NVDA.US fetch with Guy present (G5) settles the per-share point** [ingestion/src/adapters/eodhd.rs:83,251-267]
- [x] [Review][Patch] `refuse()` overwrites a parked confirm (pending study action / import / restore stay armed), routes unrelated async refusals into the open form's field error, overwrites an unread notice [app/src/wiring/dialog.rs:14-23]
- [x] [Review][Patch] Focus not trapped in the modal; dropdown-first forms (position, FX rate) get no initial focus → Esc dead, Enter re-fires the hidden opener [app/ui/components/modal_dialog.slint:193,291,514]
- [x] [Review][Patch] Dropdown: no keyboard selection, no max height / no scroll (tickers beyond the window unreachable) [app/ui/components/dropdown.slint:30-83]
- [x] [Review][Patch] Dividend transaction edit uses the buy/sell labels (« Frais (vide = 0) » vs the withholding default) [app/ui/screens/portfolio.slint:681, modal_dialog.slint:340-381]
- [x] [Review][Patch] « Renommer le portefeuille » opens empty (spec: prefilled) [app/ui/screens/portfolio.slint:135]
- [x] [Review][Patch] Deleting a portfolio with holdings confirms, then refuses; the refusal names no count (spec §5.1) [app/ui/screens/portfolio.slint:142, app/src/state/messages.rs:331]
- [x] [Review][Patch] No « Dossier en lecture seule » band in Portefeuille (buttons only greyed) [app/ui/screens/portfolio.slint]
- [x] [Review][Patch] Studies read failure shown as « aucune étude » in the position dialog (#95) [app/src/wiring/holdings.rs:211-221,861-882]
- [x] [Review][Patch] « Tout non classé » band compares list lengths (0 == 0 when concentration is unavailable) [app/ui/screens/portfolio.slint:385]
- [x] [Review][Patch] Portfolio rename with an unparsable id reports success; stop / sell return false without a cause [app/src/wiring/holdings.rs:138-145]
- [x] [Review][Patch] A refused dossier switch clears the open dossier's location status [app/src/wiring/journal.rs:163-166]
- [x] [Review][Patch] Adoption chips (#213/#214): no year shown, may come from a very old year, est-low EPS ≤ 0 proposed; candidate (a) has no positivity guard; (a)/(d) overflow names the wrong reason [app/src/viewmodel/engine.rs:380-381,448-470, core/src/ssg/risk_reward.rs:34-38]
- [x] [Review][Patch] Orphan confirm properties and dead code (`study-action-confirm-visible`, `import-confirm`, `restore-confirm`, `Dialog.notice`, `LabeledDropdown.changed`, `NARROW_NBSP` misnamed) [app/ui/state.slint:784,900,908]
- [x] [Review][Patch] Tests: #217 per-share rebasing coverage lost in `eodhd_mapping.rs`; no n=3 / fractional split test [ingestion/tests/eodhd_mapping.rs]
- [x] [Review][Defer] Focus taken through a 30 ms timer (Slint 1.17 workaround) [app/ui/components/action_button.slint] — deferred, cross-cutting
- [x] [Review][Defer] Same-day ledger rows ordered by entry time → a back-dated buy after a same-day sale is refused as an oversell [persistence/src/transactions.rs:464,516] — deferred, design of day-granular dates
- [x] [Review][Defer] Some states still plain text instead of bands (candidates panel, watchlist « Aucune étude liée », consolidation rows); success notices of Études in a StatusBand [portfolio.slint:201, watchlist.slint:123, dashboard.slint:264] — deferred, cosmetic

Fixed in PR E (G1, branch `fix/g1-e-dialogs-holdings`): every item checked above, and decisions 3
(the position dialog lists studies), 4 (« Modifier » never changes a holding's currency — a legacy
NULL currency stays NULL and follows the register's ticker-only link), 5 (read-only confirm names
its reason; `confirm_restore` refuses), 8 (no currency prefill; optional company name) and 9a
(Réglages keeps the typed text). The candidate (a) positivity guard was NOT made in core — it would
change a method formula (METHOD_VERSION bump) — and is left to Guy. The refused-switch status is in
the dossier-switch PR; the #217 tests in the EODHD PR.
Fixed in PR H (G1, branch `fix/g1-h-eodhd-splits`): decision 10 — a /splits failure stays a hard
failure but is named (403 / 429 / unreadable), an unreadable body or a malformed, duplicated or
future-dated split is a named failure (never « no splits »), decimal ratios are exact (rounded to
4 dp once when a split applies), the key test keeps its verdict (#42), and the #217 coverage is
restored with a real split in the fixture. Dividend / book value per share stay « as served »,
pinned on the assumption that EODHD restates the balance-sheet share counts — to be confirmed by
ONE real NVDA.US fetch with Guy present (G5).
Fixed in PR G (G1, branch `fix/g1-g-dossier-switch`): a refused switch recomputes the open
dossier's own status (never blanks it, never keeps a stale reclaim offer).

### Review Findings — G1 final review, area 6: study screen, dossier switch, EODHD (2026-09-25, #237)

Checked against `integ/g1-final` e686141; every item verified in the code before the fix.

- [x] [Review][Patch] M1 — A study fetch in flight across a dossier change applies to the new dossier (a restore keeps the ids: late provider data written into the restored dossier) — **each fetch stamped with a dossier generation that every dossier change moves on; a stale result is dropped unwritten and unsaid** [app/src/wiring/fetch.rs, app/src/wiring/journal.rs, app/src/fetch.rs]
- [x] [Review][Patch] M2 — The present price is not rebased when a split falls after the last bar [ingestion/src/adapters/eodhd.rs]
- [x] [Review][Patch] M3 — The trace panel and the scenario comparison survive a close and show over another study / the demo [app/src/wiring/studies.rs]
- [x] [Review][Patch] M4 — Validated ✓ cells show « ⋯ » at `grid-col-min` 80 (✓ box 34 + lock 34) — **G3 rework: two 12 px marker columns (?/✓ over the lock; △ over ◦), the lock always shown, constant geometry; `grid-col-min` 88 px (« 1 234,56 » whole with ✓ at 80, with ✓ + △/◦ at 88)** — the lead verifies on screen at 1 280 px [app/ui/components/editable_cell.slint, app/ui/tokens.slint]
- [x] [Review][Patch] M5 — A fetch result lands in a hidden study's slot when the reader is on another screen — **said in the study's slot AND the list's** [app/src/wiring/fetch.rs]
- [x] [Review][Patch] L8 — The study notice slot's comment contradicts the code (failures do land there) [app/ui/screens/study_screen.slint]
- [x] [Review][Patch] L9 — Close paths not all through `close-study`; judgment-hover not reset; `demo-active` survives a switch [app/src/wiring/studies.rs, app/src/wiring/journal.rs]
- [x] [Review][Patch] L10 — A failed switch that loses the previous dossier: the status still reads as an open dossier — **the path goes with the journal; the status says no dossier is open (MSG_NO_JOURNAL_OPEN)** [app/src/state/journal_io.rs, app/src/wiring/journal.rs]
- [x] [Review][Patch] L11 — A split dated today is refused around midnight (UTC vs local) — **the fetch day is the local calendar day** [ingestion/src/adapters/eodhd.rs]
- [x] [Review][Defer] L6 (tips hidden / clipped), L7 (« n/a » vs lock overlap), L12 (no horizontal-scroll affordance) — deferred to the layout debt G6

- [x] [Review][Patch] G3 #1 — The reclaim branch of a refused switch that lost the dossier kept its session [app/src/wiring/journal.rs]
- [x] [Review][Patch] G3 #2 — The holdings price refresh and the FX refresh were not stamped (a restore keeps the ids) — **stamped; stale results dropped silently; the batch resets on a dossier change** [app/src/fetch.rs, app/src/wiring/{fetch,holdings,fx,journal}.rs]
- [x] [Review][Patch] G3 #3 — `fetch_latest_price` served an unrebased close — **reads `/splits`; an unreadable history is a named failure** [ingestion/src/adapters/eodhd.rs]
- [x] [Review][Patch] G3 #6 — Other open rails emptied a pending fetch result; the list slot ignored F4 [app/src/wiring/study_notice.rs]
- [x] [Review][Patch] G3 #7/#8 — The fetch-day doc misnamed the early case; a present price with an unreadable bar date [ingestion/src/adapters/eodhd.rs]
- [x] [Review][Patch] G3 #9/#10/#11 — « Historique » survived a close; a failed restore that lost the dossier; `fetching` across a dossier change + tests [app/src/wiring/{studies,journal}.rs, app/src/state/restore.rs]

Fixed in PR O (G1 final, branch `fix/g1-o-study-dossier`): every item checked above except the
deferred ones. To watch: the holdings price refresh now makes two EODHD requests per ticker
(`/eod` + `/splits`) — a plan without `/splits` refuses the refresh, named.

### Review Findings — G1 final review, area 1: dialogs, portfolio, input (2026-09-25, #237)

Decisions are Guy's (2026-09-25).

- [x] [Review][Decision] H1 « Retirer » on a position with ledger transactions confirmed a ledger deletion, then was refused with English text under « L'enregistrement a échoué » — **refused UP FRONT, no confirm; the refusal names the cause in French (its transactions must be deleted first); typed check (`holding_has_transactions`), never string matching** [app/ui/screens/portfolio.slint, persistence/src/holdings.rs, app/src/state/mod.rs]
- [x] [Review][Decision] M2 the ledger « Vente » promised « vide = toute la position » while the form and the rail refused an empty quantity — **promise removed; the quantity is required and its absence names itself** [app/ui/components/modal_dialog.slint, app/src/state/ledger.rs]
- [x] [Review][Patch] M3 portfolio rate notes (and the candidates' rates) spelled a raw « 0.8 » [app/src/wiring/holdings.rs, app/src/wiring/replacement.rs]
- [x] [Review][Patch] M4 ledger rails named « quantité et prix d'achat… aucune position » for fees, withholding, gross, overflow [app/src/state/ledger.rs]
- [x] [Review][Patch] M5 read failures shown as empty states: holdings, sold positions, portfolios, ledger, watchlist, FX rates [app/src/wiring/{holdings,watchlist,fx}.rs]
- [x] [Review][Patch] L6 watchlist link read failure → « aucune étude »; « Étude : » dangling [app/src/wiring/watchlist.rs]
- [x] [Review][Patch] L7 trigger sell / trailing stop silently used the cost basis when the study read failed [app/src/state/{ledger,holdings}.rs]
- [x] [Review][Patch] L8 a case-only ticker edit cleared the trailing stop [persistence/src/holdings.rs]
- [x] [Review][Patch] L9 trigger-sell Enter on a non-number had no visible effect [app/src/wiring/holdings.rs]
- [x] [Review][Patch] L10 the ledger-backed guard compared strings (« 5,0 » vs « 5 ») [app/src/state/holdings.rs]
- [x] [Review][Patch] L11 an edit / a portfolio switch cleared the in-flight refresh banner (F4) [app/src/wiring/holdings.rs]
- [x] [Review][Decision] L12 a restore proceeded without its safety snapshot / ignored a checkpoint failure — **refused, naming the cause** [app/src/state/restore.rs]
- [x] [Review][Patch] L13 raw English error text appended to French refusals (`watch_error`, restore) [app/src/state/{mod,restore}.rs]
- [x] [Review][Patch] L14 ledger display order differed from the replay order on a full tie [persistence/src/transactions.rs]
- [x] [Review][Patch] L15 this record carried the G1 catch-up items twice, checked and unchecked — merged
- [x] [Review][Patch] (from the review branch's G3) a legacy lot without a declared currency had its stop compared to / ratcheted by a price of unknown currency [app/src/wiring/holdings.rs, app/src/state/holdings.rs]

Fixed in PR K (G1, branch `fix/g1-k-portfolio-ledger`): every item above. Beyond the letter: a
failed rollback after a restore keeps the pre-restore snapshot on disk (it is then the only copy);
a legacy lot's stop is seeded from its cost basis; « Lier une étude » names an unreadable
watchlist. Not in scope and left as found: `watch_error` still words a failed READ on the write
rails as « L'enregistrement a échoué. »; the reinvestable-cash panel still swallows its read;
a legacy lot's trigger-sale price still comes from its ticker-only study.

Follow-ups in PR P (G1, branch `fix/g1-p-followups`, on `integ/g1-final2`):
- [x] D5 (Guy, 2026-09-25): a lot without a declared currency is presumed in the reference currency — ONE rule (`state::stop_basis`) for the register (display, trigger, ratchet, seed), the review screen + its PDF and the trigger sale; against a study in another currency the stop is not compared, both facts stated; the trigger sale refuses that study's price by name
- [x] G3 M1: a restore never replaces nor deletes an earlier `-prerestore` copy (refused, named; the snapshot is created new); a failed rollback says the dossier WAS replaced and names the kept snapshot; a path without an open handle is refused (the checkpoint is never skipped)
- [x] G3 L1–L6: the in-flight banner re-set by name; a deleted watched study worded as an absence; corrupt drafts / missing positions named; no default portfolio and no duplicate watch on a failed read; the SQL ticker compare trims; tests for a NULL-kind legacy sale and a dividend at full tie
- [x] K residues: a write rail's failed READ says « Le dossier n'a pas pu être lu » (`read_error`); the reinvestable dividends say « indisponible » on a failed read
- [x] The quick examination's list notice and the startup state go through `list_notice` (F4). The create-study and demo clears of `Studies.notice` stay outside; `progress` / `clear` keep their `dead_code` marker (still no writer).
