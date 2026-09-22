# Application Zedflow — composition et suivi ADK

Le produit associe les neuf crates de `rust/`, le SDK Zod et les adaptateurs Vue,
un client Vue/Vite et une coque Electron. Les graphes ciblent ADK **2.2.0**.
Voir [architecture](architecture/crates.md) et [SDK](development/sdk.md).

## Lancer une instance isolée

Node 24, pnpm 11.5.1 et Rust 1.96.1 sont requis. Depuis la racine du dépôt :

```sh
REPO="$(pwd -P)"
FIXTURE="$(mktemp -d /tmp/zedflow-demo.XXXXXX)"
mkdir -p "$FIXTURE/workspace" "$FIXTURE/home"
pnpm --dir web install --frozen-lockfile
pnpm --dir web build
(cd rust && CARGO_TARGET_DIR=/tmp/zedflow-adk-target cargo run --locked -p zf-serve --bin zedflow-daemon -- \
  --workspace "$FIXTURE/workspace" --data "$FIXTURE/data" \
  --flow-home "$FIXTURE/home" --context-home "$FIXTURE/home" \
  --web "$REPO/web/apps/client/dist" --listen 127.0.0.1:3143)
```

Ouvrir http://127.0.0.1:3143. Dans un autre terminal :
`ZEDFLOW_CLIENT_URL=http://127.0.0.1:3143 pnpm --dir web --filter @zedflow/desktop start`.
Fermer Electron ne termine pas le daemon. Interrompre le daemon au premier plan
après une frontière inactive. Chaque instance possède son propre répertoire data.
Aucune option home ci-dessus ne remplace HOME, CODEX_HOME ou les credentials.

Pour le développement Vite, `pnpm --dir web dev` démarre le client ; le daemon
reste un processus séparé (voir `tooling/dev`). Les flows nouveaux sont des
[packages](development/flow-packages.md), les formats historiques restent lisibles.
Le daemon gère les workspaces, sessions et checkpoints sur sa machine. L'accès
distant exige un tunnel privé ; ne pas exposer son écoute sur le réseau public.

## Versions du client et du daemon

Le footer distingue le client chargé, les assets servis et le daemon. Le
[parcours supervisé](app-versioning.md) prépare un candidat sans l'activer et
refuse une mise à jour pendant une exécution active. Une instance directe reste
possible sans superviseur. Les opérations sur service/données personnels exigent
un mandat distinct : les commandes de test ne les activent pas.

## Parcours

1. L’application ouvre **Exécution** : la sidebar regroupe les sessions sous leurs workspaces. « Ouvrir un workspace » parcourt les dossiers de la machine du daemon, avec chemin direct, navigation parent et dossiers cachés optionnels.
2. Créer une session, choisir son flow d’entrée ou une composition de flows et les modèles, puis envoyer la première demande. Les flows interactifs ont une session ; les runs autonomes restent consultables sous « Exécutions autonomes » en Exécution. Le titre initial d’une session vient de la demande et peut être renommé.
3. Le menu de contexte de l’en-tête résume le workspace, le flow, les modèles et le statut. Il ouvre l’inspecteur détaillé : graphe, passages des nœuds, état, checkpoints, événements et contexte. Sur un écran étroit, cet inspecteur se superpose au chat.
4. **Conception** réunit le catalogue global/local, le canvas, la palette repliable et les propriétés ADK. Les modèles proposés couvrent le harness, l’interaction, les outils et une exécution autonome. Le brouillon et la session restent distincts lorsque l’on change d’espace.
5. Enregistrer écrit un fichier Rust ; Enregistrer sous choisit une copie globale ou dans le workspace. Rust exporte un projet Cargo comprenant le fichier source exact ; Compiler lance `cargo check`. Préparer une session depuis le canvas sauvegarde sa définition avant de proposer l’entrée du flow.
6. Répondre à une attente reprend son checkpoint. Fermer un workspace dans la sidebar ne supprime ni ses sessions ni leurs checkpoints et n’arrête pas le travail. Un autre client peut suivre la même session ; deux réponses concurrentes à la même attente ne sont pas toutes deux acceptées.
7. La timeline affiche messages et outils dans leur ordre persisté. Les progrès et résultats complètent le bloc outil à son emplacement. Les réponses utilisent `MessageResponse` d’AI Elements Vue pour le Markdown. Le détail technique des passages du graphe reste dans l’inspecteur.
8. Le fournisseur Fixture est déterministe et sans appel LLM. Les appels live requièrent un fournisseur configuré sur le daemon ; ils ne font pas partie des tests automatiques.

## Composer le contexte dans un flow

Les nouveaux starters dessinent deux passages distincts : **Contexte → Modèle**.
Le premier choisit la stratégie enregistrée, lie ses ressources et accorde ses
capacités. Le second choisit le fournisseur et le modèle et consomme cette fenêtre.
Toutes les entrées du Modèle passent par son Contexte, y compris au retour d’un
outil ou d’une réponse humaine. Le graphe et l’inspecteur montrent leurs passages
séparés, reliés par des connexions orientées.

L’ouverture du workspace ajoute les stratégies manquantes `workspace-default`
(instructions, skills, fichiers et outils read/write/edit/exec),
`conversation-default` (sans outils) et `tools-default` (inspect_json). Elles sont
éditables dans `.zedflow/context/` ; une source déjà présente n’est jamais remplacée.
Les anciens flows restent exécutables. « Séparer Contexte et Modèle » prépare une
nouvelle copie ; les montages non convertibles automatiquement sont signalés.

