# Utiliser l'IA avec SteadyInvest — guide complet

Ce guide décrit tout ce qui permet de travailler avec une IA sur vos études : ce que l'IA peut et
ne peut pas faire, la construction et l'enregistrement du serveur MCP, les répertoires en jeu, le
traitement des propositions dans l'application, et le dépannage.

Il a été vérifié sous **Linux** avec **Claude Code** comme client IA. Les autres systèmes et les
autres clients sont décrits en fin de guide, sans avoir été essayés.

---

## 1. Le principe en une page

SteadyInvest **ne contient aucune IA**. Une IA ne peut travailler avec lui que de l'extérieur, à
travers un petit programme séparé, le **serveur MCP** (`steadyinvest-mcp`). C'est votre client IA
(Claude Code, par exemple) qui lance ce programme et lui parle ; l'application, elle, ne parle
jamais à une IA.

Par ce serveur, l'IA peut :

- **lire vos études** : les cellules avec leur provenance, vos jugements, votre justification, vos
  notes, l'historique, le verdict figé si vous en avez validé un, et les résultats calculés (zones,
  ratio hausse/baisse, potentiel à 5 ans, état du verdict) ;
- **déposer des propositions** : une nouvelle étude, une note, une valeur de cellule ou de
  jugement — chacune avec un commentaire obligatoire qui dit pourquoi.

Elle ne peut **jamais** :

- modifier une étude, une valeur, un jugement ou une note : une proposition n'a **aucun effet**
  (ni sur les chiffres, ni sur les zones, ni sur le verdict) tant que vous ne l'avez pas validée,
  une par une, dans l'application ;
- voir votre **portefeuille**, votre **liste de suivi**, vos **clés** de fournisseur ou votre
  **configuration** ;
- interroger le **fournisseur de données** : les récupérations restent votre geste.

Ces garanties tiennent **par construction** (le serveur n'a techniquement accès qu'à la lecture des
études et à l'écriture de propositions), et des tests automatiques les vérifient à chaque
modification du code.

Tout texte écrit par une IA apparaît dans l'application **dans un cadre « IA »**, avec
l'avertissement « Texte rédigé par une IA, non vérifié — ne constitue pas un conseil financier. »

---

## 2. Les répertoires et fichiers en jeu

| Quoi | Où (Linux) | Rôle |
|---|---|---|
| **Votre dossier** | le chemin affiché dans **Réglages** (carte du dossier) — par défaut `~/.local/share/steadyinvest/journal.db`, mais souvent ailleurs (ex. `~/steadyinvest/dossier.db`) | le fichier SQLite qui contient vos études, votre portefeuille, les propositions |
| Fichiers annexes du dossier | `<dossier>-wal`, `<dossier>-shm` à côté du dossier | journal d'écriture de SQLite ; à copier **avec** le dossier si vous le copiez à la main |
| **Configuration de l'application** | `~/.config/steadyinvest/config.json` | contient notamment `journal_path`, le dossier ouvert en dernier ; le serveur MCP la **lit** si vous ne lui donnez pas `--dossier`, sans jamais l'écrire |
| **Journaux** | `~/.local/share/steadyinvest/logs/` | `steadyinvest.log.<date>` (application) et `steadyinvest-mcp.log.<date>` (serveur MCP), côte à côte |
| **Binaire du serveur** | `<dépôt>/target/release/steadyinvest-mcp` (ex. `/home/gcorbaz/devel/steadyinvest/target/release/steadyinvest-mcp`) | le programme que votre client IA lance |
| **Répertoire de travail IA** | un répertoire à vous, **hors du dépôt**, par exemple `~/steadyinvest-ia` | c'est là que vous enregistrez le serveur et que vous lancez votre client IA |
| Enregistrement Claude Code | `~/.claude.json`, section du projet `~/steadyinvest-ia` | écrit par `claude mcp add --scope local` ; à ne pas éditer à la main |

Les variables `XDG_CONFIG_HOME` et `XDG_DATA_HOME`, si vous les définissez, déplacent la
configuration et les journaux comme pour toute application Linux.

---

## 3. Mise en place, pas à pas

### 3.1 Prérequis

- Le dépôt SteadyInvest et sa chaîne de construction Rust (ceux qui servent à `cargo run`).
- `just` (installé par `cargo install just`) — facultatif : chaque recette `just` a son équivalent
  `cargo` indiqué ci-dessous.
- Un client IA qui parle MCP par stdio. Ce guide utilise **Claude Code** (`claude`).
- Au moins une étude dans votre dossier, si vous voulez que l'IA ait quelque chose à lire.

### 3.2 Construire le serveur

Depuis le dépôt :

```sh
cd ~/devel/steadyinvest
just mcp-build            # équivalent : cargo build --release -p steadyinvest-mcp
```

Le programme produit est `target/release/steadyinvest-mcp`. Vérifiez-le :

```sh
./target/release/steadyinvest-mcp --version
```

**À refaire après chaque mise à jour du dépôt** (un `git pull`, un changement de branche) : le
client IA lance toujours ce fichier, il faut donc qu'il corresponde au code de l'application. Une
fois reconstruit, relancez votre session de client IA ; l'enregistrement, lui, reste valable.

### 3.3 Choisir le dossier que l'IA lira

Deux façons :

- **`--dossier <chemin>` (recommandé)** : le serveur lit toujours ce fichier-là, quoi que vous
  ouvriez dans l'application. Donnez un **chemin absolu** (un chemin relatif serait compris depuis
  le répertoire où le client IA lance le serveur).
