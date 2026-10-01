# Revue SteadyInvest — P1 : exigences non fonctionnelles, contraintes de domaine, critères de succès, parcours, exigences propres au bureau

Référence : `_bmad-output/planning-artifacts/prd.md` (1104 lignes), branche de travail `feat/validate-year` (= `main` + validation d'une année entière). Revue en lecture seule : aucun build, aucun test lancé, pas d'interface graphique, pas de réseau. Les preuves viennent du code, des tests (par leur nom et leur emplacement), des fiches de story, des rétrospectives et de `git log`.

Échelle : **Tenu** · **Tenu avec écart** (l'essentiel est là, avec une réserve précise) · **Partiel** · **Non tenu** · **Non vérifiable** (rien dans le dépôt ne permet de trancher).

---

## 1. Exigences non fonctionnelles (30)

### 1.1 Exactitude et intégrité du calcul

| Id | Exigence (résumé) | État | Preuves |
|---|---|---|---|
| NFR-C1 | Moteur déterministe, stable au bit près d'une exécution et d'une plateforme à l'autre | **Tenu avec écart** | Test de hachage épinglé `core/src/lib.rs:104` `determinism_hash_matches_cross_os_contract`, exécuté par une étape dédiée de la CI (`.github/workflows/ci.yml`, étape « Determinism hash ») ; arithmétique `rust_decimal` exacte ; proptest `determinism_and_no_panic` (`core/tests/ssg_metamorphic.rs:252`). **Écart :** la CI tourne sur Linux seul (`ci.yml:27` `os: [ubuntu-latest]`, décision du 2026-06-09) : la stabilité « d'une plateforme à l'autre » n'a jamais été observée. |
| NFR-C2 | Égalité avec chaque étude de référence embarquée (zonage et verdict exacts, ±0,5 %) | **Tenu** | 11 fixtures `core/tests/golden/g01…g11*.json` ; porte CI `core/tests/golden_gate.rs:75` `every_bundled_golden_passes_the_self_check` + tests de contrôle (tolérance, zone altérée, version de méthode périmée : l. 128–227) ; copie identique au bit près vérifiée côté app (l. 262). Réserve : toutes les fixtures sont synthétiques (`core/tests/golden/README.md`) ; aucune « fiche SSG remplie obtenue » comme le prévoit la section Domaine (voir §2). |
| NFR-C3 | Invariants par propriétés : bornes de zones ordonnées, U/D ≥ 0, CaR ≥ 0, aller-retour FX A→B→A à 1e-6 près | **Tenu avec écart** | `zones_are_strictly_ordered_whenever_present` (`core/tests/ssg_metamorphic.rs:269`), `ud_is_nonnegative_for_in_range_prices` (l. 286), `capital_at_risk_is_non_negative…` (`core/src/risk/mod.rs:263`), proptest du registre `derivation_is_total_deterministic_and_never_negative` (`core/src/risk/ledger.rs:376`). **Écart :** aucun test d'aller-retour FX. La conception refuse d'ailleurs l'inversion d'un taux (`persistence/src/fx.rs:8` : « 1/rate is refused ») : l'invariant, tel qu'il est écrit, n'a donc pas d'objet. |
| NFR-C4 | FX appliqué seulement à la consolidation | **Tenu** | `core/src/risk/fx.rs:1-13` : une seule multiplication vérifiée, appelée seulement aux points de consolidation (6.6, 6.7, 6.8) ; `core::ssg` n'importe rien de `risk` (aucune occurrence de `fx` ni de `risk::` dans `core/src/ssg`). |
| NFR-C5 | Moteur et risque verrouillés en CI par les tests de référence et de propriétés (objectif ≥ 95 % de couverture) ; un test en échec bloque la fusion | **Partiel** | La CI exécute `cargo test --all --locked` et les portes de référence (`ci.yml`). **Aucun outil de couverture** : la story 1.9 le reconnaît (`1-9-…md:277` « NFR-C5's ≥95% target has no tooling in CI yet »). Le blocage de la fusion dépend de la protection de branche GitHub, invisible depuis le dépôt. Incident de la rétro 8 : le commit `bb85ecf` est entré avec un test de posture en échec, masqué par un pipeline `grep`, puis a été corrigé par `1c9e6d0` (`epic-8-retro-2026-09-30.md` §2.5). |

### 1.2 Performance

| Id | Exigence | État | Preuves |
|---|---|---|---|
| NFR-P1 | Recalcul et recoloration en direct (~100 ms) pendant le glissement | **Tenu** | Spike B : 40 à 60 µs en typique, 235 µs au maximum, sur 660 événements (`1-5-…md`, `docs/spikes/spike-b-native-slint-chart.md`) ; chemin direct sans persistance (`app/src/wiring/judgment.rs:336`, `wiring/push.rs:284`) ; glissement piloté par Guy sur son écran (`2-8-…md:225`). |
| NFR-P2 | Ouverture ou recalcul d'une étude complète en ~1 s | **Non vérifiable** | Aucune mesure : l'exigence est seulement argumentée (« orders away for pure decimal math », `1-8-…md:99`) ; aucun benchmark. |
| NFR-P3 | Rafraîchissement manuel du portefeuille (quelques dizaines de lignes) en quelques secondes, sans bloquer l'interface | **Tenu avec écart** | Hors du fil d'interface : worker + `invoke_from_event_loop` (`4-4-…md:37,97`). **Écart :** le cadencement 6.9 impose 7 500 ms entre deux requêtes Twelve Data (`ingestion/src/fetch.rs:63-65`). Vingt positions servies par ce fournisseur prennent donc environ 2 min 30, et non « quelques secondes ». Durée jamais mesurée ; quota réel jamais observé (G8 reporté). |
| NFR-P4 | Application interactive en ~3 s après le lancement | **Tenu avec écart** | 0,12 s mesuré sur un build debug en story 2.1 (`2-1-…md:390`, juin 2026). Jamais remesuré depuis, alors que le lancement exécute désormais les migrations jusqu'à v9 et la relecture périodique de la boîte de propositions. |

### 1.3 Sécurité et confidentialité

| Id | Exigence | État | Preuves |
|---|---|---|---|
| NFR-S1 | Clés uniquement dans le trousseau de l'OS ; jamais dans le dépôt, la configuration, les journaux, les exports, les sauvegardes ou les réponses MCP | **Tenu avec écart** | `app/src/keychain.rs:1-12` (seul module qui touche `keyring`, erreur sans champ, `zeroize`) ; `mcp/tests/non_exposure.rs:111` `without_dossier_argument_no_config_value_or_key_reaches_stdout` ; `.gitignore` (`*.key`, `.env`). **Écarts :** (a) repli sur la variable d'environnement `STEADYINVEST_EODHD_API_KEY` quand le trousseau est indisponible (`app/src/wiring/fetch.rs:26,42`) ; (b) `keyring` n'est compilé qu'avec `async-secret-service` (`Cargo.toml:82`). Sous Windows et macOS, la crate retombe alors sur son magasin **mock**, non persistant (`keyring-3.6.3/src/lib.rs:296-297`). |
| NFR-S2 | Aucune télémétrie ; seuls appels réseau : les récupérations lancées par l'utilisateur ; MCP non exposé au réseau | **Tenu** | `reqwest` n'apparaît que dans `ingestion` et `app/src/fetch.rs` ; aucun écouteur TCP ; serveur MCP en stdio (`mcp/`, `docs/mcp-registration.md`) ; FR65 : aucune interrogation périodique (`4-4-…md:16`). |
| NFR-S3 | Données locales ; les données d'étude ne sortent que par le client IA ; le portefeuille jamais par MCP | **Tenu** | SQLite local ; périmètre MCP restreint (voir S4/A2). Le résidu admis (texte libre) est documenté dans le PRD. |
| NFR-S4 | MCP ne renvoie jamais portefeuille, liste de suivi, clés ou configuration (tests sur toute la surface) | **Tenu** | `mcp/tests/non_exposure.rs:13` `no_tool_on_any_page_exposes_portfolio_watchlist_cache_or_config_data` ; `persistence/tests/mcp_access.rs:295` `no_read_returns_portfolio_watchlist_or_cache_data` ; autoriseur SQLite (`persistence/src/mcp_access.rs:802`). |

### 1.4 Fiabilité et intégrité des données

| Id | Exigence | État | Preuves |
|---|---|---|---|
| NFR-R1 | Étude et revue de risque complètes hors ligne ; seul le fetch se dégrade, avec marquage périmé | **Tenu** | 2.2 AC 6 (« fully offline », `2-2-…md:81`) ; 3.3 fraîcheur ; 3.5 marquage `Freshness::Stale` et conservation de la dernière valeur (`3-5-…md:48,70`) ; taux FX datés (6.5). |
| NFR-R2 | Écritures atomiques et résistantes aux pannes ; brouillons MCP atomiques, sans course avec la session | **Tenu** | `persistence/tests/journal_roundtrip.rs:329` `interrupted_write_leaves_prior_version_and_no_partial_row` ; `migrations.rs:139` (rollback) ; `config.rs:1048` (rename atomique) ; `mcp_access.rs` tests l. 781 et 1303 (course avec une suppression, retentatives concurrentes). Remarque hors journal : les exports PDF écrivent sur place, sans fichier temporaire ni garde contre l'écrasement du dossier ouvert (`deferred-work.md`, section G1 Epic 7). |
| NFR-R3 | Migrations sûres vers l'avant ; un ancien journal s'ouvre sans perte | **Tenu** | Corpus figé `persistence/tests/corpus/v1.db`, `v8.db` + `corpus_gate.rs` ; `readonly_newer.rs:68,115` (un fichier plus récent s'ouvre en lecture seule, sans migration). Nuance : la décision de Guy du 2026-09-27 (« pas en production », mémoire du projet) réduit l'effort de compatibilité aux champs serde additifs. |
| NFR-R4 | La réconciliation ne détruit jamais une valeur manuelle ni un jugement ; la valeur fournisseur est gardée à côté | **Tenu** | Story 3.4 ; `app/src/state/refresh.rs:65,507` (une cellule ✓ est figée, la valeur divergente est mise en attente) ; tests `app/src/state/tests.rs:1603,1695`. |
| NFR-R5 | Export, import et restauration vérifient l'intégrité et la version ; un fichier non conforme est rejeté en entier | **Tenu avec écart** | 5.2–5.4 (intégrité, `schema_version`, identité, échange temp+rename, rollback : `5-4-…md:3`) ; arbitrage de version par l'appelant (`persistence/src/export.rs:193`) ; `drafts.rs:614,665` (import malformé, rien appliqué). **Écart :** #67 est toujours différé : une copie brute externe sans son `-wal` perd en silence les commits non pointés (`deferred-work.md`, 5.4). |

### 1.5 Portabilité

| Id | Exigence | État | Preuves |
|---|---|---|---|
| NFR-X1 | Comportement et résultats identiques sous Windows, macOS et Linux | **Non tenu** | CI Linux seule (`ci.yml:27`, commit `6116cc6`) ; rétro 8, H6 : « Windows path never compiled or tested » ; trousseau « mock » hors Linux (voir S1). L'architecture acte le « Linux-only for now » (`architecture.md:322,478,846`) ; le PRD n'a pas été réconcilié. |
| NFR-X2 | Nombres selon les conventions locales (virgule décimale, milliers), réglables indépendamment de la locale de l'OS | **Tenu avec écart** | `app/src/viewmodel/format.rs:26-30` (`Comma` par défaut / `Point`), `parse_decimal` l. 321 (espaces et apostrophes suisses comme séparateurs de milliers) ; réglage dans Réglages. **Écart** avec la section Bureau (« follows OS locale ») : la locale de l'OS n'est jamais lue ; le défaut est fixe. |
| NFR-X3 | Fichier journal portable d'une plateforme à l'autre | **Non vérifiable** | Un fichier SQLite est portable par nature, mais aucune ouverture n'a été faite hors Linux. Le verrou d'instance et l'identité de fichier sous Windows (`same-file`, 8.3) n'ont jamais été compilés (H6). |

### 1.6 Ergonomie et accessibilité

| Id | Exigence | État | Preuves |
|---|---|---|---|
| NFR-U1 | Zones distinguables sans la couleur seule | **Tenu** | `app/ui/components/zone_bar.slint:7-8` (teinte + valeur + position + libellé) ; `nav_item.slint:1`, `ai_judgment.slint:5` (trois indices non colorés), `frozen_verdict_strip.slint:5` ; PDF « black-and-white first » (`report/src/pdf.rs:11`). |
| NFR-U2 | Parcours d'étude et de saisie entièrement au clavier | **Tenu avec écart** | Champ de valeur exacte synchronisé avec le glissement (2.8 AC 3/7), `action_button.slint:2`, `collapsible_section.slint:5`. **Écarts :** la bande de glissement annonce `accessible-role: slider` mais ne réagit pas aux flèches ; l'annulation au clavier (2.9) n'a pas de couverture automatisée et l'ordre de focus n'a pas été vérifié (`deferred-work.md` 2.8/2.9) ; aucun harnais de test d'interface (H7). |
| NFR-U3 | Mises en page (écran et impression) proches du formulaire d'origine, libellés neutres | **Tenu avec écart** | Formulaire SSG repliable (2.3), PDF (5.6, 7.5) ; test anti-marques dans le PDF (`report/src/pdf.rs:3899`). **Écart :** G9 / #207 (comparer le PDF NVDA au formulaire NAIC) n'est pas fait (rétro 7 l. 70, rétro 8 §3). |

### 1.7 Maintenabilité

| Id | Exigence | État | Preuves |
|---|---|---|---|
| NFR-M1 | UI mince au-dessus d'un crate de calcul testé, indépendant de l'UI, et d'un contrat versionné découplé de Slint et du stockage | **Tenu avec écart** | `core` et `contract` ne dépendent ni de `slint` ni de `rusqlite` (Cargo.toml). **Écart de proportion :** `app/src` compte 52 866 lignes de Rust (plus 12 722 de `.slint`), contre 8 377 pour `core` et 9 737 pour `persistence`. Les modules `state/` et `viewmodel/`, testables mais logés dans le crate UI, rendent le qualificatif « mince » discutable. |
| NFR-M2 | `schema_version` explicite ; toute rupture livre une migration | **Tenu** | `contract` (`schema_version`, `method_version`) ; migrations v1→v9 ; tests `contract/tests/roundtrip.rs`, `fingerprint.rs`. |

### 1.8 Asymétrie des capacités IA [P4]

| Id | Exigence | État | Preuves |
|---|---|---|---|
| NFR-A1 | Seule écriture MCP : la création de brouillon ; tout autre écriture est rejetée et journalisée (suite CI) | **Tenu** | Autoriseur SQLite (`persistence/src/mcp_access.rs:802`) + test `every_other_statement_is_denied_by_the_engine_and_the_dossier_is_unchanged` (l. 1872) ; journal `tracing::warn!(… "write denied")` (`mcp/src/tools.rs:452`) et « unknown tool » (l. 481) ; trigger v9. |
| NFR-A2 | Non-exposition du portefeuille par construction (test sur toute la surface) | **Tenu** | Voir S4 ; fermeture des dépendances (`mcp/tests/closure.rs:82`). |
| NFR-A3 | Aucun appel fournisseur atteignable depuis MCP (testé) | **Tenu** | `mcp/tests/closure.rs:82` `the_dependency_closure_excludes_every_network_keychain_and_gui_crate` + contrôle positif l. 99. |
| NFR-A4 | Chaque brouillon porte son origine (client + modèle), un horodatage et un commentaire non vide | **Tenu** | `persistence/src/schema.rs:166-168` (CHECK `length(trim(…)) > 0` sur `comment`, `origin_client`, `origin_model`) ; `persistence/tests/drafts.rs:194` `every_check_refuses_its_row_and_writes_nothing`. |

---

## 2. Exigences propres au domaine et contraintes

| Section | État | Preuves et écarts |
|---|---|---|
| Posture réglementaire : faits, jamais de recommandation ; verbes bannis (FR13) ; avertissement sur chaque page, boîte de propositions comprise | **Tenu** | Listes canoniques `core/src/method/mod.rs:115` (16 EN) et `:135` (10 FR) ; portes : `app/src/posture.rs:425` `all_user_facing_app_strings_are_neutral`, `:469` `user_facing_strings_state_facts_not_advice`, `:550`, `:407` (aucun littéral hors `@tr`), plus des portes par domaine (notes, brouillons, décisions, verdict figé, lignes IA : l. 824–1723) ; mêmes contrôles dans `mcp/src/tools.rs:814`, `ingestion/src/error.rs:111` et `report/src/comparison.rs:736`. Avertissement épinglé hors défilement (`app/ui/app.slint:173-186`), sur chaque page PDF (`report/src/pdf.rs:1665,2778`) et dans chaque cadre IA (`ai_frame.slint:126`). Les actions de 4.7 sont des noms (« Vente… », « Relever le stop… »). |
| Fidélité à la méthode (références, invariants, plancher de 5 ans, drapeau « validé », FX natif) | **Tenu avec écart** | Voir C1–C4 ; plancher de 5 ans et faible confiance (2.7, fixture g08) ; tri-état ✓ (2.5). **Écarts :** références seulement « calculées à la main », aucune « fiche SSG remplie obtenue » ; le drapeau `high_debt` est au catalogue mais **ne peut pas être levé** faute de champ dette (`docs/method/ssg-method-spec-v1.md:234`, `core/src/ssg/types.rs:153` : 9 drapeaux levables, sans la dette) ; G9 (#207) pas fait. |
| Licences des données et CGU des fournisseurs (aucune donnée fournisseur dans le dépôt, fixtures synthétiques, clé de l'utilisateur, dépôt privé) | **Partiel** | **Des extraits réels d'EODHD sont suivis par git** : `ingestion/tests/fixtures/eodhd-fundamentals-AAPL-real.json` et `…-JPM-real.json`, présentés comme « trimmed extracts of that fetch » (`ingestion/tests/eodhd_mapping.rs:390-403`, commit `806f423` du 2026-09-27). Ce sont des chiffres publics de rapports annuels, peu volumineux, mais ils contredisent la règle « synthetic only ». Le caractère privé du dépôt n'est pas vérifiable hors ligne (remote `github.com/guycorbaz/steadyinvest`). |
| Propriété intellectuelle et marques (libellés neutres dans une couche interchangeable, avis « projet indépendant, non affilié », GPL-3.0 sous réserve d'audit) | **Partiel** | Couche de libellés NAIC ↔ neutre (`app/src/labels.rs`), mais **4 entrées seulement** (l. 48) et **NAIC par défaut** (l. 13-17 : « SSG (Stock Selection Guide) » à l'écran) ; test anti-marques dans le PDF (`report/src/pdf.rs:3899`) ; GPL-3.0 et `cargo deny` en CI (`deny.toml`). **Écarts :** (a) **10 PDF NAIC/BetterInvesting protégés suivis par git** dans `docs/NAIC/` (handbook, tutoriels, formulaires), exactement l'« expression protégée » que le PRD demande d'écarter ; (b) **aucun avis « non affilié » dans l'application** : il ne figure que dans le README. |
| Sécurité et confidentialité (clés dans le trousseau, local et hors ligne, exposition IA, fichier unique copiable) | **Tenu avec écart** | Voir S1–S4 (repli sur variable d'environnement ; trousseau Linux seulement). |
| Audit et traçabilité (source, provenance, validé et horodatage par cellule ; motivation ; contrat versionné ; origine IA jusqu'à la prochaine modification) | **Tenu** | `contract/src/cell.rs` (provenance, `Coverage` l. 48-55), 2.10 motivation, 8.2b origine IA, `report/src/pdf.rs:153-155` (marque « † »), Registre 8.7. |
| Contraintes techniques (Rust + Slint, graphique natif, SQLite, hors ligne, pas de serveur réseau hormis MCP local) | **Tenu** | Graphiques `Path`/`TouchArea` (`growth_chart.slint`, `pe_history_chart.slint`) ; `rusqlite` intégré ; MCP en stdio. |
| Contraintes juridiques (GPL-3.0 + audit, pas de données fournisseur, pas de marques NAIC) | **Partiel** | Même constat que les deux lignes précédentes (PDF NAIC, extraits réels). Nuance de licence : les crates Slint déclarent `GPL-3.0-only` (`deny.toml:13`) alors que l'espace de travail annonce `GPL-3.0-or-later` ; le binaire combiné est de fait sous GPL-3.0 uniquement. |

---

## 3. Critères de succès

### 3.1 Succès utilisateur

| Critère | État | Preuves |
|---|---|---|
| Étude fiable en quelques minutes, et achevable par saisie manuelle quand la couverture est partielle | **Tenu avec écart** | Saisie manuelle de première classe (2.4, collage, `Coverage`) ; fetch EODHD et Twelve Data (3.1, 7.4) ; validation d'une année d'un seul geste (branche `feat/validate-year`, commit `81258d5`). Durée jamais chronométrée. |
| Zonage, U/D et projection à 5 ans corrects ; drapeaux de qualité automatiques (marges en baisse, dette élevée, ROE faible) | **Tenu avec écart** | Références et propriétés (C2/C3). **Le drapeau « dette élevée » ne peut pas être levé** (voir §2). |
| Rejouer à tout moment le raisonnement (jugements, entrées, provenance, motivation, notes) | **Tenu** | 5.1 (confrontation en lecture seule), 2.10, 8.1, historique de l'étude ; verdict figé 8.8. |
| Capital à risque d'un coup d'œil ; alertes neutres (zone d'achat, stop) | **Tenu** | 4.2, 4.6, 6.6, 4.7. |
| Moments « aha » (recoloration en direct ; relire une étude de deux ans) | **Tenu avec écart** | Recoloration vérifiée par Guy (2.8). La relecture d'une étude ancienne n'a pas pu être éprouvée : les données ont moins de quatre mois. |

### 3.2 Succès du projet

| Critère | État | Preuves |
|---|---|---|
| Adoption comme outil unique pendant 6 à 12 mois | **Non vérifiable** | Hors production (mémoire du projet, 2026-09-27) ; Epic 8 jamais testé par Guy (rétro 8, H2). |
| Soutenable et maintenable (UI mince sur un cœur testé) | **Tenu avec écart** | 1 454 tests (selon l'appelant) ; voir NFR-M1 pour la proportion du crate `app`. |
| Prêt pour l'open source (GPL appliquée, libellés neutres, fixtures synthétiques, aucune donnée fournisseur) | **Partiel** | GPL et `deny` en place ; mais PDF NAIC suivis par git, extraits EODHD réels, libellés NAIC par défaut, README périmé (« Status: Pre-implementation », `README.md`). |
| Aucun objectif de revenu ou d'audience | **Tenu** | Aucun élément contraire. |

### 3.3 Succès technique

| Critère | État | Preuves |
|---|---|---|
| Moteur déterministe, 100 % des références, propriétés, CI verte | **Tenu** | Voir C1/C2/C5. |
| Justesse FX (natif, conversion à la consolidation, aller-retour idempotent) | **Tenu avec écart** | Aller-retour non testé (voir C3). |
| Fonctionnement hors ligne ; provenance, source et horodatage par cellule | **Tenu** | R1 ; `contract/src/cell.rs`. |
| Graphique interactif (< ~100 ms, geste et valeur exacte, annulation) | **Tenu** | P1 ; 2.9 annuler/rétablir. |
| Contrat versionné découplé de Slint et SQLite | **Tenu** | M2. |
| Builds multiplateformes (Windows, macOS, Linux) | **Non tenu** | X1. |

### 3.4 Résultats mesurables

| Critère | État | Preuves |
|---|---|---|
| Étude en moins de ~5 min avec une bonne couverture | **Non vérifiable** | Aucune mesure. |
| 100 % des références ; propriétés vertes | **Tenu** | `golden_gate.rs:75`. |
| Étude à 0 % de couverture achevable de bout en bout | **Tenu** | 2.2 à 2.6 (tout est saisissable) ; 2.13 démonstration. |
| Étude complète et revue de risque sans réseau | **Tenu** | R1 ; 2.2 AC 6. |
| Capital à risque visible et recalculé à chaque rafraîchissement | **Tenu** | 4.6 / 6.6 (recalcul au rafraîchissement 4.4). |
| Adoption à 100 % des décisions, chacune rejouable | **Non vérifiable** | Voir 3.2. |

### 3.5 Résultats de l'assistance IA (Epic 8)

| Critère | État | Preuves |
|---|---|---|
| 100 % des écritures MCP hors boîte rejetées et journalisées | **Tenu** | A1. |
| Aucun accès au portefeuille, à la liste de suivi, aux clés ni à la configuration | **Tenu** | S4/A2. |
| Aucun appel fournisseur depuis MCP | **Tenu** | A3. |
| Un brouillon en attente ne change aucune sortie (test métamorphique) | **Tenu** | `app/src/viewmodel/verify.rs:382` `pending_drafts_of_every_kind_change_no_computed_output_of_any_fixture_study`, `:503`. |
| 100 % des brouillons avec commentaire et origine | **Tenu** | A4. |
| Décision en ≤ 2 actions, valeurs actuelle et proposée côte à côte | **Tenu** | 8.5b AC 1 (`8-5b-…md:12,21`). |
| Une étude issue d'un brouillon suit le chemin normal (fetch par le propriétaire, puis lecture IA) | **Tenu avec écart** | 8.7 (test MCP de bout en bout) ; le fetch réel de Journey 6 n'a jamais été exécuté (stub ; checklist H2 §6 « jamais vu »). |
| Toute surface qui montre un texte IA porte le libellé IA et l'avertissement (testé) | **Tenu** | `app/src/posture.rs:1903` `ai_written_text_is_read_only_inside_an_ai_frame`, `:2010` ; `ai_frame.slint:126` ; PDF : « † » + note, avertissement en pied de page. |

---

## 4. Parcours utilisateur

| Parcours | État | Étapes présentes | Étapes absentes ou différentes |
|---|---|---|---|
| **J1** — Nouvelle étude, bonne couverture | **Tenu avec écart** | Création et sauvegarde (2.2), fetch (3.1, 7.4), grille fidèle (2.3), zones, U/D et projection (2.6), drapeaux (2.7), motivation (2.10), glissement et recoloration (2.8). | Seules les lignes **BPA haut et BPA bas** se glissent (`growth_chart.slint:132-138`, `est_high_eps`/`est_low_eps`) ; la croissance des ventes se saisit en valeur, pas par un geste (le PRD dit « drags the future sales/EPS growth lines »). « Debt low » : aucun drapeau dette. |
| **J2** — Petite capitalisation CH/UE, couverture partielle | **Tenu** | Trois états de couverture (`contract/src/cell.rs:48-55`), saisie et collage (2.4), provenance manuelle, ✓ par cellule (2.5), plancher de 5 ans et faible confiance (2.7), calcul en devise native, plausibilité (erreur d'échelle). Désormais aussi : validation d'une année entière (branche). | — |
| **J2b** — Mise à jour annuelle | **Tenu avec écart** | Re-fetch et réconciliation (3.4, 3.6), manuel et jugements conservés, prolongation de la projection (2.11), historique (5.1 / 8.1). | **Comportement différent :** une valeur fournisseur qui diverge d'une cellule ✓ **ne remet pas** le drapeau à `?`. La cellule est **figée** (✓ et valeur gardés), la valeur divergente est mise en attente et une notice de contradiction s'affiche (issue #110, `app/src/state/refresh.rs:65,507`, test `tests.rs:1603`). Le PRD dit encore « validated flags on changed cells reset » (J2b l. 363) et, dans FR20, « auto-tags it ✓→? » (l. 783-784) : l'écart n'a pas été réconcilié. |
| **J3** — Risque sur plusieurs banques | **Tenu** | Plusieurs portefeuilles (6.1), plusieurs devises (6.2), rafraîchissement manuel et fraîcheur (4.4), alerte zone d'achat (4.2), capital à risque par devise, par banque puis global (6.6), FX à la consolidation (6.5), concentration sur le total (6.7), stop suiveur à cliquet (4.5). | Durée du rafraîchissement : voir P3 (cadencement Twelve Data). |
| **J3b** — Panne du fournisseur | **Tenu avec écart** | Marquage périmé, conservation des dernières valeurs, message qui nomme la cause, repli manuel, travail hors ligne (3.5) ; chaîne de repli (6.9). | Quota et retry-after jamais observés en réel (G8) ; la journalisation de la cause des échecs (G7) est reportée (rétro 7 l. 68-69, rétro 8 §3). |
| **J4** — Signal de vente et remplacement | **Tenu** | Déclencheurs neutres, « Vente… » / « Relever le stop… » / « Ignorer », priorité du stop (4.7) ; vente partielle (6.3) ; candidats de remplacement (6.8) avec faits de reconcentration par devise et par secteur (#98, `app/src/state/replacement.rs:55-70`) ; ouverture de l'étude du candidat. | La trace « vendu X → remplacé par Y » est la motivation de vente saisie par l'utilisateur ; l'application ne l'écrit pas (choix FR13 documenté, `6-8-…md:71`). |
| **J5** — Confronter un jugement passé | **Tenu** | Réouverture, lignes de jugement, provenance et validation, motivation, notes (8.1), superposition en lecture seule de la projection et du cours réel (5.1, cache d'historique de prix alimenté par le fournisseur). | La confrontation demande un historique de prix (Twelve Data). |
| **J6** — Éclaireur IA sous contrôle humain | **Tenu avec écart** | Serveur MCP stdio (8.4), lecture des études sans portefeuille (8.3), brouillons d'étude, de note, de cellule et de jugement avec commentaire et origine (8.2a/b), boîte et rappel par étude (8.5a), décision une à une (8.5b), lignes IA inertes tant qu'elles sont en attente, « placée par l'IA » après validation (8.6), étude issue d'un brouillon et Registre (8.7). | **Jamais parcouru de bout en bout en réel** : aucun client IA branché, fetch stubbé pendant les parcours, aucun test à l'écran par Guy depuis 8.5a (rétro 8 §2.2, H2 ouvert ; `epic-8-test-checklist.md` §2, §6 « jamais vu »). |

---

## 5. Exigences propres à l'application de bureau

| Rubrique | État | Preuves |
|---|---|---|
| Plateformes (Windows, macOS, Linux ; spike du graphique) | **Non tenu** (Linux seul vérifié) | CI Linux seule (`ci.yml:27`, commit `6116cc6`, `architecture.md:322`) ; Windows jamais compilé (rétro 8, H6) ; macOS : aucune trace ; `deferred-work.md` (1.1) : aucune dépendance Slint provisionnée pour mac et Windows. Au runtime, trousseau « mock » hors Linux (`Cargo.toml:82` + `keyring-3.6.3/src/lib.rs:296-297`). Le spike du graphique natif est GO (1.5). Le PRD annonce toujours trois OS (l. 418-425) ; il n'a pas été aligné sur la décision d'architecture. |
| Intégration système (trousseau, fichier SQLite unique à un emplacement connu, locale, notifications internes, MCP local) | **Tenu avec écart** | Trousseau (Linux), emplacement et journaux récents (5.5), sauvegarde (5.4), notifications internes seulement (4.2), MCP qui sert le dernier dossier même application fermée (8.4, `docs/mcp-registration.md`). La locale n'est pas lue sur l'OS (X2). |
| Stratégie de mise à jour (pas d'auto-update ; migrations sûres) | **Tenu** | Aucun mécanisme de mise à jour dans le code ; migrations et corpus (R3). Rétro 8 : le premier lancement migre en v9, sans retour possible pour ce fichier (checklist §0). |
| Fonctionnement hors ligne (étude et risque sans réseau ; cache daté ; FX daté ; aucun appel IA depuis l'application) | **Tenu** | R1 ; `fx_rates` datés (6.5) ; l'application n'a aucune dépendance IA (seul `mcp` sert le client, en stdio). |
| Mise en œuvre (UI mince, trait `MarketDataProvider`, contrat serde découplé) | **Tenu avec écart** | Trait et adaptateurs EODHD et Twelve Data (3.1, 7.4) ; contrat découplé ; voir M1 pour la proportion du crate `app`. |

---

## 6. Écarts et lacunes notables (du plus au moins important)

1. **Multiplateforme : non tenu, et le PRD ne le dit pas.** Linux est la seule plateforme construite, testée ou utilisée. Windows n'a jamais été compilé (H6) ; macOS n'a aucune trace. `keyring` n'est compilé qu'avec le backend Secret Service : sous Windows et macOS, les clés iraient dans un magasin fictif perdu à la fermeture. NFR-X1, NFR-X3, le succès technique « cross-platform » et la section Bureau restent écrits pour trois OS, alors que l'architecture a acté « Linux-only for now » dès le 2026-06-09.
2. **Données et propriété intellectuelle suivies par git, contraires à la posture « open-source-ready ».** Dix PDF NAIC/BetterInvesting protégés (`docs/NAIC/`) et deux extraits réels de fondamentaux EODHD (`ingestion/tests/fixtures/*-real.json`, commit `806f423`) sont dans le dépôt. Aucun avis « non affilié » dans l'application ; libellés NAIC par défaut. Sans conséquence tant que le dépôt reste privé, mais c'est précisément ce que le PRD veut éviter avant toute publication, et un historique git le conserve.
3. **Parcours J2b et FR20 contredits par le code.** Depuis #110, une valeur fournisseur divergente **fige** la cellule ✓ au lieu de la repasser à `?`. Le choix est défendable et testé, mais le PRD (J2b, FR20, résumé des parcours « reset on change ») n'a pas été mis à jour. S'y ajoute le chantier H3 (FR68, spec 8.0 §3.3, architecture A8/A9) de la rétro 8.
4. **Parcours J6 et plusieurs critères IA non éprouvés en réel.** Tout est livré et couvert par des tests, mais aucun client IA n'a été branché, aucun fetch réel n'a eu lieu après une validation ou un figement, et Guy n'a rien vu à l'écran depuis 8.5a (H2). L'acceptation par le propriétaire est suspendue.
5. **Couverture de la méthode incomplète.** Le drapeau « dette élevée », cité dans les critères de succès et dans J1, ne peut pas être levé (aucun champ dette) ; les références ne comprennent aucune fiche SSG « obtenue » ; la comparaison PDF / formulaire NAIC (G9, #207) est ouverte ; l'invariant d'aller-retour FX de NFR-C3 n'est pas testé (la conception refuse l'inversion).

Secondaires : pas d'outil de couverture pour l'objectif ≥ 95 % (NFR-C5) ; NFR-P2 et P4 non mesurés depuis juin ; le cadencement Twelve Data (7,5 s par requête) rend NFR-P3 intenable pour quelques dizaines de positions ; la locale de l'OS n'est pas lue (NFR-X2) ; #67 (`-wal` d'une copie brute) reste différé ; les exports PDF écrivent sur place ; README périmé (« Pre-implementation ») ; accessibilité clavier du glissement incomplète et aucun harnais de test d'interface (H7).

---

## 7. Décompte

| État | NFR (30) | Domaine et contraintes (8) | Succès (29) | Parcours (8) | Bureau (5) | **Total (80)** |
|---|---|---|---|---|---|---|
| Tenu | 16 | 3 | 18 | 4 | 2 | **43** |
| Tenu avec écart | 10 | 2 | 6 | 4 | 2 | **24** |
| Partiel | 1 | 3 | 1 | 0 | 0 | **5** |
| Non tenu | 1 | 0 | 1 | 0 | 1 | **3** |
| Non vérifiable | 2 | 0 | 3 | 0 | 0 | **5** |

(Succès = 5 utilisateur + 4 projet + 6 technique + 6 mesurables + 8 IA.)