Le studio explique **Sources → Programme → Contexte du modèle**. Déclarer une
source ne l’envoie pas : « Ajouter au programme » crée le fragment correspondant.
Les exemples « Instructions et demande » et « Résultat d’outil conditionnel »
ouvrent des brouillons avec données d’aperçu. Ces données ne sont pas enregistrées
dans le Rust de la stratégie.

« Exemple multiflow » crée un harness, un flow Working System indépendant et le
bridge `working-system`. Choisir le dossier de documentation sur la machine du
daemon (par exemple `../docs` depuis Zedflow), puis **Utiliser**, activer le bridge
et choisir les deux modèles. Le harness attend le résultat documentaire avant de
préparer sa restitution. Le flow documentaire expose read/exec ; son périmètre
métier détaillé reste à définir. Créer l’exemple ne lance aucune exécution et ne
réécrit pas une définition existante.

Les paramètres du graphe portent un **Dossier de travail du flow**. Un chemin
absolu désigne un dossier sur le daemon, un chemin relatif part du workspace (ou
du dossier hérité pour un sous-graphe). Les outils, fichiers relatifs, instructions
et skills utilisent ce dossier. Les différentes instances gardent le même store,
les mêmes reçus et la même annulation du run ; le cwd du processus ne change pas.

## Catalogue de fichiers

Le [context engine](context-api.md) ajoute les stratégies Rust de `.zedflow/context`,
les bibliothèques de projections, les types et les bridges. En Conception, l’éditeur
de blocs prépare les stratégies et leurs fenêtres depuis des ressources fixtures.
Les nœuds choisissent leurs sources et bindings explicites. « Composer une
exécution » résout le flow d’entrée, les bridges et les choix de stratégies/modèles
de chaque instance avant le lancement ADK. Le [plan](context-engine.md) détaille
les contrats, la publication de révisions et les limites des adaptateurs.

Les racines locales sont `<workspace>/.zedflow/flows` et `<workspace>/.agents/flows`. Les racines globales sont `~/.zedflow/flows` et `~/.agents/flows` (ou sous `--flow-home`). Les nouveaux fichiers vont par défaut dans `.zedflow/flows`. Un fichier existant reste à son emplacement lors d’une sauvegarde. Les fichiers Rust des sous-dossiers sont découverts ; les liens de fichiers sont signalés et restent hors édition.

L’identité de fichier tient compte de son chemin canonique. Les homonymes des différentes racines restent distincts et portent leur provenance. La découverte est relancée à l’ouverture du workspace, au retour au premier plan et via Actualiser. Un fichier invalide reste visible avec son diagnostic. L’empreinte du fichier lu sert au contrôle de concurrence ; les sauvegardes utilisent un remplacement atomique et refusent une version obsolète.

Au premier démarrage après mise à jour, les anciennes compositions SQLite sont sauvegardées dans `before-file-flows.sqlite`, exportées dans le workspace initial, puis leur table est supprimée. Les routes historiques `/api/compositions` restent des adaptateurs du catalogue de fichiers pour lecture et création ; les mises à jour exigent `/api/flows` avec la clé de fichier et le hash chargé. Elles n’utilisent plus une table de définitions. Les runs conservent les octets Rust et les empreintes de leurs définitions exécutées. Enregistrer publie désormais une nouvelle révision pour les prochains passages compatibles des runs concernés ; les passages déjà engagés gardent leur version. Supprimer une source ne supprime pas les définitions historiques.

Les nouvelles interfaces sont `/api/workspaces`, `/api/filesystem` et `/api/flows`. Les listes de flows, le contexte, les modèles et les sessions acceptent `workspaceId`. Un lancement avec `flowKey` exige le `flowHash` choisi ; le daemon refuse une source modifiée entre sélection et lancement. L’ancien lancement avec composition embarquée reste disponible pour les aperçus API isolés.

`/api/generate` et `/api/build` acceptent aussi `runId` pour exporter ou compiler la source exacte d’une session, indépendamment des modifications ultérieures du flow. Les sessions antérieures au format Rust n’ont pas cette source et ne proposent pas cet export.

## Harness de workspace

Le starter **Harness de workspace** v2 fournit une boucle complète : Début
→ Réorientation → Appel modèle. Les instructions, skills et outils sont des pièces
attachées à l'agent, et non des étapes de contrôle. Une condition dirige les appels demandés vers un
outil à la fois ; une seconde condition termine le lot avant de revenir au contrôle
de réorientation et au modèle. Une réponse sans outil passe par Réponse puis Suite
du travail, qui consomme un message en file ou attend une entrée humaine. Chaque
transition reste une arête du graphe ADK.

Le panneau Modèles présente les nœuds configurables, y compris ceux des
sous-compositions. Un nœud avec `modelBinding: runtime` attend une sélection lors
de sa première invocation si elle manque ; une branche non visitée ne bloque pas
le lancement. Les nœuds à modèle fixe gardent leur configuration. Un changement
de sélection s'applique à une prochaine invocation et conserve la version du
graphe lancé. Les révisions refusent une modification concurrente obsolète. Les attentes
identifient le chemin du nœud et son occurrence précise dans le parcours.

