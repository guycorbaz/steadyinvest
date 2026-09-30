# Epic 8 — liste de contrôle du test à l'écran (action H2 de la rétro)

Ce que tu n'as pas encore vu : tout ce qui a été livré de 8.1 à 8.8. Les points marqués **(jamais vu)**
n'ont été vérifiés ni en parcours headless ni ailleurs que par des tests. Les points ⚑ sont des
décisions prises à ta place : accepte-les, ou ouvre une issue. Ce qui ne va pas → une issue GitHub
(capture d'écran bienvenue) ; l'agent la traitera.

## 0. Avant de commencer

- [ ] **Sauvegarde ton dossier** (« Créer une sauvegarde », ou copie du fichier). Le premier lancement de
  cette version le migre en **v9** (garde contre un identifiant de proposition réutilisé) ; c'est
  sans retour pour ce fichier. Variante prudente : tout le test sur une copie.
- [ ] Construire depuis `main` : `cargo build -p steadyinvest-app` et `just mcp-build`.
- [ ] Taille de fenêtre : celle que tu utilises d'habitude. Note ce qui déborde (dette G6, §9).

## 1. Notes d'étude (8.1)

- [ ] Ajouter, modifier, supprimer une note (dialogue titré, confirmation de suppression).
- [ ] La note apparaît dans l'historique de l'étude ; Ctrl+Z / Ctrl+Y la font aller et venir.

## 2. Brancher ton client IA (8.4) — **(jamais vu)**

Suivre `docs/mcp-registration.md` : d'abord sur une **copie** du dossier (§2), portée `local`, depuis
un répertoire hors du dépôt. Passer au vrai dossier (§5) est ta décision.

- [ ] Le client lit la liste des études et une étude (`get_study`) : zones en codes neutres
  (`low` / `middle` / `high`), jamais « achat / vente / maintien ».
- [ ] Il ne voit **ni** portefeuille, **ni** liste de suivi, **ni** clés, **ni** configuration : demande-lui.
- [ ] Il propose : une note, une valeur de cellule, un jugement (PER, BPA estimé, croissance), une
  nouvelle étude. Chaque proposition porte un commentaire.
- [ ] Demande-lui d'écrire directement dans une étude : refusé, avec un code nommé.
- [ ] ⚑ Les messages de refus MCP (en français, écrits par l'agent) te semblent-ils lisibles ?
  ⚑ Limites : 10 000 caractères pour une note ou un commentaire, 200 pour un nom de société.

## 3. Boîte « Propositions » (8.5a)

- [ ] Le compte apparaît sur le rail dans les secondes qui suivent la proposition (relecture toutes
  les 2,5 s ; ⚑ pas au retour du focus, Slint ne le permet pas).
- [ ] Bande ★ sur la carte Études et dans l'étude ; « Voir les propositions » arrive filtré sur l'étude.
- [ ] Le cadre IA : libellé, avertissement, « — Proposée par {client} ({modèle}) le {date} ».
- [ ] ⚑ Filtre « Étude : » aussi dans « À traiter » ; ⚑ rail à 200 px, compte plafonné à « 99+ ».

## 4. Décider une proposition (8.5b)

- [ ] Valider, « Modifier avant de valider… », Rejeter ; la valeur actuelle et la proposée côte à côte.
- [ ] ⚑ Entrée / Espace n'agissent qu'après 350 ms de focus sur un verbe (garde contre le réflexe).
- [ ] Ctrl+Z annule une validation, **même après avoir fermé puis rouvert l'étude**.
- [ ] Cellule validée : ligne « Source » = « proposée par l'IA, validée le … » ; une modification de
  ta part efface la marque.
- [ ] **(jamais vu)** PDF de l'étude : ⚑ marque « † » sur une valeur d'origine IA, et sa note.

## 5. Lignes IA sur les graphiques (8.6)

- [ ] Une proposition de jugement se trace en ligne grise pointillée à anneau creux, avec sa puce ;
  on la distingue bien de l'est-low (tirets roses) **sur ton écran**.
- [ ] La puce ouvre la décision ; après validation, légende « placée par l'IA · validée le … ».
- [ ] Ouvrir un dialogue par-dessus un graphique : pas de restes de tracé (vu seulement sous Xvfb).
- [ ] ⚑ Une validation annulée ne fait pas revenir la ligne IA (AC 8 corrigé).
- [ ] ⚑ La croissance proposée ne trace un est-high implicite que si tu n'as pas saisi d'est-high.

## 6. Étude proposée et Registre (8.7)

- [ ] Une proposition d'étude : dialogue « Valider… / Rejeter / Annuler », focus sur « Annuler ».
- [ ] « Valider… » ouvre le formulaire prérempli (cadre IA) ; tu peux corriger ticker, devise, nom.
  ⚑ Le verbe reste « Enregistrer » (les epics disaient « Créer »).
- [ ] Un doublon (même ticker et devise) est refusé dans le formulaire.
- [ ] **(jamais vu) Journey 6 avec ta clé** : sur l'étude créée, « Récupérer (fournisseur) ».
- [ ] Registre : vues, filtres de résultat, « Détail » dans un cadre IA ; historique de l'étude avec
  ses entrées ★.
- [ ] ⚑ Supprimer une étude créée d'une proposition efface aussi sa trace du Registre.

## 7. Verdict figé (8.8)

- [ ] Sur une étude au verdict **complet** : « Valider l'étude » → notice « Étude validée ; verdict
  figé le JJ/MM. » et la ligne « … identique au verdict figé ».
- [ ] Sur un verdict incomplet : bouton désactivé, raison « Verdict incomplet — entrées ouvertes : … ».
- [ ] Modifier une entrée (ex. PER haut) → bande « • Le verdict actuel diffère… », « Voir la
  comparaison » : huit lignes, « • » sur les lignes changées, « Cause : modification de votre part ».
- [ ] **(jamais vu) Rafraîchir avec ta clé** après la validation → « Cause : rafraîchissement du JJ/MM ».
- [ ] ⚑ **À trancher** : tape un cours à la main après un rafraîchissement — la cause dira
  « rafraîchissement » (le cours n'a pas de provenance). Acceptable ?
- [ ] Revalider → confirmation de remplacement ; Ctrl+Z rétablit le verdict précédent et le dit.
- [ ] Historique : « Étude validée ; verdict figé ».
- [ ] **(jamais vu)** PDF : bloc « Verdict figé le … » après « Position », et la comparaison s'il diffère.
  ⚑ Le PDF ne donne que le **nombre** d'entrées modifiées, l'écran les nomme.
- [ ] Export / import d'une étude et du dossier : le verdict figé survit.
- [ ] ⚑ Placement du bouton : sur sa propre ligne sous les actions (la spec disait « après
  Historique », hors écran à 1280 et 1600 px). ⚑ Mots « critères réunis / non réunis », « réuni /
  non réuni / inconnu ». ⚑ Dates en UTC (JJ/MM à l'écran, JJ/MM/AAAA dans le PDF). ⚑ Revalider un
  verdict identique reste permis (une entrée d'historique de plus).

## 8. Textes à réconcilier ensuite (action H3)

Selon tes arbitrages ci-dessus, l'agent alignera : PRD FR68 (verdict actuel vivant et coloré,
comparaison à toute différence), spec 8.0 §3.3 (codes MCP ajoutés), architecture A8 (historique
d'annulation conservé à la fermeture) et A9 (pas de relecture au focus).

## 9. Largeur (G6)

- [ ] Note, à ta taille de fenêtre, ce qui déborde : rangée d'actions de l'étude, historique ouvert,
  lignes de la carte Études. La décision de mise en page t'appartient ; elle a déjà dicté le rail à
  200 px et le placement de « Valider l'étude ».
