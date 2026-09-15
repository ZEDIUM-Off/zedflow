# @zedflow/sdk

SDK Zedflow en modules ES, indépendant de Vue, d’Electron et de l’application web.
Les schémas Zod définissent les types TypeScript et valident les frontières du
protocole. Le transport est injecté par client ; aucun état de connexion global.

Ce socle expose le transport et les primitives JSON. Les clients de domaines
(flows, composition, contexte, runs, sessions…) sont ajoutés par les unités
suivantes de la migration ; le frontend historique n’est pas encore migré.

```ts
import { createClient, jsonObjectSchema } from '@zedflow/sdk';

const client = createClient({
  baseUrl: 'http://127.0.0.1:3142/api',
  protocol: 1,
  // fetch: une implémentation injectable pour les tests ou un autre hôte
});

const health = await client.transport.json(
  { operation: 'daemon.health', method: 'GET', path: 'health' },
  jsonObjectSchema,
);
```

La base est une URL HTTP absolue incluant le préfixe API. Les chemins sont
relatifs à cette base ; les segments dynamiques doivent être encodés et les
paramètres passés par `query`. Le protocole configuré accompagne les lectures
comme les commandes. `signal` adresse l’annulation d’une requête particulière.

`transport.json` valide sa réponse avec le schéma fourni et ne remplace jamais
un JSON invalide par `null`. `jsonValueSchema` et `jsonObjectSchema` préservent
les canaux et valeurs JSON ouverts. Une entrée qui n’est pas du JSON, notamment
un nombre non fini ou un champ `undefined`, est refusée avant envoi.

`transport.bytes` restitue un `Uint8Array`, sans décodage texte ni reconstruction
JSON : utiliser cette voie pour le raw d’inférence et les archives. Une réponse
vide doit être consommée par cette voie plutôt que par un faux contrat JSON.

Les erreurs portent `operation` et `kind` : réseau, HTTP, JSON, validation de
requête, validation de réponse ou annulation. `HttpError` conserve le statut,
le corps original textuel et le corps JSON lorsqu’il est disponible, y compris
les diagnostics du daemon. `RequestAbortedError.cause` conserve le motif
d’annulation. Les captures raw historiques manquantes doivent être traitées par
leur client de domaine, sans inventer une requête.

Un hôte peut fournir `{ transport }` à `createClient` pour remplacer HTTP. Ce
transport doit respecter les mêmes contrats de validation, d’octets et
d’annulation. L’instanciation du client n’effectue aucune requête.

Depuis le dépôt candidat :

```sh
pnpm --dir web --filter @zedflow/sdk typecheck
pnpm --dir web --filter @zedflow/sdk build
pnpm --dir web --filter @zedflow/sdk test
```

Les tests utilisent le paquet construit, des réponses fixtures et un consommateur
isolé Node/TypeScript/navigateur. Ils ne contactent aucun daemon personnel. La
version Zod est verrouillée ; ce package n’est pas publié sur un registre lors
de cette migration.