Le catalogue Codex est lu depuis le cache du CLI sur l'hôte ; cette liste ne
prouve pas l'accès du compte à un modèle. Les modèles Gemini déjà référencés par
les compositions restent sélectionnables. Le réglage d'effort de raisonnement,
de résumé et de verbosité est celui du transport Codex. `thinkingBudget` Gemini
n'est pas intégré et est refusé ; les paramètres d'échantillonnage Gemini restent
ceux de la définition du nœud.

Une réorientation est reçue après le lot d'outils courant, avant une prochaine
invocation du modèle. Un message « En file » attend la fin du travail courant,
au nœud Suite du travail. Un message encore disponible peut être retiré ; une
consommation déjà engagée est refusée à la suppression. Arrêter déclenche
l'annulation du modèle ou du groupe de processus `exec`. Reprendre utilise les
checkpoints de l'exécution, sans relancer automatiquement un appel outil dont
l'effet est devenu incertain. Choisir un autre flow prépare une nouvelle exécution.

## Instructions et skills

Un agent v2 sans pièces ne reçoit aucun contexte implicite. La découverte fournit
des sources disponibles ; seules les pièces attachées autorisent leur utilisation
par cet agent. Le lecteur et le runtime v1 conservent le comportement historique
des anciennes sessions. La conversion v1 → v2 crée une nouvelle copie et refuse
les montages de contexte qui ne peuvent pas être traduits fidèlement.

Les instructions disponibles sont capturées pour un nouveau run depuis `~/.pi/agent/`, puis
les ancêtres de la racine jusqu'au répertoire du workspace. Dans chaque dossier,
la priorité est `AGENTS.override.md`, `AGENTS.md` / `AGENTS.MD`, puis `CLAUDE.md` /
`CLAUDE.MD`. Les descendants ne sont pas parcourus automatiquement. Les chemins,
contenus d'instructions et empreintes sont inspectables dans le contexte.

Les racines automatiques de skills sont parcourues dans cet ordre :

1. `<workspace>/.pi/skills`.
2. `.agents/skills` du workspace puis de ses ancêtres, jusqu'à la racine Git
   incluse ; hors dépôt Git, jusqu'à la racine filesystem.
3. `~/.pi/agent/skills`, puis `~/.agents/skills`.

`--skill-dir /chemin` ajoute une racine ou un fichier explicite prioritaire ;
l'option peut être répétée. `~/.codex/skills` peut ainsi être ajouté. La découverte
respecte les fichiers d'exclusion usuels, s'arrête à une racine contenant
`SKILL.md`, déduplique les liens vers le même fichier et signale les noms en
collision. Le premier nom découvert gagne. Les packages et extensions Pi ne
sont pas chargés.

En v2, la pièce Skills définit le catalogue autorisé pour un agent. Ce catalogue
contient les descriptions et chemins ; les corps sont injectés uniquement après
activation explicite ou lorsqu'une ressource est configurée comme permanente.
Une commande `/skill:nom arguments` cible un agent, active sa ressource et conserve
les arguments dans le message ; elle fonctionne aussi avec la file et la
réorientation. Dans un flow contenant plusieurs agents, le composer permet de
choisir ce destinataire. Le panneau Contexte expose les activations indépendantes
et le contenu effectivement chargé à chaque appel. Les métadonnées d'un skill
n'accordent aucun outil. La lecture historique v1 reste inchangée.
`disable-model-invocation: true` masque le skill du catalogue automatique tout en
laissant l'invocation explicite disponible. Un nom inconnu ou un fichier devenu
illisible produit une erreur. Les références relatives partent du dossier du skill.
Les chargements observés conservent leur provenance et leur empreinte ; un aperçu
tronqué est distingué d'un chargement complet.

Les pièces Instructions acceptent du texte littéral ou template, un fichier ou les
instructions découvertes du workspace. Les pièces Fichiers sélectionnent un chemin,
une portion de lignes et une limite de contenu. Les pièces Outils énumèrent les
primitives accordées. Le runtime capture cette configuration avant l'appel modèle
et persiste la provenance de chaque demande d'outil : agent, passage, invocation et
capacités applicables. Le dispatcher la vérifie avant tout effet, puis réutilise le
reçu d'une demande déjà exécutée. Un outil explicitement programmé dans le graphe
conserve sa propre configuration.

## Navigation, chronologie et inspection

La sidebar affiche cinq sessions récentes par workspace, puis « Voir plus ». Les
actions secondaires sont accessibles par menu au survol, au focus et au clavier.
Sa largeur initiale est de 280 px, réglable entre 220 et 440 px avec la souris ou
les flèches du séparateur ; la préférence est mémorisée. Sur petit écran elle
devient un tiroir. Les starters se trouvent dans « Créer un flow ».

Le chat centre les messages et le composer. Les sélecteurs de flow et de modèles
s'ouvrent dans des popovers ; les modèles restent configurables indépendamment par
agent. Une attente texte, confirmation ou choix de modèle transforme le composer.
Sa réponse porte l'identité de l'attente pour refuser une reprise périmée.

La timeline conserve un ordre daemon commun et, lorsqu'elle est connue, l'origine
`{ nodePath, occurrenceId }`. Les progrès d'outils mettent à jour leur entrée sans
la dupliquer. Le rail de passages ouvre la même sélection que le graphe et les
listes d'activités ; les étapes sans message peuvent être dépliées. L'absence
d'origine précise dans un ancien historique est signalée dans les détails.