- **Sans `--dossier`** : à chaque appel, le serveur lit le dossier que l'application a ouvert en
  dernier (`journal_path` de `config.json`). Pratique, mais l'IA change de dossier dès que vous en
  changez dans l'application.

**Pour un premier essai**, donnez-lui une **copie** de votre dossier ou un dossier de test :

- la façon la plus sûre : dans l'application, **Réglages → « Créer une sauvegarde »**, puis copiez
  le fichier de sauvegarde produit ;
- sinon, application **fermée**, copiez le fichier du dossier **et** ses fichiers `-wal` / `-shm`
  s'ils existent, en gardant les suffixes :

```sh
mkdir -p ~/steadyinvest-ia/dossiers
D="<le chemin affiché dans Réglages>"
cp "$D" ~/steadyinvest-ia/dossiers/copie.db
[ -f "$D-wal" ] && cp "$D-wal" ~/steadyinvest-ia/dossiers/copie.db-wal
[ -f "$D-shm" ] && cp "$D-shm" ~/steadyinvest-ia/dossiers/copie.db-shm
```

Pour **voir** les propositions d'une copie dans l'application, ouvrez-la par Réglages ;
l'application la retient alors comme dossier courant — revenez ensuite à votre vrai dossier par
Réglages.

### 3.4 Enregistrer le serveur dans Claude Code

L'enregistrement se fait **depuis un répertoire hors du dépôt**, en portée **`local`** :

```sh
mkdir -p ~/steadyinvest-ia
cd ~/steadyinvest-ia
claude mcp add --scope local steadyinvest -- \
    /home/gcorbaz/devel/steadyinvest/target/release/steadyinvest-mcp \
    --dossier /home/gcorbaz/steadyinvest/dossier.db
```

(adaptez les deux chemins : celui du binaire, celui de votre dossier.)

Pourquoi ces choix :

- **portée `local`** : le serveur n'est actif que pour ce répertoire (`~/steadyinvest-ia`), sur
  cette machine ;
- **pas `--scope user`** : le serveur serait actif dans **tous** vos projets, y compris le dépôt
  SteadyInvest — les sessions de développement y verraient vos études ;
- **pas `--scope project` dans le dépôt** : un fichier `.mcp.json` serait versionné avec le code.

**Vous lancerez donc toujours votre client IA depuis `~/steadyinvest-ia`** pour qu'il voie le
serveur.

