# Revue SteadyInvest — P3 : santé technique

Date : 2026-09-30 · Référence : `main` (dernier commit 0ee58d9, #270) ; arbre de travail sur `feat/validate-year` (PR #269).
Méthode : lecture seule (fichiers, `git log`, `gh`), sans compilation ni exécution des tests. Les chiffres de tests sont des comptes d'attributs `#[test]` / `#[tokio::test]`, pas des résultats d'exécution.

## Synthèse

L'état technique est **sain dans le cœur et dans la persistance**. Le calcul est déterministe et figé par des études de référence, les migrations sont couvertes par un corpus et la garantie « l'IA ne peut rien écrire hors de la boîte » est vérifiée par la construction même du code. Les risques se concentrent sur **l'interface** : aucun test ne l'exerce, les défauts de mise en page ne se découvrent qu'en parcours, et le premier test à l'écran de Guy (H2, 30/09) a immédiatement révélé un plantage (#267) et un défaut de protocole MCP (#268). Deux constats nouveaux, que les retros ne mentionnent pas :

1. **Un test instable (flaky) sur `main`.** L'exécution CI de la fusion #268 (run 36745087347, 30/09 16:33) a échoué sur `persistence/tests/readonly_protected.rs::the_private_read_copy_stays_private` (`metadata: NotFound`, ligne 460). L'exécution suivante (#270) est verte sans changement du test. Cause probable : une course TOCTOU — le test parcourt les copies privées `steadyinvest-read-{pid}-*` de **tout le processus** alors que les tests voisins créent et suppriment les leurs en parallèle ; `read_dir` réussit, puis une entrée disparaît avant `mode_of`. Rien ne le consigne (ni issue, ni rétro).
2. **La CI de `main` n'est pas un garde-fou.** `main` n'a aucune protection de branche (`gh api …/protection` → 404) et `concurrency.cancel-in-progress` annule la CI des commits de fusion : les fusions #264, #265 et #266 ont une CI **annulée** sur `main` (le commit « chore(sprint) » suivant l'a couverte). Ce point avait été différé dès la story 1.1 et s'est matérialisé.

---

## 1. Registre de la dette

Légende — **Impact** : effet sur Guy (haut / moyen / bas). **Bloque** : empêche-t-il l'usage réel sur son vrai dossier ? (oui / partiel / non).

### 1.1 Vue groupée (du plus important au moins important)

| # | Groupe | Éléments (sources) | Impact | Bloque | Commentaire |
|---|---|---|---|---|---|
| D1 | **Recette d'Epic 8 inachevée (H2)** | H2, G4 partiel ; `epic-8-test-checklist.md` : client IA jamais branché, Journey 6 avec clé jamais vu, PDF « † » et « Verdict figé » jamais ouverts, rafraîchissement après gel jamais vu | haut | partiel | H2 a commencé le 30/09 et a déjà produit #267, #268, #269, #270. Le reste de la liste n'est pas coché. |
| D2 | **Migration v9 irréversible** | Liste H2 §0 ; rétro 8 §6 | haut | oui, tant qu'aucune sauvegarde n'a été faite | Le premier lancement migre le vrai dossier en v9, sans retour possible. C'est la seule opération sans filet de toute la liste : il faut une sauvegarde avant. |
| D3 | **Largeur à fenêtre étroite (G6)** | G6 (rétro 7, rétro 8) ; 7.0 L6/L7/L12 ; « Seen, not changed » 8.5a, 8.5b, 8.6, 8.7, 8.8 (écran d'étude plus large que la fenêtre à 1280 px, historique ouvert, lignes Études à 1600 px) | haut | partiel | La dette dicte désormais des choix de produit : rail à 200 px, bouton « Valider l'étude » sur sa propre ligne. La décision appartient à Guy. |
| D4 | **Aucun banc de test d'interface (H7)** | H7 ; « No UI-level harness » dans 8.6, 8.7 et 8.8 ; #267 (plantage RefCell non reproductible sous Xvfb) | haut | non (risque) | Le câblage Slint ↔ état n'est vérifié qu'à l'œil. Aucune utilisation de `slint::testing` / `i-slint-backend-testing` dans le dépôt. |
| D5 | **Focus et clavier** | Après fermeture d'une fenêtre modale, aucun élément n'a le focus et Ctrl+Z exige un clic (depuis 2.9, revu en 8.8) ; focus déclenché par une minuterie de 30 ms (contournement Slint 1.17, `action_button.slint`) ; en 8.7, le focus suit la position de la ligne après relecture ; en 2.8, le curseur de glissement annonce un rôle a11y *slider* sans pas clavier ; en 2.9, l'ordre du FocusScope et Ctrl+Z ne sont pas vérifiés | moyen | non | Le réflexe Ctrl+Z après un dialogue échoue en silence : c'est le plus gênant en usage quotidien. |
| D6 | **Diagnostic des fournisseurs (G7)** | G7 : cause d'un échec absente du journal ; la notice « dernières données connues » est fausse dans l'examen | moyen | non | Hors périmètre d'Epic 8, reporté tel quel. |
| D7 | **Quota réel (G8) et conventions de tickers** | G8 / reste de F6 (retry-after, arrêt du criblage jamais observés) ; 7.4 [Defer] HIGH : `NESN.SW` chez EODHD contre `NESN` chez Twelve Data → `TickerNotFound` quand on change de fournisseur (#70) | moyen | non (EODHD seul fonctionne) | À traiter avant de s'appuyer sur la chaîne de repli. Vérifier l'état de #70 (fermé ou absorbé) : il n'est plus dans les issues ouvertes. |
| D8 | **PDF comparé au formulaire NAIC (G9, #207)** | #207 ouverte ; G9 ; 7.3 : chevauchement des libellés sur les études de plus de 30 ans ; 5.6 : le PDF ignore la locale des nombres (« 2.9 % ») ; les PDF sont vérifiés par la structure et le déterminisme, jamais visuellement | moyen | non | La règle « NAIC tranche » rend #207 prioritaire pour la crédibilité du PDF. |
| D9 | **Textes de référence non réconciliés (H3)** | PRD FR68, spec 8.0 §3.3, architecture A8 / A9 | moyen | non | Documentation seulement ; à faire après les arbitrages de H2. |
| D10 | **Provenance de `current_price` / `ttm_eps` (H5)** | 8.8 Deferred : un cours saisi à la main après un rafraîchissement est attribué au rafraîchissement | moyen | non | Décision de Guy attendue pendant H2. |
| D11 | **Windows et macOS jamais construits (H6)** | H6 ; matrice CI Linux seule depuis le 2026-06-09 ; 8.3 : fenêtre de verrou de restauration propre à Windows (A11) ; chemins de 8.4 ; différé 1.1 (dépendances Slint des runners) | bas aujourd'hui | non (Guy est sous Linux) | Ce n'est un risque que si l'on distribue un jour. Un seul `cfg(windows)` dans le code, jamais compilé. |
| D12 | **Dettes de revue (G1 / 7.x)** | Exports PDF écrits sur place (pas de fichier temporaire + renommage : un échec tronque un PDF existant) ; français codé en dur dans le Rust hors inventaire (`quick_screen.rs`, `fetch.rs`, titres rfd) ; indices d'écran magiques (`set_current_screen(n)`, 8 occurrences) ; ordre du grand livre dans une même journée ; états rendus en texte brut | bas à moyen | non | Le risque « écraser le dossier vivant » est en pratique neutralisé : `with_pdf_extension` ajoute `.pdf` et refuse un nom déjà pris. Le français codé en dur contredit la règle §4 de `review-checklist.md`. |
| D13 | **Différés faibles d'Epic 8** | 8.6 : l'aperçu pendant le glissement ne retrace pas la superposition, hauteurs de ligne codées en dur, collisions d'étiquettes non vérifiées ; 8.7 : plusieurs décisions dans la même seconde mal ordonnées, le test FR65 ne construit pas tous les modèles de vue ; 8.8 : revalider un verdict identique est permis, « entrées ouvertes : — » | bas | non | Cosmétique ou cas limites. |
| D14 | **Différés anciens (Epics 1–2)** | Arrondi bancaire ou demi-supérieur (1.1, tranché par la spec 1.2 ?) ; validation à l'exécution des chaînes libres du contrat (horodatage, ISO-4217) ; tolérance des enums inconnus (choix assumé) ; saisie IME ou chiffres non ASCII ; graphique : axe 1→200 (#25), point unique (#27), grip orphelin (#26) ; compare-scénario en *placeholder* ; import d'une ancienne sauvegarde sans arbitrage de version (#65, repris par 5.4 ?), `-wal` d'une sauvegarde externe (#67) | bas | non | Le fichier `deferred-work.md` n'a plus été tenu depuis G1 : on ne sait pas lesquels de ces points sont clos. Le débordement Decimal de 4.6 est **résolu** (`max_holding_magnitude`, `checked_*` dans `core/src/risk`). |
| D15 | **Issues « interprétations » #16–#24** | 9 issues ouvertes depuis juillet (1.10, 1.11, 2.1, 2.3–2.7) | bas | non | Ce sont des registres de décisions et non de la dette. Les laisser ouvertes brouille la liste des issues ; fermer avec un libellé « record », ou les déplacer dans la documentation. |
| D16 | **Veille d'avis de sécurité orpheline** | `deny.toml` ignore RUSTSEC-2026-0194/0195 (quick-xml 0.39.4, toujours dans `Cargo.lock`) « tracked in issue #83 » — or #83 est **fermée** depuis le 2026-07-05 | bas | non | Le risque est réel mais limité (générateurs au moment de la compilation). Le mécanisme de levée de l'exception n'existe plus. |
| D17 | **Test instable sur `main`** | `the_private_read_copy_stays_private` (voir la synthèse) | moyen (processus) | non | Il peut masquer une vraie régression ou faire relancer la CI à l'aveugle. |

### 1.2 Actions de rétro encore ouvertes

| Action | Origine | État | Note |
|---|---|---|---|
| F3 (règle du discriminant) | Epic 6 | ✅ dans `review-checklist.md` §2 | Marquée « non évidencée » en rétro 7, présente depuis. |
| F6 (vérification réelle des fournisseurs) | Epic 6 | ⏳ → G8 | |
| G4 (correctif visuel revu à l'écran) | Epic 7, amendée en 8 | ⏳ partielle | H2 en cours |
| G6 largeur | Epic 7 | ❌ ouverte | Décision de Guy |
| G7 diagnostic fournisseur | Epic 7 | ⏸ | |
| G8 quota réel | Epic 7 | ⏸ | Guy |
| G9 / #207 | Epic 7 | ⏸ | Guy |
| H1 liste de recette | Epic 8 | ✅ | |
| H2 test à l'écran | Epic 8 | ⏳ commencé le 30/09 | #267–#270 en sont issus |
| H3 réconciliation des textes | Epic 8 | ❌ | |
| H4 lire le résumé des tests | Epic 8 | règle | Pas inscrite dans `review-checklist.md` |
| H5 provenance du cours | Epic 8 | ❌ | Guy |
| H6 Windows | Epic 8 | ❌ | |
| H7 banc de test d'interface | Epic 8 | ❌ | Proposition à chiffrer |

### 1.3 Marqueurs dans le code

Aucun `TODO`, `FIXME`, `unimplemented!` ni `todo!` dans les huit crates. Les deux « XXX » trouvés sont des données de test (ticker et devise fictifs). La dette vit dans les registres, pas dans le code : c'est bon pour la propreté, mais ces registres doivent alors être tenus (voir D14).

### 1.4 Issues GitHub ouvertes (9) et PR ouvertes (2)

- #207 PDF comparé au formulaire NAIC (enhancement) ; #16, #17, #19–#24 : registres d'interprétations.
- PR #269 (validation d'une année en un geste, CI verte) ; PR #271 (guide IA et README, CI en cours au moment de la revue).

---

## 2. Santé des tests

### 2.1 Comptes par crate (attributs de test ; l'enregistrement officiel est de 1454 tests)

| Crate | Tests | Fichiers `tests/` | Types présents |
|---|---:|---|---|
| app | 657 | — (tous en ligne) | Unitaires sur l'état (`state/tests.rs` : 309), modèles de vue (moteur, graphique, brouillons, historique, format…), **barrières de posture** (`posture.rs` : 26 — plancher exact `@tr` à 1172, inventaire `MSG_*` à 251, verbes interdits, absence de marque NAIC) |
| persistence | 267 | 16 | Intégration SQLite, **corpus de migration figés** (`corpus/v1.db`, `v8.db`, générateurs `#[ignore]` à usage unique), e2e du cycle de vie, dossier en lecture seule ou protégé, autorisateur MCP (`mcp_access.rs`), sondage de la boîte |
| core | 239 | 12 | **Études de référence** (g01–g09 en JSON), **métamorphiques** (`ssg_metamorphic`, `normalize_metamorphic`), **propriétés** (proptest, 5 blocs), cohérence du verdict, **hachage de déterminisme** (étape CI dédiée) |
| report | 100 | — | PDF : bien formé, **octet pour octet déterministe**, notes absentes = identique à l'octet près, métriques Helvetica, pagination, largeurs, « † », verdict figé |
| contract | 77 | 5 | Aller-retour serde, empreinte, marques IA, gel ; proptest (4 blocs) |
| ingestion | 72 | 1 + fixtures | Mapping EODHD sur fixtures (aucun appel réseau) |
| mcp | 28 | 4 | **e2e stdio** (vrai binaire), **clôture** (aucune dépendance HTTP ne peut entrer), **non-exposition** (portefeuille, clés, configuration invisibles), isolation de HOME |
| paths | 7 | — | Unitaires |
| **Total** | **1447 attributs** | | L'écart avec 1454 vient probablement des cas générés par macro. |

### 2.2 Points forts

- Le cœur de calcul est protégé sur plusieurs axes : études de référence, métamorphique, propriétés et déterminisme.
- La garantie d'asymétrie de l'IA est testée par construction : clôture des dépendances, autorisateur SQLite, test métamorphique « une proposition en attente ne change aucune sortie ».
- Les migrations sont testées contre de vrais fichiers figés et non contre des schémas reconstruits.
- Les barrières de posture à compte exact empêchent les textes non traduits ou les verbes prescriptifs.

### 2.3 Lacunes notables

| Lacune | Conséquence | Preuve |
|---|---|---|
| **Aucun banc de test d'interface** (MainWindow, callbacks, focus, mise en page) | Les plantages et les défauts géométriques ne se voient qu'à l'œil. #267 (RefCell) : invisible sous Xvfb, découvert par Guy. | Rétro 8 §2.1 ; 8.6 / 8.7 / 8.8 |
| **PDF vérifiés par structure et déterminisme, jamais par rendu** | Un PDF peut être « déterministe et faux ». Chevauchements et troncatures (« Moyenne ») découverts en relecture visuelle. | Rétro 7 §2.3 ; aucun test de rendu raster |
| **Client MCP permissif dans les tests** | Les tests e2e passaient par le client rmcp tolérant ; les indications de cache SEP-2549 manquaient sans que rien échoue (#268). Un test stdio brut a été ajouté depuis. | PR #268 |
| **Windows et macOS** | Jamais compilés ; le test de hachage « multi-OS » ne tourne que sous Linux. | `ci.yml` |
| **Fournisseurs réels** | Quota, retry-after et repli jamais observés en conditions réelles. | G8, F6 |
| **Test instable** | `the_private_read_copy_stays_private` : course sur le répertoire temporaire partagé. | CI run 36745087347 |
| `state/tests.rs` monolithique | 10 791 lignes et 309 tests dans un seul fichier : difficile à parcourir, conflits de fusion probables. | `wc -l` |

---

## 3. Taille du code et points chauds

### 3.1 Lignes par crate (Rust + Slint, tests inclus)

| Crate | Rust | Slint | Part |
|---|---:|---:|---:|
| app | 53 534 | 12 722 (34 fichiers) | 55 % |
| persistence | 18 960 | — | 16 % |
| core | 13 193 | — | 11 % |
| report | 9 539 | — | 8 % |
| ingestion | 4 036 | — | 3 % |
| mcp | 3 700 | — | 3 % |
| contract | 3 624 | — | 3 % |
| paths | 338 | — | < 1 % |
| **Total** | **106 924** | **12 722** | **119 646** |

### 3.2 Plus gros fichiers

| Fichier | Lignes | Remarque |
|---|---:|---|
| `app/src/state/tests.rs` | 10 791 | +2 261 en Epic 8 ; fichier-dieu de tests |
| `report/src/pdf.rs` | 5 160 | Moteur PDF, dont ~1 500 lignes de tests en ligne ; +612 en Epic 8 |
| `persistence/src/mcp_access.rs` | 2 573 | Nouveau en Epic 8 ; surface de sécurité critique |
| `app/src/posture.rs` | 2 111 | Barrières de posture |
| `app/src/viewmodel/drafts.rs` | 2 053 | Nouveau en Epic 8 |
| `app/src/viewmodel/engine.rs` | 2 012 | |
| `app/ui/state.slint` | 1 962 | État global Slint : couplage fort, tout écran en dépend |
| `app/src/wiring/holdings.rs` | 1 800 | Signalé à surveiller dès la rétro 6 (~1 300 lignes alors) |
| `app/ui/screens/study_screen.slint` | 1 547 | Siège de la dette G6 |
| `app/ui/components/modal_dialog.slint` | 1 233 | +482 en Epic 8 |

On compte 23 fichiers Rust de plus de 1 000 lignes.

### 3.3 Croissance en Epic 8 (`bffbcf7..main`, `*.rs` et `*.slint`)

118 fichiers touchés, **+28 336 / −591 lignes**. Plus fortes hausses : `mcp_access.rs` (+2 573, nouveau), `state/tests.rs` (+2 261), `viewmodel/drafts.rs` (+2 053, nouveau), `persistence/tests/mcp_access.rs` (+1 375), `persistence/tests/drafts.rs` (+1 249), `wiring/drafts.rs` (+1 229), `state/drafts.rs` (+923), `mcp/src/tools.rs` (+819), `report/src/pdf.rs` (+612), `contract/src/draftable.rs` (+598).

### 3.4 Risques structurels

- **Découpage des crates sain** : `core` est pur, `contract` découplé, la clôture de `mcp` est testée, `main.rs` est court (390 lignes) et le câblage réparti en 21 modules `wiring/`.
- **Concentration dans `app`** (55 % du code) : l'état global `state.slint` et des modules `wiring/*` de plus de 1 000 lignes. Le couplage passe par des globals Slint et des indices d'écran magiques (D12).
- **`report/src/pdf.rs`** : un seul module porte la mise en page de quatre PDF. Une correction profite aux quatre (« shared engines pay twice »), mais une régression les touche aussi tous les quatre.
- **Aucune dette de type `unsafe` notable** : un seul appel FFI (`SQLITE_FCNTL_HAS_MOVED`), commenté « SAFETY » (8.3).

---

## 4. Dépendances

### 4.1 Versions clés (résolues dans `Cargo.lock`, 764 paquets)

| Dépendance | Déclarée | Résolue | Note |
|---|---|---|---|
| Rust toolchain | 1.96 (épinglée, `rust-toolchain.toml` et CI) | — | MSRV 1.96, édition 2024 |
| slint / slint-build | `"1.16"` | **1.17.0** | La déclaration dit 1.16, le lock résout 1.17 : incohérence cosmétique mais trompeuse (les commentaires de `rust-toolchain.toml` citent encore 1.16). |
| rmcp | 3.4 (`server`, `transport-io`, sans défaut) | 3.4.1 | Clôture HTTP vérifiée par test |
| rusqlite | 0.40 (`bundled`, `hooks`) | 0.40.1 | SQLite compilé dans le binaire |
| reqwest | 0.13 (rustls sans fournisseur, webpki-roots) | 0.13.1 | `ring`, pas d'aws-lc/cmake |
| tokio | 1.52 | 1.52.3 | |
| rust_decimal | 1.42 (`maths`) | 1.42.1 | Aucun `f64` |
| keyring | 3 (async-secret-service, async-io) | 3.6.3 | |
| rfd | 0.15 (xdg-portal, async-std) | 0.15.4 | |
| pdf-writer | 0.12 | 0.12.1 | |
| arboard | 3 | — | Désormais **dépendance d'exécution** (collage de colonne) ; le commentaire de `Cargo.toml` à la racine dit encore « Not a shipping dependency ». |

62 crates apparaissent en plusieurs versions (surtout la pile objc2, nix, bitflags, hashbrown) ; `multiple-versions = "warn"`.

### 4.2 `deny.toml`

- **Licences** : liste d'autorisation explicite, chaque exception justifiée (BSL-1.0, NCSA, CDLA-Permissive-2.0, Unlicense…).
- **Avis ignorés** : RUSTSEC-2026-0194 et 0195 (quick-xml 0.39.4, via wayland-scanner et zbus-lockstep, outils de compilation seulement). Suivi censé passer par #83, **fermée le 2026-07-05** (D16).
- `yanked = "deny"`, registres et git inconnus refusés, jokers refusés.

### 4.3 Problèmes amont découverts le 2026-09-30

| Problème | Contournement | Où | Fragilité |
|---|---|---|---|
| **Slint 1.17 : RefCell ré-entrant dans le layout du texte** (« RefCell already borrowed » quand `select-all` réévalue un `font-size` dépendant du layout pendant que Slint tient son contexte de mise en page ; vu sous FemtoVG) | Forcer l'évaluation de `font-size` avant `select-all()` (`if (self.font-size > 0px)`) | `app/ui/components/editable_cell.slint:288-294` (PR #267) | Contournement par effet de bord, non testable sous Xvfb (non reproduit). À signaler en amont et à revoir à chaque montée de Slint. |
| **rmcp laisse facultatives les indications de cache SEP-2549**, que le protocole 2026-07-28 exige (Claude Code rejette `tools/list` sans elles) | `with_cache_scope(CacheScope::Private)` explicite ; nouveau test stdio brut sur les deux versions de protocole | `mcp/src/server.rs:58-63` (PR #268) | Corrigé et testé. Le risque résiduel est qu'une montée de rmcp change la valeur par défaut. |
| Focus Slint 1.17 dans `init` d'un élément conditionnel (plus ancien) | Minuterie de 30 ms | `action_button.slint` | Idem : à revoir à chaque montée. |

---

## 5. Processus

### 5.1 CI (`.github/workflows/ci.yml`)

- Déclenchée sur les PR et les pushs vers `main`. Un seul OS (ubuntu-latest). Étapes : fmt, clippy `-D warnings --locked`, `cargo test --all --locked`, hachage de déterminisme, puis job `cargo deny`.
- Actions épinglées par SHA pour le toolchain ; `actions/checkout@v4` déclenche l'avertissement de dépréciation de Node 20. `ubuntu-latest` passe à Ubuntu 26 à partir du 19/10/2026 : risque de casse des dépendances système (libfontconfig, mold).
- **Faiblesses** : (a) pas de protection de branche sur `main` — rien n'empêche une fusion sur une CI rouge ; (b) `cancel-in-progress` annule la CI des commits de fusion de `main` (#264, #265 et #266 annulées) ; (c) le test instable (D17) a rendu `main` rouge le 30/09 sans suite enregistrée.

### 5.2 `justfile`

Les commandes `lint`, `test` et `ci` reproduisent la CI (le `ci` local ajoute `cargo deny`). Les cibles `mcp-build` et `mcp-seed` refusent le vrai dossier. Les cibles `spike-a` et `spike-c` sont des reliques de spikes, encore utiles pour la non-régression du déterminisme.

### 5.3 Règles de revue, tenue par les enregistrements

| Règle | Suivie ? | Preuve |
|---|---|---|
| G3, revue adverse à trois couches avant chaque fusion | ✅ pour 8.0–8.8 (section « G3 review » dans chaque record) · ❌ **pour #267–#270** (aucune mention de revue dans les descriptions de PR) | Les correctifs issus de H2 sont fusionnés « sur CI verte » sans revue adverse, alors que #267 touche un chemin de plantage. |
| G4, correctif visuel revu à l'écran | ⏳ en parcours headless, pas chez Guy avant H2 | Rétro 8 |
| G5, pas d'appel fournisseur sans accord | ✅ | Rétro 8 |
| H4, lire la ligne de résumé des tests | Nouvelle règle, **absente de `review-checklist.md`** | |
| `review-checklist.md` | À jour jusqu'à l'Epic 6 (§1–§8). Aucune leçon des Epics 7 et 8 : parcours de mise en page, focus après modale, client MCP strict, lecture des résultats de tests. | Fichier de 68 lignes |
| `definition-of-done.md` | **Obsolète et hors sujet** : datée du 2026-02-10, elle parle de Docker, `.env.example`, « console du navigateur », « API endpoints » — issue d'un autre projet. Elle n'est manifestement pas appliquée (application de bureau). `deployment-verification-checklist.md` est dans le même cas. | |

### 5.4 Incidents

| Date | Incident | Traitement |
|---|---|---|
| 2026-09-29 | **8.7, commit `bb85ecf`** : test de posture en échec masqué par un pipeline `grep` qui avalait le code de sortie ; commit sur la branche de la story. | Corrigé au commit suivant ; règle H4. La PR #265 a eu une CI verte. |
| 2026-09-30 | **CI rouge sur `main`** après la fusion #268 : test instable (D17). | Non consigné ; la CI suivante (#270) est verte. |
| 2026-09-30 | **Plantage en usage réel** (focus d'une cellule, RefCell Slint) découvert par Guy. | #267, contournement. |
| 2026-09-24 (rétro 7) | Appel EODHD réel lancé en headless sans l'accord de Guy. | Règle G5, tenue depuis. |

---

## 6. Documentation

| Document | État | Constat |
|---|---|---|
| `README.md` (`main`) | **Très obsolète** | « Status : Pre-implementation » ; décrit un MVP à portefeuille et devise uniques ; tableau des crates sans `paths` ni `mcp` ; rien sur l'IA, la compilation ou le lancement. PR #271 le met à jour (+43 lignes), non fusionnée. |
| Manuel utilisateur | **Absent** | Aucun guide d'usage général (étude, portefeuille, sauvegarde, restauration, fournisseurs). Seuls existent le glossaire intégré (Réglages, story 2.13, quelques termes SSG) et l'aide contextuelle. |
| `docs/guide-ia.md` | En PR #271 (480 lignes, en français) | Absorbe l'essentiel de `mcp-registration.md` (−128 lignes). |
| `docs/mcp-registration.md` | À jour sur `main` (8.4) | Allégé par #271. |
| `docs/method/ssg-method-spec-v1.md` | Référence de méthode | Non revue ici. |
| `docs/review-checklist.md` | Partiellement à jour | S'arrête à l'Epic 6 (voir 5.3). |
| `docs/definition-of-done.md`, `deployment-verification-checklist.md` | **Hors sujet** | Modèles Docker/web ; à réécrire ou supprimer. |
| `docs/change_request.md`, `change-request_guy.md`, `on-display-walk-epics-5-6.md`, `lessons-learned-chart-rendering.md`, `living-documentation-process.md` | Historiques | À classer (archives) pour ne pas passer pour des documents actifs. |
| `_bmad-output/implementation-artifacts/deferred-work.md` | **Plus tenu** | Dernière section : G1 (25/09). Rien d'Epic 8 ; des éléments résolus (débordement 4.6) ne sont pas marqués. |
| Planification (PRD, architecture, UX) | Écarts connus | FR68, A8, A9, §3.3 → H3. |
| Commentaires de manifeste | Écarts mineurs | `slint = "1.16"` contre 1.17.0 résolu ; `arboard` « not shipping » alors qu'il l'est. |

---

## Recommandations (par ordre de priorité)

1. **Sauvegarder le vrai dossier avant tout lancement de la version v9, puis finir H2** (D1, D2). C'est la seule écriture irréversible en jeu, et H2 est la seule source de défauts réels (4 PR le premier jour).
2. **Remettre la revue adverse G3 sur les correctifs issus de H2** (#267–#270 fusionnés sans), au moins une passe légère sur #267 (chemin de plantage) avant d'enchaîner.
3. **Corriger et consigner le test instable** `the_private_read_copy_stays_private` : limiter le parcours aux copies ouvertes par *ce* test, ou tolérer `NotFound` sur l'entrée comme c'est déjà fait pour le répertoire. Ouvrir une issue.
4. **Protéger `main`** (CI obligatoire) et **ne plus annuler la CI des fusions sur `main`** (`cancel-in-progress: ${{ github.ref != 'refs/heads/main' }}`).
5. **Trancher G6** (décision de Guy) : la dette de largeur décide déjà à sa place.
6. **Chiffrer H7** : un banc minimal avec le backend de test de Slint (instancier `MainWindow`, déclencher les callbacks clés, vérifier le focus après fermeture d'une modale). Il n'aurait pas attrapé #267 (propre au rendu FemtoVG), mais il attraperait les régressions de câblage.
7. **Fusionner #271, puis écrire un manuel utilisateur court** (installation, sauvegarde et restauration, étude, portefeuille, fournisseurs), et remplacer `definition-of-done.md` et `deployment-verification-checklist.md` par des versions propres au projet (parcours headless, test à l'écran de fin d'epic, H4).
8. **Remettre à jour `review-checklist.md`** avec les leçons des Epics 7 et 8 : parcours de mise en page, focus après modale, test MCP en stdio brut, lecture du résumé des tests (H4).
9. **Ranger le registre** : reprendre `deferred-work.md` (marquer résolu ou ouvert, ajouter les différés d'Epic 8), fermer ou relabelliser #16–#24, rouvrir un suivi pour les avis quick-xml (#83 fermée), vérifier #70 (tickers entre fournisseurs).
10. **Signaler le bug RefCell de Slint 1.17 en amont** et noter les deux contournements Slint (30 ms, `font-size`) comme points à revoir à chaque montée ; aligner la déclaration `slint = "1.17"`.
11. Plus tard : G7 et G8 avant de s'appuyer sur le repli de fournisseur ; #207 (la règle « NAIC tranche ») ; découper `state/tests.rs` par domaine quand on y retouche ; H6 seulement si la distribution hors Linux redevient un objectif.