« Agrandir le graphe » utilise l'espace principal ; revenir au chat conserve son
brouillon, son défilement et la sélection du passage. Le graphe garde des caméras
distinctes pour les vues compacte et agrandie. Les détails exposent les entrées,
sorties, erreurs, durées, contexte chargé et appels associés. Une sélection manuelle
de passage reste fixée lorsqu'une occurrence plus récente arrive.

Le canvas et l'inspection utilisent le même routeur orthogonal local
`libavoid-js` 0.4.5. Les nœuds et pièces sont des obstacles ; leurs déplacements et
changements de dimensions déclenchent le recalcul des chemins. L'alignement
optionnel sur la grille de 16 px concerne les nœuds déplacés, sans réarranger les
positions existantes. Début/fin, conditions, entrées/sorties et sous-graphes ont des
formes distinctes. Les ports Oui/Non indiquent les branches des conditions.

## Partager une session

SQLite reste la source de vérité, avec un espace logique par workspace. Fermer un
workspace masque ses sessions ; cela ne les archive pas et ne les supprime pas.
Les accès aux événements, checkpoints et fichiers d'une session vérifient son
appartenance au `workspaceId` demandé. Les anciennes requêtes locales sans ce
paramètre sont limitées au workspace par défaut.

« Exporter la session » crée explicitement
`<workspace>/.zedflow/sessions/<session-id>/` et propose une archive ZIP du même
contenu. Le dossier contient `session.jsonl`, un inventaire versionné avec hashes,
la source Rust figée, la configuration, la conversation et les événements,
les passages, les checkpoints imbriqués, les contenus de contexte disponibles et
les reçus et sorties d'outils. La capture est verrouillée ; une session active doit
atteindre une pause ou sa fin avant l'export. Cette action ne l'arrête pas.

« Importer une session » choisit un workspace cible et un chemin du daemon. Les
formats, chemins d'archive et empreintes sont vérifiés avant de rendre la session
visible. Réimporter un export identique est idempotent ; un contenu différent avec
le même identifiant est refusé sans écrasement. Les références techniques et
séquences sont remappées, sans modifier les commandes ni le texte utilisateur.
L'import n'exécute rien ; la reprise est une action distincte, proposée seulement
si les contrôles du runtime, des checkpoints et des ressources le permettent.

Un export transporte une session, pas le dépôt ni les effets externes déjà
produits. Le workspace cible doit fournir les ressources encore nécessaires.
Les données historiques manquantes et obstacles à la reprise sont exposés dans
les diagnostics. Les flows et exports choisis sont versionnables ; les fichiers
temporaires et bases restent ignorés par `.zedflow/.gitignore`. Aucun export
automatique, ajout Git ou commit n'est effectué.

## Stockage, synchronisation et maintenance

Le stockage de session v2 est indépendant des formats Rust v1/v2. Dans
`zedflow.db`, `runs` ne contient que les métadonnées ; `run_entities` conserve
les projections par identité, `events` le journal ordonné et `run_changes` les
opérations rejouables. `zf_content` contient les valeurs immuables adressées par
SHA-256 et `zf_content_edges` leurs dépendances. Les objets sont des manifestes
et les tableaux des séquences à préfixes partagés ; l'ajout d'un message ne
réenregistre pas ses prédécesseurs. Les longues chaînes partagent également
leurs fragments. Des événements distincts gardent leurs identités même lorsqu'ils
référencent le même contenu.

`StoredCheckpointer` implémente le trait ADK `Checkpointer` et conserve tous les
champs natifs, notamment les interruptions, tentatives et registres des
sous-graphes. Les passages capturent l'état de leur `NodeContext` réel ; le numéro
d'étape seul n'identifie pas l'entrée après reprise. Les invocations et reçus
restent des enregistrements distincts dans `zf_records`, avec des références vers
leurs contenus. Les requêtes modèle peuvent être reconstruites depuis leur
manifeste. Le runtime continue à matérialiser le contexte requis par ADK et le
fournisseur : ce format supprime les copies persistées, pas toutes les allocations.

Une connexion persistante écrit les données de session, avec un pool de lecture,
SQLite WAL et `synchronous=FULL`. La file d'observation est bornée ; le consumer
regroupe au plus 64 événements ou 10 ms avant de commencer une écriture. Le temps
de traitement d'un lot peut dépasser cette fenêtre. Les aperçus d'outils peuvent
être regroupés, mais leurs fragments complets sont déjà durables. Les garanties
restent : provenance avant autorisation d'outil, reçu initial avant effet, reçu
terminal avant consommation et checkpoint avant publication d'un état reprenable.
Un navigateur qui ne lit plus son flux ne bloque pas cette progression.

`GET /api/runs` renvoie des résumés. `/runs/{id}/snapshot` fournit un bootstrap
avec la page récente de conversation et les passages légers ; `?after=<cursor>`
renvoie des deltas ou un heartbeat. SSE (`events`, événement `sync`) et RTC
emploient les mêmes messages. Les abonnés sont avertis après commit, avec un
heartbeat toutes les cinq secondes ; le client attend douze secondes avant de
considérer un transport silencieux. Les détails sont chargés par les routes
`activities/{occurrence}`, `tools/{call}`, `tools/{call}/output`,
`context/{invocation}`, `state`, `flow-source`, `event-history` et `timeline`.
Les pages ne coupent pas les éléments qui partagent une même séquence. Les
retransmissions sont ignorées par révision et identité, sans remplacer les objets
inchangés. `GET /runs/{id}` reste une lecture complète explicite de compatibilité.
Les index d'invocations ne recopient pas leur catalogue de skills : les suggestions
`/skill:` et le panneau de contexte chargent le dernier contexte de l'agent à
l'usage, dans le même cache versionné. Les erreurs du transport sont limitées à
512 caractères avec une indication de troncature ; les détails gardent le texte
intégral. Ces résumés s'appliquent aussi aux anciens deltas lors de leur lecture.

