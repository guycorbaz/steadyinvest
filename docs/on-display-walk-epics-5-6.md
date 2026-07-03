# On-display verification walk — Epics 5 + 6 batches (retro gates F1 + F6)

**Who:** Guy, on a real display (the sandbox cannot walk these). **When:** before the first
Epic-7 UI story. **How:** launch the app (`cargo run -p steadyinvest-app` from `main`), tick each
line, and file a GitHub issue for ANY gap (one issue per gap, however small). Checked = seen
working with your own eyes, not "probably fine".

## A. Carried Epic-5 batch (retro E1, never walked)

- [ ] **5.1 Confront** — open a past study → « Confronter » : the recorded projection band + the
  actual price trajectory render; close cleanly.
- [ ] **5.2/5.3 Export/Import (Réglages)** — export one study; export the whole journal; re-import
  each into a scratch journal; counts + notices correct.
- [ ] **5.4 Backup/Restore** — create a backup; restore it; the confirm banner shows the
  (journal_id, version) and stale/foreign warnings when applicable.
- [ ] **5.5 Journal location** — the RECENT-JOURNALS list; switch journals; the single-instance
  lock; **the rfd NATIVE PICKER dialogs** (open + create — never verified headless: no
  xdg-desktop-portal in the sandbox); the sync-folder warning if you pick a Drive-watched path.
- [ ] **5.6 PDF** — « Exporter PDF » from a study; open the file; greyscale-readable; all
  sections expanded.

## B. Epic-6 batch

- [ ] **6.1 Portfolios** — add a second bank; the selector chips; rename; the guarded deletes
  (has-holdings refusal, last-portfolio refusal).
- [ ] **6.2 Currency** — the holding currency picker (allow-list only); per-row currency labels;
  per-currency CaR lines never mixing currencies.
- [ ] **6.3 Ledger** — open a holding's Transactions; record a buy, a partial sell (position
  stays, average recomputes), edit and delete a row; the oversell refusal.
- [ ] **6.4 Dividende** — record one with an empty « Retenue » (auto 35 %); the net line; the
  reinvestable-cash block updates.
- [ ] **6.5 FX (Réglages)** — manual rate entry (bad rate/date refusals); « Actualiser les
  taux » fetches one pair per foreign currency in use.
- [ ] **6.6 Consolidation** — with two banks + a foreign holding: per-bank lines, « Total
  global », the rates footnote; delete the rate → the named « taux manquant » refusal, global
  absent; re-add → it consolidates without a restart.
- [ ] **6.7 Concentration** — the per-security shares (one line for a ticker held at two banks);
  the murmur at/near the threshold; the size mix vs targets; a « non classé » line with its
  reason; the Réglages threshold + table panel (bad input refusals keep your text).
- [ ] **6.8 Candidats** — sell a holding (partial AND whole): the panel auto-opens headed
  « Vente enregistrée : … »; open it from a trigger's « Candidats » (header must NOT say vente);
  ordering (in-zone first); « Ouvrir l'étude » lands on the study; « Fermer »; switch journal →
  panel gone.
- [ ] **6.9 Repli (Réglages)** — the three fallback rows; the fundamentals row has NO Twelve
  Data chip; pick a fallback = the current primary → the chip shows « Aucun repli » (the
  effective chain dedups).
- [ ] **#94 fix** — edit a study's judgment (zone flips), navigate to Portefeuille: the zone
  chip/candidates are CURRENT on arrival (no unrelated mutation needed).

## C. Live-keys session (retro F6 — network + real keys)

- [ ] **EODHD end-to-end** — key saved + « Tester » OK; a study fetch fills cells; a holdings
  price refresh stamps « à jour ».
- [ ] **Twelve Data end-to-end** — same (remember its symbol convention differs: `NESN` not
  `NESN.SW` — issue #70; note what you observe).
- [ ] **Forced fallback** — set the price fallback, then DELETE the primary's key: a price
  refresh must serve via the fallback WITH the notice « Données obtenues via … (fournisseur de
  repli). » (the 6.9 CRITICAL fix — this must never be silent).
- [ ] **Pacing** — a Twelve Data refresh of >8 linked tickers: no quota error (requests spaced
  7.5 s — the batch takes ~1 min; note the UI feedback gap, issue #100).
- [ ] **Quota reality (#80)** — if you hit a quota, note whether the provider actually sends a
  retry-after (the adapters never populate `retry_after_secs` today).
- [ ] **FX live** — « Actualiser les taux » on both providers; the stored rows carry the right
  source wire name.

## Outcome

- Every unchecked line at the end of the walk = a filed issue.
- When the walk is done, note it in the sprint-status history — this gate then stops carrying
  from retro to retro (E1 slipped three times; F1 is blocking for Epic 7's UI stories).
