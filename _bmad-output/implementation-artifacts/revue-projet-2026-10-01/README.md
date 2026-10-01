# Revue de projet — 2026-10-01

Revue menée à la demande de Guy après l'epic 8, par huit agents en lecture seule (rapports ci-joints),
puis discutée avec lui. Les rapports sont l'état du code au matin du 2026-10-01 : plusieurs constats
ont été corrigés le jour même (voir « Suites »).

## Rapports

| Fichier | Contenu |
|---|---|
| `p1-fr01-16.md` … `p1-fr65-78.md` | Couverture des 78 exigences fonctionnelles du PRD, preuves `fichier:ligne` |
| `p1-nfr-succes-parcours.md` | Exigences non fonctionnelles, critères de succès, parcours, exigences « bureau » |
| `p2-ecarts-textes.md` | Écarts entre les textes validés (PRD, architecture, specs UX) et le code |
| `p3-sante-technique.md` | Dette, tests, dépendances, processus, documentation |

## Synthèse

- **78 FR** : 57 livrées et testées, 16 livrées avec écart, 4 partielles, 0 non livrée, 1 hors périmètre.
- **80 NFR / critères / parcours** : 43 tenus, 24 avec écart, 5 partiels, 3 non tenus, 5 non vérifiables.
- **24 écarts** textes ↔ code, dont 10 consignés nulle part.

## Suites (2026-10-01)

Les dix défauts confirmés par le test de Guy et la revue ont été corrigés, chacun revu en trois couches :
#274 (origine du cours, traçabilité, cours périmé), #275 (libellés neutres), #276 (convention des
symboles), #277 (devise), #278 (signaux de qualité sur l'étude et son PDF), #279 (démonstration),
#280 (confrontation sur la bande décidée), #281 (test instable), #282 (rattrapage de #267–#270).

## Décisions en attente (Guy)

1. Seuil d'âge du cours (FR23 : « un jour de bourse » par défaut, réglable) — non implémenté.
2. Comptes publiés dans une autre devise que la cotation : avertissement (spec §3) ou blocage.
3. Écran Comparaison et PDF en vocabulaire neutre (décision du 2026-09-26) face à FR63.
4. Cotations en centièmes (GBX…) : refusées ; conversion ou non.
5. Confrontation : une re-validation déplace la date de décision.
6. Colonne du ratio hausse/baisse affichée pour un verdict retenu ou provisoire ; tri par ratio.

Restent aussi la réconciliation des textes (H3 élargi, `p2-ecarts-textes.md`) et les points de
licence et de plateformes (`p1-nfr-succes-parcours.md`, constats 1 et 2).