Le footer global donne accès à Exécution/Conception, même sidebar masquée. Il
indique la machine du daemon ; son détail affiche identité persistante, instance,
endpoint et transport. La présence du daemon est suivie sans session ouverte.
Les mesures de l'inspecteur distinguent les durées des passages, les intervalles,
l'encodage, l'attente du writer, la transaction, la publication et le traitement
côté client. Ces mesures sont diagnostiques et ne constituent pas un benchmark Pi.

Les nouvelles archives v2 transportent leur fermeture de contenus une seule fois,
avec un journal de références, les checkpoints et les reçus. Le lecteur v1 reste
pris en charge. L'import vérifie les empreintes, la fermeture et les limites de
décodage avant de rendre la session visible ; il n'exécute aucun nœud. L'export
Cargo embarque le codec et le checkpointer requis avec la source Rust exacte.

La maintenance de nettoyage est une commande explicite hors ligne. Arrêter le
daemon lorsqu'aucune exécution n'est active, puis lancer :

```sh
(cd rust && CARGO_TARGET_DIR=/tmp/zedflow-adk-target cargo run --locked -p zf-serve --bin zedflow-daemon -- \
  --data /chemin/vers/.zedflow \
  --maintain-keep-session IDENTIFIANT --maintenance-cutoff HORODATAGE_UNIX_MS)
```

Elle sauvegarde les bases et fichiers, prépare un stockage compact dans un dossier
voisin, vérifie les données reconstruites et bascule les répertoires par renommage.
Un verrou et un marqueur permettent de reprendre une opération interrompue. La
sauvegarde est conservée. Les sessions postérieures au cutoff, ou dont l'ancienneté
ne peut pas être prouvée, restent présentes. Les workspaces, flows et fichiers du
projet sont conservés. Cette commande ne réexécute pas les graphes.

## Paramétrage disponible

`GET /api/capabilities` expose la version ADK, les champs appliqués, les outils locaux, les renderers et les capacités non intégrées. Ce catalogue décrit ce backend précis ; il ne prétend pas que toute fonctionnalité présente dans un crate ADK est exécutable par l'application.

| Niveau | Configuration appliquée |
| --- | --- |
| Graphe racine | Nombre maximal de super-étapes (`recursionLimit`), concurrence (`maxConcurrency`), canaux stricts, timeout par nœud et timeout d'inactivité |
| Reprises sur erreur | Nombre total de tentatives, délai initial et maximal, facteur exponentiel, jitter et filtre toutes erreurs ou timeout ; politique du graphe et surcharge par nœud |
| État | Canaux nommés, valeurs initiales, reducers ADK overwrite, append et sum |
| Arrivées d'un nœud | `fanIn: all` conserve la jonction ADK des arêtes directes ; `fanIn: any` accepte les arrivées alternatives et convient au retour vers un modèle depuis Outils ou Attente |
| Modèle | Fournisseur, identifiant, instructions locales/globales, description, canal d'entrée, canal de sortie et historique, outils déclarés, sortie texte ou JSON et schéma transmis au fournisseur |
| Échantillonnage Gemini | Température, top-p, top-k, tokens maximaux, séquences d'arrêt |
| Codex | Modèle, effort de raisonnement, résumé de raisonnement et verbosité ; les paramètres d'échantillonnage non pris en charge sont refusés |
| Outil | Outil fixe avec arguments JSON ou canal d'entrée, ou `execute_calls` pour les demandes du modèle ; canal de sortie et renderer associé au nœud |

Les outils de démonstration `inspect_json`, `format_text` et `delay` restent des
`adk_tool::FunctionTool`. Le harness ajoute les outils suivants :

| Outil | Comportement intégré |
| --- | --- |
| `read` | Lecture UTF-8 ADK avec numéros de ligne, `offset` à partir de 1 et `limit` ; aperçu de 2 000 lignes / 50 Kio et indication de continuation |
| `write` | Création ou remplacement complet UTF-8 via ADK, avec création des répertoires parents |
| `edit` | Remplacement exact ADK `old_string` → `new_string`, cible unique sauf `replace_all: true` ; lecture préalable dans ce run exigée, ou fichier créé par `write` |
| `exec` | Commande Bash dans le cwd du run, environnement du daemon, timeout optionnel en secondes, sortie progressive combinée stdout/stderr, aperçu des dernières 2 000 lignes / 50 Kio, fichier de sortie complet référencé |

Les chemins peuvent être relatifs au workspace, absolus ou commencer par `~/`.
La racine d'accès des outils ADK est `/` : le workspace détermine le cwd, sans
restreindre les droits du processus aux seuls fichiers du projet. `exec` utilise
les droits et l'environnement de l'utilisateur du daemon. Il ne fixe aucun
timeout propre par défaut ; une politique du graphe peut néanmoins limiter le
nœud. Annulation et timeout arrêtent son groupe de processus Unix. Les processus
de fond ne constituent pas des sessions gérées.

