# ADK Studio pour le laboratoire

ADK Studio devient l'interface de composition visuelle du lab. Le serveur et
l'interface sont ceux du [projet upstream](https://github.com/zavora-ai/adk-studio),
sans fork ni adaptation HTML. La [documentation officielle](https://adk-rust.com/en/docs/studio/studio)
décrit son canvas, les propriétés des nœuds, la génération Rust et les traces.

## Installation et lancement

Version vérifiée : `adk-studio` **1.0.1**, publiée sur crates.io.
Installer dans le répertoire utilisateur dédié :

```sh
ADK_STUDIO_SKIP_UI_BUILD=1 CARGO_TARGET_DIR=/tmp/zedflow-studio-target \
  cargo install adk-studio --version 1.0.1 --locked --debug \
  --root "$HOME/.local/share/zedflow-adk-tools"
./scripts/studio.sh
```

`ADK_STUDIO_SKIP_UI_BUILD=1` utilise les assets officiels `ui/dist` embarqués dans
le paquet. Sans ce flag, son build script tente `npm ci` alors que le lockfile npm
n'est pas inclus dans le paquet publié. `--debug` accélère l'installation du lab.

Le build nécessite les outils C/C++, pkg-config et les en-têtes OpenSSL.
Sur le DGX, `libssl-dev` Ubuntu arm64 3.0.13-0ubuntu3.15 a été téléchargé avec
`apt-get download`, puis extrait avec `dpkg-deb -x` dans
`~/.local/share/zedflow-adk-tools/openssl`, sans installation système.
Pour réutiliser ces fichiers lors de l'installation Cargo :

```sh
export OPENSSL_LIB_DIR="$HOME/.local/share/zedflow-adk-tools/openssl/usr/lib/aarch64-linux-gnu"
export OPENSSL_INCLUDE_DIR="$HOME/.local/share/zedflow-adk-tools/openssl/usr/include"
export OPENSSL_STATIC=1
export CPATH="$HOME/.local/share/zedflow-adk-tools/openssl/usr/include/aarch64-linux-gnu${CPATH:+:$CPATH}"
```

Le launcher détecte cette installation OpenSSL pour les builds lancés depuis
Studio. Il résout le dépôt depuis son propre emplacement, utilise
`.adk-studio/projects/` et isole les fichiers temporaires dans `/tmp/zedflow-studio`.
`ADK_STUDIO_BIN`, `ZEDFLOW_ADK_TOOLS` et `ZEDFLOW_STUDIO_TMPDIR` permettent de
changer ces emplacements. À défaut du binaire dédié, il cherche dans `PATH`.

Par défaut, ouvrir <http://127.0.0.1:3000>. Pour l'accès depuis un autre appareil
autorisé sur Tailscale :

```sh
./scripts/studio.sh --host <IP_TAILSCALE_DGX> --port 3141
```

Ou conserver l'écoute locale et transmettre le port :
`ssh -N -L 3000:127.0.0.1:3000 dgx`, si cet alias SSH est configuré.

## Première expérience

Ouvrir **Zedflow Studio Recherche**. Le graphe contient :

1. **Demande** : déclenchement manuel, entrée utilisateur.
2. **Cadrer la recherche** : copie de l'entrée dans `query`.
3. **Sources de démonstration** : ajout de données locales dans `evidence`.
4. **Restituer les résultats** : construction de `response` à partir de cet état.

Cliquer sur les nœuds pour modifier leurs propriétés. Utiliser **Build**, fermer
le résultat de compilation, puis **Run** pour lancer le prompt du déclencheur
manuel. Ce projet composé uniquement d'actions utilise ce déclencheur ; le champ
chat standard demande la présence d'un agent. Le mode debug et la timeline sont activés
dans le projet. L'expérience ne contacte aucun modèle ou moteur de recherche.
L'autobuild est désactivé pour que chaque compilation soit explicite.
La connexion de session peut rester ouverte après l'événement `Done` ; utiliser
**Stop** pour la fermer. Le résultat déterministe se consulte dans **Events**
et dans l'état de sortie du nœud `reponse`.

Deux conventions de cette version sont prises en compte dans le projet : l'entrée
du runtime généré s'appelle `message` (donc `{{message}}` dans le premier Set), et
`START` doit être relié directement à `cadrer`. Le générateur saute les edges qui
ciblent un Trigger avant de résoudre l'entrée ; `START → demande → cadrer` produisait
un binaire compilable sans entrée de graphe. Le Trigger reste l'interface de lancement
manuel et pointe également vers `cadrer`. Ces réglages portent sur le projet natif,
pas sur une modification du générateur upstream.

Le endpoint d'exécution de Studio 1.0.1 exige aussi une configuration fournisseur,
même pour un graphe sans LLM. Le launcher définit donc `OLLAMA_HOST` vers l'adresse
locale conventionnelle `http://127.0.0.1:11434`, sauf valeur déjà fournie. Ce graphe
ne construit aucun modèle : aucun serveur Ollama, téléchargement ou appel réseau
n'est nécessaire. Un véritable agent Ollama demanderait un serveur et un modèle.

## Définition et limites

Les fichiers `.adk-studio/projects/*.json` sont les sources des expériences Studio.
Les modifications enregistrées dans le canvas sont donc visibles dans le diff Git
de Zed. La génération Rust se fait dans le répertoire temporaire de Studio ; ses
fichiers peuvent être régénérés et ne sont pas une deuxième définition à maintenir.

Les `flows/*.rs` restent des expériences Rust distinctes. Le projet de recherche
Studio reprend un principe pédagogique, sans être un import ni une copie exacte
de `flows/research.rs`. Aucun import général Rust → Studio n'a été trouvé dans la
version installée. Les expériences mémoire, sous-graphe et checkpoint Rust ne sont
donc pas présentées comme migrées dans le canvas.

Le manifeste et le générateur du paquet 1.0.1 ciblent **ADK 1.0.0** (dépendance
compatible 1.x), même si la documentation web montre aussi des exemples ADK 2.x.
Le générateur fixe cette version ; modifier `settings.adkVersion` ne suffit pas à
le passer en 2.2.0. Le workspace Rust du lab reste fixé en 2.2.0 et possède son propre
lockfile. Une convergence de versions demandera une validation séparée.

L'inspecteur d'état de Studio reconstruit certains instantanés de nœuds à partir
des sorties attendues et de l'état final. Ces vues ne doivent pas être assimilées
à des checkpoints indépendants de chaque nœud ni à une preuve de reprise durable.

Le prototype `web/` et sa feature Cargo sont conservés comme expérience historique ;
ils ne constituent plus l'interface recommandée. Le service `zedflow-lab-web` est
arrêté au profit de `zedflow-adk-studio`.

## Service sur le DGX

Le service utilisateur transitoire `zedflow-adk-studio.service` lance le script
avec le bind Tailscale et le port 3141. Il n'est pas activé au démarrage de l'appareil.
Il conserve les projets JSON dans le dépôt et les builds dans le répertoire temporaire.

```sh
systemctl --user status zedflow-adk-studio.service
journalctl --user -u zedflow-adk-studio.service -n 50
systemctl --user restart zedflow-adk-studio.service
systemctl --user stop zedflow-adk-studio.service
```

Validation locale du 10 septembre 2026 : installation du binaire, génération et
compilation depuis le bouton Build, exécution depuis Run, trois étapes réussies
dans la timeline, réponse contenant la question et les sources fixture. Aucun
appel à un modèle n'a été effectué. Voir [validation.md](validation.md).
