# Enregistrer le serveur MCP de SteadyInvest dans Claude Code

Ce document explique comment rendre le serveur `steadyinvest-mcp` (Story 8.4, Epic 8) utilisable
par un client IA — aujourd'hui Claude Code. **Aucune story ne fait cet enregistrement à votre
place** : c'est votre geste, fait à la main, avec les commandes ci-dessous.

## Ce que fait le serveur

- Il **lit** les études du dossier : cellules avec leur provenance, jugements, justification, notes,
  historique, et les résultats calculés (zones en codes neutres `low` / `middle` / `high`, ratio
  hausse/baisse, potentiel à 5 ans, état du verdict).
- Il **enregistre des propositions** (nouvelle étude, note, valeur de cellule ou de jugement), chacune
  avec un commentaire obligatoire. Une proposition ne change rien tant que vous ne l'avez pas validée,
  une par une, dans l'application.
- Il ne donne **jamais** accès au portefeuille, à la liste de suivi, aux clés ni à la configuration, et
  il ne peut pas interroger le fournisseur de données.
- Chaque réponse nomme le dossier lu (identifiant et chemin).

## Règle de prudence pendant l'Epic 8

Tant que l'Epic 8 est en cours, **passez toujours `--dossier` explicitement**, et commencez par une
**copie de test** de votre dossier. Sans `--dossier`, le serveur suit le dossier que l'application a
ouvert en dernier — votre vrai dossier.

## 1. Construire le binaire

Depuis le dépôt :

```sh
just mcp-build
```

Le binaire est `target/release/steadyinvest-mcp` (chemin absolu à noter, par exemple
`/home/gcorbaz/devel/steadyinvest/target/release/steadyinvest-mcp`).

## 2. Faire une copie de test du dossier

Le fichier du dossier est celui dont le chemin est affiché dans **Réglages** (carte du dossier). La
façon la plus sûre d'en obtenir une copie cohérente est la **sauvegarde de l'application** (Réglages →
créer une sauvegarde) : copiez le fichier de sauvegarde produit. Sinon, application **fermée**, copiez
le fichier du dossier **et** ses fichiers `-wal` et `-shm` s'ils existent, en gardant les mêmes
suffixes :

```sh
mkdir -p ~/steadyinvest-ia/dossiers
D="<le chemin affiché dans Réglages>"
cp "$D" ~/steadyinvest-ia/dossiers/copie.db
[ -f "$D-wal" ] && cp "$D-wal" ~/steadyinvest-ia/dossiers/copie.db-wal
[ -f "$D-shm" ] && cp "$D-shm" ~/steadyinvest-ia/dossiers/copie.db-shm
```

**Attention :** si vous ouvrez cette copie dans l'application (pour y voir les propositions),
l'application la retient comme dossier courant. Revenez ensuite à votre vrai dossier (Réglages →
ouvrir un dossier) avant de reprendre votre travail.

Pour voir des propositions dans l'application sans session IA, vous pouvez en déposer de chaque sorte
dans la copie : `just mcp-seed ~/steadyinvest-ia/dossiers/copie.db` (la commande refuse votre vrai
dossier).

## 3. Enregistrer le serveur — portée `local`, depuis un répertoire hors du dépôt

Le serveur doit être enregistré à une portée **qui n'est pas active dans le dépôt steadyinvest** : les
sessions de développement de Claude Code dans ce dépôt ne doivent jamais voir vos études.

```sh
mkdir -p ~/steadyinvest-ia
cd ~/steadyinvest-ia
claude mcp add --scope local steadyinvest -- \
    /home/gcorbaz/devel/steadyinvest/target/release/steadyinvest-mcp \
    --dossier /home/gcorbaz/steadyinvest-ia/dossiers/copie.db
```

Pourquoi pas une autre portée :

- **`--scope user`** : le serveur serait actif dans **tous** vos projets, y compris le dépôt
  steadyinvest — les sessions de développement verraient vos études.
- **`--scope project` dans le dépôt** : le fichier `.mcp.json` serait versionné avec le code et actif
  pour toute session dans le dépôt — exactement ce qu'il faut éviter.

La portée `local` enregistre le serveur pour ce seul répertoire (`~/steadyinvest-ia`), sur cette
machine.

## 4. Vérifier

Toujours depuis `~/steadyinvest-ia` :

```sh
claude mcp list
```

`steadyinvest` doit y figurer. Puis lancez `claude` dans ce répertoire et tapez `/mcp` : le serveur
est connecté et propose exactement huit outils — `list_studies`, `get_study`,
`get_judgment_history`, `get_notes`, `get_drafts_record`, `submit_draft_study`,
`submit_draft_note`, `submit_draft_value`.

Depuis le dépôt steadyinvest, `claude mcp list` ne doit **pas** montrer `steadyinvest`.

## 5. Passer plus tard au vrai dossier (votre décision)

Quand vous le déciderez, retirez l'enregistrement de test et enregistrez le serveur sur votre vrai
dossier, toujours depuis `~/steadyinvest-ia` et toujours avec `--dossier` explicite :

```sh
cd ~/steadyinvest-ia
claude mcp remove --scope local steadyinvest
claude mcp add --scope local steadyinvest -- \
    /home/gcorbaz/devel/steadyinvest/target/release/steadyinvest-mcp \
    --dossier /home/gcorbaz/.local/share/steadyinvest/journal.db
```

L'application peut rester ouverte : le serveur ne prend pas son verrou, ne migre jamais le dossier et
n'y écrit que des propositions. Une proposition déposée pendant que l'application est fermée apparaît
à la prochaine ouverture.

## Où lire ce qui s'est passé

Le serveur écrit son propre journal, `steadyinvest-mcp.log.<date>`, à côté de celui de l'application
(`~/.local/share/steadyinvest/logs/`). Chaque refus y est noté avec son code et sa raison ; aucune clé
n'y figure.