La lecture d'images, l'édition multi-blocs et le rapprochement Unicode de Pi ne
sont pas repris : les outils fichiers conservent leur sémantique ADK. Les outils
MCP et les nœuds d'action réseau/base restent à intégrer. Le dispatcher laisse
toujours la transition suivante aux arêtes du graphe.

Les reçus d'outils sont stockés avant l'effet, puis complétés avec le résultat ou
l'erreur. Une reprise du même appel restitue son résultat connu. Un reçu commencé
sans résultat bloque sa répétition automatique ; cela ne garantit pas qu'un
effet externe n'a eu lieu qu'une fois. Les reçus et les sorties complètes résident
dans le magasin de contenus SQLite. Les fragments stdout/stderr sont validés avant
leur aperçu, et le reçu final référence leur séquence. Les anciens fichiers de
`runs/<runId>/receipts/` et `runs/<runId>/tool-output/` restent lisibles pour la
compatibilité ; les fichiers de sortie produits pendant un appel sont des caches.
Le téléchargement explicite d'une sortie renvoie ses octets exacts depuis SQLite.

L'historique du modèle conserve les `Content`/`Part` ADK, dont les identifiants d'appels et les signatures opaques requises par certains fournisseurs. Ses canaux et ceux des appels utilisent overwrite, y compris lorsqu'ils ont un nom personnalisé. Les valeurs de template sont substituées une seule fois ; le contenu d'une valeur n'est pas réinterprété comme un autre template.

Le nœud modèle fait une requête `adk_core::Llm` et expose `hasToolCalls`, `toolCalls`, `messages` et `modelResponse`. Le nœud Outils expose `toolResults`. Des marqueurs internes distinguent une nouvelle réponse humaine d'un simple retour d'outil, y compris si le texte humain est identique au précédent. La fixture est déterministe : les paramètres d'échantillonnage ne modifient pas son résultat ; elle n'est pas un LLM local. Un schéma de sortie est transmis au fournisseur ; la sélection JSON exige aussi un résultat JSON parseable, sans promettre une validation locale complète de tout JSON Schema.

Le template Harness fixe `recursionLimit` à 10 000 super-étapes : ADK compte
ces étapes sur toute la session persistée, y compris après les reprises. Cette
limite reste modifiable dans les réglages du graphe.

## Exécution et transport en direct

L'espace Exécution ouvre le chat par défaut, avec ses contrôles d'entrée et son rail de passages. L'inspecteur expose le graphe, les détails du parcours et l'état à la demande. Les composants AI Elements Vue représentent les réponses Markdown (`MessageResponse`) et les détails techniques (`ChainOfThoughtStep`, `Tool`). Le graphe conserve les étapes terminées et le zoom choisi lors des mises à jour. Chaque occurrence de nœud possède une identité distincte, ses entrées, sa sortie, sa durée et un état running, completed, waiting, resumed, interrupted ou error. Les résumés de raisonnement explicitement fournis par le modèle peuvent être affichés ; une signature opaque ne constitue pas un texte à afficher.

Le navigateur négocie un vrai `RTCPeerConnection` et un DataChannel ordonné `zedflow-events` avec le daemon Rust. Le canal transporte le bootstrap initial puis les deltas des entités modifiées, avec un curseur de rattrapage commun à SSE et HTTP. Les commandes, dont répondre à une attente, restent des requêtes HTTP. La connexion d'un client ne possède pas l'exécution et ne déconnecte pas les autres interfaces.

Le suivi affiche le transport utilisé : WebRTC, connexion en cours, SSE, rattrapage HTTP ou hors ligne. L’abonnement SSE commence immédiatement pendant la négociation ; le badge WebRTC n’apparaît qu’après réception d’un premier état valide. Un contrôle de réception détecte les canaux ouverts mais silencieux. Des requêtes HTTP bornées sur `/api/runs/{id}/snapshot` assurent aussi le rattrapage si un proxy bloque les événements SSE. Les révisions de l’état empêchent une réponse tardive de remplacer un état plus récent, indépendamment du curseur de pagination des événements. En cas d'échec de négociation ou de coupure du canal, le client reprend l'abonnement SSE depuis son curseur ; ce repli n'est pas présenté comme du WebRTC. La vue n'exige pas de rendre les tokens du modèle au fur et à mesure : le démarrage et la fin du nœud restent observables pendant que son résultat se prépare.

Les renderers d'outils sont déclarés dans la configuration du nœud : JSON, tableau,
code ou Markdown limité. La déclaration peut porter un titre et un langage. Le
dispatcher historique `execute_calls` traite un lot ; `execute_next_call` expose
un appel par passage du graphe. Les outils du workspace publient leurs appels,
résultats et aperçus de progression, dont la sortie `exec`, pendant l'exécution.
Aucun composant Vue ou JavaScript arbitraire n'est exécuté depuis cette déclaration.

Le mode local n'exige pas de serveur STUN/TURN. Un réseau distant peut nécessiter une configuration ICE du daemon :

- `ZEDFLOW_ICE_SERVERS` : tableau JSON d'objets `{ "urls": ["stun:…"] }` ou TURN avec `username` et `credential`.
- `ZEDFLOW_ICE_TRANSPORT_POLICY` : `all` par défaut, ou `relay` avec un serveur TURN.

