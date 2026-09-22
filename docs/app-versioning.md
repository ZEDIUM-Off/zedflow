# Versions et mises à jour de l’application

Le footer ouvre **Versions et mises à jour**. Ce panneau distingue le client
chargé dans l’onglet, le client servi par le daemon et le binaire du daemon.
Une vérification est effectuée toutes les quinze secondes lorsque l’onglet est
visible, ainsi qu’au retour du focus. Une réponse absente, ancienne ou invalide
ne permet pas d’affirmer que l’application est à jour.

## Identités et compatibilité

`version.json` porte la version produit, le protocole client/daemon et l’époque
compatible du stockage. Les formats de flows, bridges, stratégies et archives
conservent leurs propres versions. Le numéro du crate Cargo n’est pas utilisé
comme preuve qu’un client possède le dernier code de développement.

Les builds du client et du daemon ont chacun une empreinte de leurs sources et
entrées de construction. Le daemon inclut également sa cible, son profil, ses
features et la version du compilateur. Les modifications locales sont prises en
compte : le commit Git n’est qu’une provenance supplémentaire. `--build-info`
affiche l’identité d’un binaire sans ouvrir les données du daemon.

Une release contient le binaire et les assets web, leurs identités et un
inventaire SHA-256 des fichiers. Son identité dépend de ce contenu, pas de sa
date de préparation. Préparer à nouveau les mêmes artefacts ne crée pas une
nouvelle release. Le superviseur refuse les fichiers altérés, les liens
symboliques et une plateforme incompatible avant de lancer le binaire.

Les clients envoient leur protocole avec leurs commandes. Une incompatibilité
explicite est refusée côté serveur ; les clients historiques sans cet en-tête
restent acceptés. Le panneau permet leur actualisation lorsque l’identité est
connue. Il n’y a pas d’actualisation automatique d’un onglet.

## Canal local et publication

Le canal actuellement implémenté est **local**, sur la machine du daemon. Aucun
registre distant, téléchargement automatique ou mise à jour Git n’est configuré.
Seules les releases explicitement préparées sont annoncées disponibles.

Depuis le dépôt, avec Node 24, pnpm et la toolchain Rust du projet :

```sh
REPO="$(pwd -P)"
RELEASE_ROOT="$(mktemp -d /tmp/zedflow-releases.XXXXXX)"
CARGO_TARGET_DIR=/tmp/zedflow-adk-target pnpm --dir tooling/releases release:prepare --root "$RELEASE_ROOT"
```

Cette commande construit les deux composants, vérifie que leurs sources n’ont
pas changé pendant la préparation, vérifie l’inventaire et publie un candidat
sous le `--root` absolu choisi. Elle n’active pas ce candidat. `pnpm --dir web build` reste
une commande de compilation du client ; elle ne publie pas une release.

Le premier démarrage supervisé est explicite :

```sh
pnpm --dir tooling/releases run --root "$RELEASE_ROOT" \
  --workspace /chemin/absolu/du/workspace --data /chemin/absolu/des/donnees \
  --listen 127.0.0.1:3142
```

Il démarre la release déjà confirmée, ou le candidat préparé lors de la première
installation. Un seul superviseur peut posséder ce répertoire. Un daemon
classique utilisant le même dossier de données doit être arrêté auparavant,
à un moment sans exécution active. Le verrou de données du daemon reste utilisé.

`pnpm --dir` change le répertoire effectif : `--root`, `--workspace` et `--data`
doivent être absolus. Ne pas déduire leurs chemins du cwd de `tooling/releases`.
Le démarrage supervisé ci-dessus est une opération explicite, pas une gate CI.
Les procédures de maintenance personnelles restent dans les fiches canoniques
multi-appareils ; elles ne sont ni copiées ni activées par cette migration.
Le smoke automatisé ne qualifie que la plateforme où il a effectivement tourné.

## Activation et retour arrière

**Activer la mise à jour daemon + client** choisit la paire contenue dans la
release. Le serveur contrôle sous une barrière commune qu’aucune commande
concurrente ne démarre une exécution. Il refuse l’activation s’il reste une
exécution active, y compris dans un autre workspace ou en arrière-plan.
Une attente humaine durable ne provoque pas, à elle seule, un refus.

La demande cible une release et le build actuel attendu, jamais un chemin ou
une commande fournis par le navigateur. Elle est enregistrée durablement avant
l’arrêt. Les nouvelles mutations sont ensuite refusées pendant l’activation.
Les sessions ne sont ni reprises ni interrompues implicitement.

Le superviseur revalide les fichiers, attend l’arrêt du processus précédent,
lance le candidat et vérifie son identité par `/api/version`. Il ne confirme la
release qu’après cette vérification. Un échec de démarrage relance la release
précédente. Une interruption de l’activation conserve la dernière release
confirmée comme point de reprise. Une différence d’époque de stockage exige
une maintenance hors ligne et ne passe pas par ce parcours de retour arrière.

Le bouton **Revenir à la release précédente** utilise les mêmes contrôles.
Le résultat et les erreurs sont visibles dans le panneau. Une défaillance du
superviseur rend l’activation indisponible ; sa présence est suivie par un bail
court, distinct de la santé du daemon.

## Recharger le client

Après activation, les autres onglets détectent leur éventuel retard. Le bouton
**Actualiser le client et conserver les brouillons** sauvegarde dans le stockage
de session du navigateur les brouillons de conversation, de flow, de stratégie,
de bridge, de types et de bibliothèques. Il conserve leurs identités d’attente
et leurs références de fichier : il n’enregistre rien sur le serveur à leur
place et ne contourne pas les conflits de source. Un échec de sauvegarde locale
annule le rechargement. Cette récupération est consommée au remontage des
composants pour ne pas réintroduire un ancien message lors d’un rechargement
ultérieur.

Les pages HTML et réponses de version sont servies avec `Cache-Control: no-store`.
Les assets sont adressés sous `/_client/<build-id>/assets/` et mis en cache comme
contenus immuables. Les anciens builds restent disponibles aux onglets ouverts.
Il n’y a pas de suppression automatique des anciennes releases ou assets ;
leur conservation consomme de l’espace disque dans le répertoire de mises à jour.

## Validation

Les tests du superviseur utilisent des exécutables fixtures et des répertoires
temporaires. Ils couvrent plusieurs activations, un candidat qui échoue au
démarrage, une altération des fichiers et une activation interrompue.
Les tests API couvrent les exécutions actives, les demandes périmées, le bail du
superviseur, l’origine navigateur, le protocole et le blocage des mutations.

```sh
node --test tooling/releases/tests/supervisor.test.mjs
(cd rust && CARGO_TARGET_DIR=/tmp/zedflow-adk-target cargo test --locked -p zf-serve --test app_updates)
pnpm --dir web build
ZEDFLOW_E2E_STATIC=1 pnpm --dir e2e test specs/app-versions.spec.ts
```

Le mode `ZEDFLOW_E2E_STATIC=1` utilise le client compilé avec un daemon E2E dédié,
sans Vite ni surveillance des fichiers. Il vérifie donc les assets et le parcours
de rechargement distribués aux navigateurs, sans contacter de modèle réel.