### 3.5 Vérifier

Depuis `~/steadyinvest-ia` :

```sh
claude mcp list
```

La ligne doit se lire :

```
steadyinvest: …/steadyinvest-mcp --dossier …/dossier.db - ✔ Connected
```

Puis lancez `claude` dans ce répertoire et tapez `/mcp` : le serveur est connecté et propose
**huit outils** (section 5).

Depuis le dépôt, `claude mcp list` ne doit **pas** montrer `steadyinvest`.

### 3.6 Donner des consignes à l'IA (facultatif, conseillé)

Un fichier `CLAUDE.md` placé dans `~/steadyinvest-ia` est lu par Claude Code à chaque session
lancée depuis ce répertoire. C'est l'endroit pour vos consignes permanentes, par exemple :

```markdown
- Réponds en français.
- Suis la méthode NAIC (Stock Selection Guide) ; en cas de doute, la méthode NAIC tranche.
- Chaque proposition dit, dans son commentaire, d'où vient le chiffre (source, date).
- Ne propose une valeur que si tu peux la justifier ; sinon, dis-le.
```

Vos objectifs de recherche (« trouve des titres de croissance suisses ») restent dans la session
du client IA ; le dossier ne les enregistre pas.

### 3.7 Changer de dossier, désinstaller

Depuis `~/steadyinvest-ia` :

```sh
# changer de dossier : retirer puis réenregistrer
claude mcp remove --scope local steadyinvest
claude mcp add --scope local steadyinvest -- \
    /home/gcorbaz/devel/steadyinvest/target/release/steadyinvest-mcp \
    --dossier "<le nouveau chemin>"

# ne plus utiliser l'IA : retirer l'enregistrement
claude mcp remove --scope local steadyinvest
```

L'application peut rester ouverte pendant que l'IA travaille : le serveur ne prend pas son verrou,
ne migre jamais le dossier et n'y écrit que des propositions. Une proposition déposée pendant que
l'application est fermée apparaît à la prochaine ouverture.

---

## 4. Travailler avec l'IA

Quelques demandes typiques, dans une session `claude` lancée depuis `~/steadyinvest-ia` :

- « Liste mes études » / « Lis mon étude NVDA.US et commente mes jugements. »
- « Mon PER haut moyen te semble-t-il cohérent avec l'historique ? Si non, propose une valeur. »
- « Propose une note de synthèse sur SIKA.SW. »
- « Propose-moi trois études de sociétés suisses de croissance. »
- « Qu'ai-je fait de tes propositions précédentes ? » (le Registre, `get_drafts_record`).

L'IA voit les **signaux de qualité** du moteur (par exemple un PER haut jugé au-dessus de 20,
« agressif », ou de 25, « invraisemblable »). Dans l'application, ces signaux apparaissent dans
« Comparer des études » et dans la Revue du portefeuille, pas sur l'écran d'étude.

Ce que l'IA **ne peut pas proposer** : le **cours actuel** et le **BPA des douze derniers mois**
(ce sont des faits de marché, qui viennent du fournisseur ou de votre saisie), et une **année** qui
n'existe pas dans l'étude (une proposition n'ajoute jamais d'année).

Unités attendues pour une valeur : ventes et bénéfice avant impôt en **montant absolu** (pas en
millions), montants par action et cours dans la devise de l'étude, pourcentages écrits en pour
cent (`12` pour 12 %), PER en simple multiple.

---

## 5. Les huit outils du serveur