Les paramètres ICE nécessaires sont transmis aux clients. Ils ne doivent pas contenir de credentials de fournisseur de modèle. L'accès distant complet, la découverte des daemons et l'authentification multi-machine restent hors de cette version ; un tunnel HTTP seul ne garantit pas la connectivité UDP de WebRTC.

## Connexion Codex

Installer le CLI Codex officiel sur la machine du daemon, puis y lancer :

```sh
codex -c cli_auth_credentials_store="file" login
```

Le compte doit disposer d'un accès Codex. Choisir ensuite Codex dans le nœud modèle et renseigner un identifiant de modèle disponible pour ce compte. Le panneau affiche l'état de connexion ; aucun token n'est stocké dans la composition ni renvoyé au navigateur.

L'adaptateur s'inspire du transport Codex utilisé par Pi. Il implémente `adk_core::Llm` au-dessus du backend d'abonnement Codex. Le CLI officiel gère la connexion et le rafraîchissement ; le daemon lit le fichier d'authentification sur la machine hôte. Ce transport est distinct de l'API publique OpenAI et peut évoluer ; un abonnement ne devient pas une clé d'API. Il ne lance pas un tour d'agent Codex avec sa propre boucle d'outils. Le texte et les appels sont reconstruits à partir des événements intermédiaires du fournisseur, y compris lorsque son événement terminal ne répète pas le contenu. Une génération ne contenant que des signatures ou du raisonnement, sans texte ni appel outil, produit une erreur explicite.

Les tests utilisent un faux CLI et un serveur local pour vérifier la conversion des requêtes, les réponses SSE, les appels d'outils, les résumés, l'usage et la non-exposition des credentials. Ils ne prouvent pas qu'un compte donné a accès à un modèle ; une connexion et une invocation live restent nécessaires pour vérifier cet accès.

Les branches internes ADK pouvant produire plusieurs attentes indépendantes sont
refusées à la validation. Les chemins conditionnels exclusifs et les branches
réunies par une jointure `all` avant une attente commune restent disponibles. Les
flows raccordés par une route `launch` ont leurs propres checkpoints et peuvent
continuer pendant que le parent attend une réponse ; leurs reprises restent liées
aux identités d’attente de chaque flow.

## Frontière de compilation

L'intégration conserve la séquence inspirée de Studio : validation, génération du projet Rust et compilation. Elle ne reprend pas sa dépendance ADK 1.x. La source Rust structurée conserve la définition et les métadonnées de présentation. Le parseur AST reconstruit une représentation de travail pour le canvas ; le compilateur construit les `StateGraph`, nœuds modèle, fonctions et sous-graphes ADK. Les opérations et politiques appliquées sont partagées avec le code exporté. ADK conserve l'ordonnancement, les mises à jour d'état et les checkpoints.

La sélection `fanIn: any` abaisse les connexions entrantes en routes ADK constantes ; cela évite que les deux retours alternatifs Outils et Attente deviennent une barrière attendant les deux. La valeur all conserve le comportement natif des jonctions. Ce choix est explicite : le compilateur ne devine pas qu'un cycle doit effacer une véritable jonction parallèle.

La compilation à la demande vérifie un projet Rust indépendant. Le lancement interactif construit le graphe dans le daemon avec le même abaissement pour y rattacher les checkpoints et l'observation ; il ne lance pas le binaire exporté. Les exports contiennent aussi les adaptateurs et leur manifeste, sans credentials. La CLI exportée sauvegarde les interruptions et accepte une reprise explicite dans un nouveau processus ; les options sont décrites dans son README généré. Depuis l’inspecteur, l’export suit la source du passage sélectionné.

## Limites explicites

- Natures intégrées : début, fin, affectation/template, condition binaire typée, contexte workspace historique, contrôle de réorientation, file/attente, invocation modèle, outils locaux/dispatcher, réponse, attente texte/confirmation et sous-graphe. Les prédicats v2 combinent égalité, différence, comparaison numérique, présence et inclusion avec ET/OU, sans coercition ni expression arbitraire. Le catalogue distingue encore les familles absentes, dont les nœuds d'action HTTP/base/email, le fan-in différé configurable, le cache et le time travel.
- Le modèle est volontairement une invocation bas niveau. Les mécanismes internes `LlmAgent` — transfert, guardrails, plugins, skills, toolsets et harness automatique — ne sont pas implicitement actifs.
- La limite de super-étapes est transmise à `ExecutionConfig` pour la racine et le binaire exporté. ADK 2.2.0 crée sa propre configuration de 50 étapes dans `SubgraphNode` : une limite enfant personnalisée est refusée. Les politiques de concurrence, timeout et retry du sous-graphe restent configurables.
- Le timeout exposé est la politique par défaut par nœud ; l'API publique `CompiledGraph` utilisée ne fournit pas de surcharge de timeout par nœud. Le journal des outils du workspace protège la reprise d'un appel identifié, sans garantir l'unicité d'effets externes entre différents appels.
- Les sous-graphes intégrés peuvent porter une attente ou un modèle à choisir ; le chemin du nœud et les checkpoints enfants sont conservés. La suspension indépendante de plusieurs branches internes n'est pas prise en charge. Les formulaires, fichiers et choix multiples restent à intégrer.
- L'état sauvegardé provient des checkpoints ADK ; les entrées/sorties et activités en cours sont des observations distinctes. Les attentes sauvegardées survivent au redémarrage. La reprise automatique d'une exécution active après crash reste à compléter.
- La compression et la préparation du contexte sont des stratégies et des flows explicites ; aucune compaction automatique universelle n’est ajoutée. Pas de fork historique, authentification mesh ni synchronisation multi-machine. Le daemon gère plusieurs workspaces de son hôte et écoute en loopback. Le workspace d’une session reste fixe ; ses routes peuvent appeler ou passer la main à des flows reliés. L’adoption de révisions conserve les frontières compatibles décrites dans le plan du contexte.
- Electron réutilise le web avec isolation de contexte, sandbox renderer et Node désactivé ; la distribution par installateurs reste à réaliser.

