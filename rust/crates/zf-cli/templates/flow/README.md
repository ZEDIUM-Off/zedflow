# {{id}}

Package de flow local, sans modèle ni effet externe. L’entrée `main` transmet
le champ `input`. Modifier `flow.rs` puis valider le package :

```sh
zf validate .zedflow/flow/{{id}}
zf compile .zedflow/flow/{{id}}
```

La création ne lance aucun flow et n’active aucun bridge. Pour exécuter après
sélection dans le catalogue du daemon, fournir explicitement sa clé et son hash :

```sh
zf run --flow-key CLE --flow-hash HASH --input '{"input":"Bonjour"}'
```

L’exécution autonome exige `--standalone --data DOSSIER_EXCLUSIF`.
Les fichiers secondaires doivent figurer dans `flow.json` ; ses dépendances
locales sont capturées avec le package lors de la compilation.