| Outil | Ce qu'il fait |
|---|---|
| `list_studies` | liste les études (id, symbole, date, statut), par pages de 50 (au plus 200) |
| `get_study` | lit une étude complète : cellules et provenance, jugements, justification, notes, verdict figé, résultats calculés (zones en codes neutres `low` / `middle` / `high`, ratio hausse/baisse, potentiel, faits du verdict) |
| `get_judgment_history` | l'historique d'une étude, chaque entrée étant l'étude entière à une sauvegarde passée (pages de 20, au plus 50) |
| `get_notes` | les notes d'une étude |
| `get_drafts_record` | le registre de toutes les propositions et de leur issue (en attente, validée, validée puis annulée, rejetée), filtrable par statut et par étude |
| `submit_draft_study` | propose une nouvelle étude (symbole, devise, nom facultatif) |
| `submit_draft_note` | propose une note sur une étude |
| `submit_draft_value` | propose une valeur de cellule (avec l'année) ou de jugement (sans année) |

Chaque réponse nomme le dossier lu (identifiant et chemin), et chaque proposition doit renvoyer
cette identité : si le dossier a changé entre la lecture et la proposition, la proposition est
refusée (`dossier_mismatch`) plutôt qu'enregistrée au mauvais endroit.

Limites : commentaire et note ≤ 10 000 caractères, nom de société ≤ 200, client et modèle ≤ 100,
valeur proposée ≤ 100 caractères ; **une seule proposition en attente par cible** (cellule ou
jugement) ; une étude déjà existante ou déjà proposée dans la même devise est refusée. Une même
proposition renvoyée avec le même `draft_id` n'est enregistrée qu'une fois (nouvelle tentative
sans doublon).

---

## 6. Traiter les propositions dans l'application

### 6.1 Où elles apparaissent

- **Le rail** : « Propositions · n », le nombre de propositions en attente. La liste est relue
  toutes les 2,5 secondes et à chaque ouverture de l'écran.
- **La carte Études** : « ★ n proposition(s) d'étude de l'IA en attente », et « ★ n » après le nom
  de chaque étude qui a des propositions.
- **Dans une étude** : la bande « ★ n proposition(s) de l'IA en attente sur cette étude. » avec
  « Voir les propositions ».
- **Sur les graphiques** (§1 et §3) : une proposition de jugement se trace en **ligne pointillée
  grise à rond creux**, étiquetée « IA », avec sa puce « Propositions de l'IA : » sous le graphique.

### 6.2 L'écran Propositions

Deux vues :

- **« À traiter »** : les propositions en attente, filtrables par sorte (Toutes, Valeurs,
  Jugements, Notes, Nouvelles études) et par étude (**« Étude : »**) ;
- **« Registre »** : toutes les propositions et leur issue (En attente, Validées, Validées puis
  annulées, Rejetées), avec « Détail ».

**Attention au filtre « Étude : »** : arrivé par « Voir les propositions » depuis une étude, la liste
est filtrée sur cette étude. Une proposition de **nouvelle étude** n'appartient à aucune étude
existante : elle n'apparaît que sous **« Toutes les études »**.

### 6.3 Décider

Cliquez sur une ligne : le dialogue « Proposition de l'IA » montre **l'actuel et le proposé côte à
côte**, avec le commentaire de l'IA dans son cadre. Trois verbes :

- **Valider** — la valeur entre dans l'étude ;
- **« Modifier avant de valider… »** — vous ajustez la valeur ; elle sera la vôtre, et la
  proposition est notée « modifiée avant validation » ;
- **Rejeter** — rien ne change, la proposition passe au Registre.

Entrée ou Espace n'agissent sur un verbe qu'après un court instant de focus (une garde contre la
touche réflexe). **Ctrl+Z annule une validation**, même après avoir fermé et rouvert l'étude.

Cas particuliers :

- **Proposition périmée** (« · périmée ») : sa cible a changé depuis le dépôt (votre saisie, un
  rafraîchissement, un changement de méthode). Sa validation demande une confirmation explicite.
- **Cible disparue** : l'année ou l'étude n'existe plus ; la proposition ne peut qu'être rejetée.
- **Cellule déjà validée (✓)** : le dialogue s'ouvre avec le focus sur « Annuler », par prudence.
- **Nouvelle étude** : « Valider… » ouvre le formulaire de création prérempli (symbole, devise,
  nom) ; vous pouvez tout corriger avant « Enregistrer ». L'étude créée est **vide** : récupérez
  ses données (« Récupérer (fournisseur) ») ou saisissez-les.

### 6.4 Après la validation

- Une cellule validée porte **« ★ »** : c'est une valeur proposée par l'IA et validée par vous. Elle
  compte comme votre propre saisie, marquée « ? » (à revoir) ; votre prochaine modification de la
  cellule retire la marque, l'historique la garde. La ligne « Source » de la cellule dit « proposée
  par l'IA, validée le … ».
- Un jugement validé se lit sur le graphique avec la légende « placée par l'IA · validée le … ».
- Dans le **PDF** de l'étude, une valeur d'origine IA est marquée **« † »**, avec sa note.
- L'**historique** de l'étude montre les propositions traitées (entrées « ★ », avec « Détail »).
- Si l'étude a un **verdict figé**, le verdict actuel en diffère désormais ; la comparaison dit
  « Cause : proposition de l'IA validée ». Revalidez l'étude (« Valider l'étude ») si le nouveau
  verdict doit devenir votre référence.

---

## 7. Dépannage

### 7.1 Le serveur ne se connecte pas

| Symptôme | Cause probable | Que faire |
|---|---|---|
| `claude mcp list` ne montre pas `steadyinvest` | vous n'êtes pas dans `~/steadyinvest-ia` | `cd ~/steadyinvest-ia` (l'enregistrement est `local` à ce répertoire) |
| `✘ Failed to connect` | chemin du binaire faux, ou binaire pas construit | vérifier le chemin, `just mcp-build` |
| `Connected · tools fetch failed — Invalid result for tools/list … ttlMs … cacheScope` | binaire construit avant le correctif du 30/09/2026 (PR #268) | mettre le dépôt à jour, `just mcp-build`, relancer `claude` |
| l'IA dit « aucun dossier n'a pu être déterminé » | pas de `--dossier` et aucun dossier ouvert dans l'application | enregistrer avec `--dossier`, ou ouvrir un dossier dans l'application |
| « La configuration de l'application n'a pas pu être lue » | `config.json` invalide ou chemin relatif dedans | ouvrir l'application une fois (elle répare), ou enregistrer avec `--dossier` |

### 7.2 L'IA reçoit un refus

Chaque refus porte un **code** (en anglais, stable) et un **message** en français que l'IA vous
relaie. Aucun refus n'écrit quoi que ce soit. Les plus courants :

| Code | Signification |
|---|---|
| `write_denied` | écriture refusée : seule la création de propositions est permise |
| `field_not_draftable` | ce champ ne peut pas être proposé (cours actuel, BPA 12 mois…) |
| `year_not_in_study` | l'année n'existe pas dans l'étude ; une proposition n'ajoute jamais d'année |
| `value_unparsable` / `value_not_an_option` / `value_out_of_range` | valeur illisible, hors des options, ou hors des bornes |
| `target_has_pending` | la cible a déjà une proposition en attente |
| `study_exists` / `draft_study_pending` | l'étude existe déjà, ou est déjà proposée, dans cette devise |
| `study_archived` | l'étude est archivée : désarchivez-la d'abord |
| `study_not_found` | l'étude n'existe pas dans ce dossier |
| `empty_comment` / `missing_origin` / `empty_note_text` / `text_too_long` | commentaire, origine ou texte manquant ou trop long |
| `dossier_mismatch` / `dossier_replaced` | le dossier a changé (ou a été restauré) depuis la lecture |
| `schema_mismatch` | le dossier et le serveur ne sont pas de la même version : ouvrez le dossier dans l'application, reconstruisez le serveur |
| `dossier_needs_recovery` / `restore_interrupted` | ouvrez d'abord le dossier dans l'application, qui termine la reprise |
| `dossier_busy` / `dossier_locked` | une restauration ou une écriture occupe le dossier : réessayer |
| `not_a_dossier` / `dossier_protected` | le fichier n'est pas un dossier SteadyInvest, ou il est protégé en écriture |

La liste complète est au §3.3 de
`_bmad-output/planning-artifacts/ux-ai-assistance-surfaces.md`.

### 7.3 Les propositions n'apparaissent pas dans l'application

- Vérifiez le filtre **« Étude : »** (section 6.2) et la sorte sélectionnée.
- Vérifiez que l'application a ouvert **le même dossier** que celui passé à `--dossier` (chemin
  dans Réglages).
- Une bande « Les propositions n'ont pas pu être lues ; la liste est indisponible (…) » nomme la
  cause ; le journal de l'application en dit plus.

### 7.4 Les journaux

`~/.local/share/steadyinvest/logs/steadyinvest-mcp.log.<date>` note chaque démarrage (avec le
dossier), chaque appel d'outil, chaque proposition enregistrée et chaque refus avec son code.
**Aucun texte écrit par l'IA et aucune clé n'y figurent.**

---

## 8. Autres systèmes, autres clients (non vérifiés)

- **macOS / Windows** : le serveur suit les emplacements standard du système (bibliothèque
  `directories`) — configuration et données sous `~/Library/Application Support/steadyinvest`
  (macOS) ou `%APPDATA%\steadyinvest\…` (Windows). Ces chemins n'ont pas été vérifiés ; le binaire
  s'appelle `steadyinvest-mcp.exe` sous Windows.
- **Autres clients IA** : tout client MCP qui lance un serveur par **stdio** convient. Il lui faut
  la commande (le chemin du binaire) et ses arguments (`--dossier <chemin>`). Enregistrez-le de
  façon à ce qu'il ne soit **pas** actif dans le dépôt SteadyInvest.
- **Protocole** : le serveur parle les versions MCP 2024-11-05 à 2025-11-25, et répond aux clients
  plus récents (2026-07-28) avec les indications de cache qu'ils exigent.

---

## Annexe A — Les concepts MCP, pour qui ne les connaît pas

Cette annexe explique, sans prérequis technique, les notions qu'on rencontre dans le guide.

### A.1 Qu'est-ce que MCP ?

**MCP** (*Model Context Protocol*) est une convention publique qui permet à un assistant IA
d'utiliser des programmes extérieurs de façon contrôlée. On peut la comparer à une prise
électrique normalisée : l'IA d'un côté, un logiciel de l'autre, et une forme de prise commune
qui dit ce qui peut passer.

Sans MCP, une IA ne connaît que ce qu'on lui colle dans la conversation. Avec MCP, elle peut
**demander** à un programme de lui fournir des informations (lire une étude) ou d'effectuer une
action précise (déposer une proposition) — mais **uniquement** ce que ce programme a choisi de
permettre.

### A.2 Les trois acteurs

| Acteur | Chez vous | Rôle |
|---|---|---|
| **Le modèle** | le modèle d'IA (par exemple Claude), qui tourne chez son fournisseur | comprend vos demandes, raisonne, décide quel outil appeler |
| **Le client MCP** | le programme que vous utilisez pour parler à l'IA : **Claude Code** (`claude`) | lance les serveurs MCP, transmet à l'IA la liste de leurs outils, exécute les appels qu'elle demande et lui rapporte les réponses |
| **Le serveur MCP** | **`steadyinvest-mcp`**, sur votre machine | offre des outils précis sur vos données, et rien d'autre |

L'IA ne touche donc **jamais** directement votre fichier : elle demande au client, qui demande au
serveur, qui applique ses propres règles. C'est le serveur qui décide de ce qui est permis.

### A.3 Un « outil »

Un **outil** est une action nommée que le serveur met à disposition, avec une description et des
paramètres : `get_study` (« lis l'étude dont voici l'identifiant »), `submit_draft_note` (« dépose
cette proposition de note sur cette étude »). L'IA lit ces descriptions et choisit, selon votre
demande, quel outil appeler avec quels paramètres.

Le serveur SteadyInvest en offre huit (section 5) : cinq qui **lisent**, trois qui **déposent des
propositions**. Aucun outil ne modifie une étude — il n'en existe tout simplement pas.

### A.4 « stdio » et « lancer le serveur »

Il existe deux façons de joindre un serveur MCP : par le réseau (une adresse web), ou en le
**lançant comme un programme local** et en lui parlant par son entrée et sa sortie standard
(*stdio*). SteadyInvest utilise la seconde : c'est **votre client IA qui démarre**
`steadyinvest-mcp` quand il en a besoin, sur votre machine, et l'arrête ensuite.

Conséquences :

- rien n'est ouvert sur le réseau : aucun autre ordinateur ne peut joindre le serveur ;
- vous n'avez **pas** à lancer le serveur vous-même, ni à le laisser tourner ;
- le client doit savoir **quel programme lancer, avec quels arguments** : c'est exactement ce que
  vous lui dites avec `claude mcp add … -- <programme> --dossier <chemin>`.

Vos données restent dans votre dossier ; seules les informations que l'IA demande par les outils
(le contenu d'une étude, par exemple) passent dans la conversation, donc chez le fournisseur du
modèle, comme tout ce que vous lui écrivez.

### A.5 « Enregistrer » un serveur, et la « portée »

**Enregistrer** un serveur, c'est inscrire dans la configuration du client : « il existe un
serveur nommé `steadyinvest`, voici comment le lancer ». La **portée** dit **où** cette
inscription est valable :

| Portée | Valable… | Pour SteadyInvest |
|---|---|---|
| `local` | dans **un seul répertoire**, sur cette machine | **celle à utiliser**, depuis `~/steadyinvest-ia` |
| `project` | pour tous ceux qui ouvrent ce projet (fichier `.mcp.json` versionné) | à éviter |
| `user` | dans **tous** vos projets | à éviter : le serveur serait visible partout, y compris dans le dépôt de développement |

C'est pourquoi le guide vous fait créer un répertoire dédié (`~/steadyinvest-ia`) et lancer
`claude` depuis celui-ci.

### A.6 Une « proposition » (ou brouillon)

Dans d'autres logiciels, un serveur MCP écrit directement. SteadyInvest a choisi l'inverse : tout
ce que l'IA « écrit » est une **proposition** — un brouillon rangé à part, avec son commentaire et
son origine (quel client, quel modèle). Il attend dans l'écran **Propositions** jusqu'à ce que
**vous** le validiez, le modifiiez ou le rejetiez. Tant que ce n'est pas fait, il ne change **aucun**
chiffre, aucune zone, aucun verdict. Le **Registre** garde la trace de toutes les propositions et de
ce que vous en avez fait.

### A.7 Les « refus » et leurs codes

Quand un appel ne peut pas aboutir (une valeur illisible, un champ qu'on ne peut pas proposer, un
dossier qui a changé), le serveur **refuse** : il n'écrit rien et renvoie un **code** stable en
anglais (`field_not_draftable`, …) et un **message** en français. L'IA vous relaie en général le
message ; le code sert au dépannage (section 7.2) et figure dans le journal du serveur.

### A.8 Petit lexique

| Terme | Sens |
|---|---|
| **Client MCP** | le programme qui relie l'IA aux serveurs (ici Claude Code) |
| **Serveur MCP** | le programme qui offre des outils (ici `steadyinvest-mcp`) |
| **Outil** (*tool*) | une action nommée qu'un serveur offre à l'IA |
| **stdio** | le mode où le client lance le serveur comme un programme local et lui parle directement |
| **Enregistrement** | l'inscription du serveur dans la configuration du client |
| **Portée** (*scope*) | là où l'enregistrement est valable (`local`, `project`, `user`) |
| **Proposition** (*draft*) | ce que l'IA dépose ; sans effet tant que vous ne l'avez pas validé |
| **Dossier** | votre fichier SteadyInvest (études, portefeuille, propositions) |
| **`--dossier`** | l'argument qui fixe le dossier lu par le serveur |
| **Version de protocole** | la version de la convention MCP que client et serveur conviennent d'utiliser au démarrage |