## Vérifications

```sh
pnpm --dir web test
pnpm --dir web typecheck
pnpm --dir web build
PLAYWRIGHT_SKIP_BROWSER_GC=1 pnpm --dir e2e exec playwright install chromium
ZEDFLOW_E2E_STATIC=1 pnpm --dir e2e test
(cd rust && CARGO_TARGET_DIR=/tmp/zedflow-adk-target cargo fmt --all --check)
(cd rust && CARGO_TARGET_DIR=/tmp/zedflow-adk-target cargo check --locked --workspace --all-targets)
(cd rust && CARGO_TARGET_DIR=/tmp/zedflow-adk-target ZEDFLOW_TEST_CODEGEN=1 cargo test --locked --workspace --all-targets)
(cd rust && CARGO_TARGET_DIR=/tmp/zedflow-adk-target cargo clippy --locked --workspace --all-targets -- -D warnings)
(cd rust && CARGO_TARGET_DIR=/tmp/zedflow-adk-target cargo check --locked --workspace --all-targets --all-features)
```

Le test navigateur démarre un daemon dédié sur le port3157 (et Vite5177 hors mode statique), avec données, home des flows et workspaces temporaires ; il ne réutilise pas le daemon personnel. `CHROMIUM_PATH` permet de choisir un Chromium existant. `ZEDFLOW_TEST_CODEGEN=1` sur les tests du daemon compile et exécute les projets exportés d'un sous-graphe et d'une boucle modèle/outils, puis compare leurs résultats aux exécutions du daemon. Ces tests sont opt-in car Cargo résout et compile des projets indépendants.

Les tests comportementaux couvrent aussi les paramètres transmis au modèle ADK, les reducers, les canaux stricts, les timeouts et retries, les surcharges par nœud, la concurrence, les jonctions all, les retours any, le parcours outil → modèle → attente puis reprise, et une réponse humaine insérée après un résultat d'outil. Les essais de transport vérifient un DataChannel Rust et une connexion navigateur réelle ; les essais Codex restent hors ligne.

Les tests du harness ajoutent des fichiers temporaires réels, des commandes Bash
avec code de sortie, timeout et annulation de descendants, des reçus commencés
ou terminés, des courses entre sélection et retrait de message, et les frontières
de chargement AGENTS/skills. Les scénarios API du harness utilisent des étapes
fixture déterministes pour exercer ces outils ; ils ne remplacent pas une
validation live de la qualité d'un modèle.

`ZEDFLOW_CLIENT_URL=http://127.0.0.1:3158 pnpm --dir web --filter @zedflow/desktop test:smoke` vérifie une fenêtre Electron sur un serveur web déjà lancé ; sur Linux sans affichage, utiliser `ZEDFLOW_CLIENT_URL=http://127.0.0.1:3158 xvfb-run -a pnpm --dir web --filter @zedflow/desktop test:smoke`. Son lancement de test emploie `--no-sandbox` pour le processus Chromium en environnement de test ; le client livré conserve la sandbox renderer activée.

## Documentation consultée

Documentation récupérée via Context7 avant intégration :

- Vue Flow : `/bcakmakoglu/vue-flow`, v-model des nœuds/arêtes, slots personnalisés et Handles ; source https://vueflow.dev/guide/.
- AI Elements Vue : `/vuepont/ai-elements-vue`, registre officiel Conversation, Message et PromptInput ; source https://github.com/vuepont/ai-elements-vue. Les composants du registre sont conservés dans le frontend, selon sa méthode d'installation.
- Reka UI : `/unovue/reka-ui`, Popover et Dialog contrôlés, placement et focus clavier ; source https://reka-ui.com/docs/components/popover.
- Electron : `/electron/electron`, BrowserWindow, isolation et restrictions de navigation ; source https://www.electronjs.org/docs/latest/tutorial/security.
- ADK-Rust : Context7 `/zavora-ai/adk-rust`, configuration `StateGraph`, `CompiledGraph`, modèles et outils ; les signatures sont vérifiées sur les sources des crates 2.2.0 installés, car certains résultats Context7 référencent des documents de conception plus anciens.
- Générateur ADK Studio 1.0.1 et sources ADK 2.2.0 installés : inspection directe de la validation, des fichiers générés, du StateGraph, des jonctions, politiques d'exécution et du mécanisme de reprise.

Le registre AI Elements Vue demandait une version bêta de vue-stream-markdown avec une sous-dépendance URL rejetée par pnpm. La résolution est fixée à la version stable 1.1.0 ; les composants installés passent le typage et le build avec cette version.
