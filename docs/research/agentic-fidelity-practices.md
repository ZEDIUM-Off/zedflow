# Research: pratiques agentiques pour la fidélité du port Pi ↔ Zedflow

## Summary
Le programme de port doit employer Wayfinder uniquement pour les décisions de fidélité encore indéterminées, jamais pour livrer une implémentation Rust. Les décisions de portée, d’équivalence sémantique et de promotion doivent être HITL; les faits vérifiables, les inventaires et les comparaisons de références peuvent être AFK, mais la preuve de fidélité exige des commandes rejouables, une validation manuelle ciblée et une revue indépendante du même SHA.

## Findings
1. **[Sourcé] Les décisions qui changent la sémantique ou la portée sont HITL.** Wayfinder distingue HITL et AFK; un ticket HITL ne se résout que par un échange humain réel, sans que l’agent réponde à la place de l’humain. La ligne de base Zedflow rend explicitement HITL la promotion du SHA produit et réserve l’arbitrage humain aux substitutions de dépendances. Donc les divergences Pi→Rust, les dispositions non bijectives, les exemptions de tests et toute promotion doivent rester bloquées sur une décision humaine enregistrée. [Wayfinder SKILL](https://github.com/mattpocock/skills/blob/main/skills/engineering/wayfinder/SKILL.md) · [Zedflow baseline](../../porting/BASELINE.md) · [AGENTS](../../AGENTS.md)

2. **[Sourcé] Utiliser le fog/frontier pour les inconnues, non comme un backlog d’implémentation.** Une question devient ticket lorsqu’elle est formulable précisément, même bloquée; sinon elle reste dans « Not yet specified ». Le frontier est l’ensemble des tickets ouverts, non bloqués et non réclamés; les dépendances natives le rendent visible. Le skill stipule aussi qu’un ticket Wayfinder résout une décision, non une tranche de construction. [Wayfinder SKILL](https://github.com/mattpocock/skills/blob/main/skills/engineering/wayfinder/SKILL.md) · [GitHub tracker contract](https://raw.githubusercontent.com/mattpocock/skills/main/skills/engineering/setup-matt-pocock-skills/issue-tracker-github.md)

3. **[Recommandation] Pour ce port, un ticket de fidélité doit poser une question testable et lier le couple source Pi / preuve Rust.** Exemples adaptés: « Quelle est l’observation externe de Pi pour cette annulation? » ou « Cette API sans équivalent accepte-t-elle la disposition X? ». Les tickets « porter X » doivent aller au flux d’exécution après décision. Chaque ticket AFK doit citer le chemin sous `references/pi/`, le crate Rust cible, le SHA Pi gelé, la commande de comparaison et le résultat attendu; chaque ticket HITL doit énoncer les options, le risque de divergence et le décideur. Cela applique le principe sourced “question, pas build” au contrat Stage 1 one-to-one du dépôt. [AGENTS](../../AGENTS.md)

4. **[Sourcé] Le handoff d’issue est un protocole d’audit minimal.** La convention GitHub du skill est: réclamer d’abord par assignation, commenter la résolution, fermer, puis ajouter au map un pointeur de contexte; le map est un index et ne duplique pas la décision. Le contrat de tracker emploie les dépendances GitHub natives et, à défaut, `Blocked by:` dans le corps. [Wayfinder SKILL](https://github.com/mattpocock/skills/blob/main/skills/engineering/wayfinder/SKILL.md) · [GitHub tracker contract](https://raw.githubusercontent.com/mattpocock/skills/main/skills/engineering/setup-matt-pocock-skills/issue-tracker-github.md)

5. **[Recommandation] La résolution d’un ticket de fidélité doit contenir une preuve exploitable, pas seulement une conclusion.** Publier: SHA Rust et gitlink Pi, chemins exacts, commande(s) exécutée(s), sortie/résultat, scénario manuel si nécessaire, divergence admise et approbateur si HITL. Cette recommandation est cohérente avec le gate Zedflow: aucune source/test Pi gelé sans implémentation/test sémantique atteignable ou disposition approuvée; les marqueurs et tests vides ne comptent pas. [Zedflow baseline](../../porting/BASELINE.md)

6. **[Sourcé] Une revue indépendante doit séparer conformité au standard et fidélité au spécification/référence.** `code-review` fait deux examens parallèles, Standards et Spec, contre un point fixe, et ne fusionne ni ne re-classe leurs résultats afin qu’une conformité de style ne masque pas une implémentation incorrecte. Pour Zedflow, l’axe Spec doit prendre le comportement/test Pi gelé comme oracle, et l’axe Standards doit prendre `AGENTS.md` et les règles Rust du dépôt. [Code-review SKILL](https://github.com/mattpocock/skills/blob/main/skills/engineering/code-review/SKILL.md) · [AGENTS](../../AGENTS.md)

7. **[Recommandation — sévérité haute] Ajouter une revue de fidélité indépendante pour tout changement de comportement observé.** Un relecteur frais, en lecture seule, doit comparer le diff au Pi gelé et exécuter (ou inspecter) les preuves, sans utiliser le compte rendu de l’implémenteur comme autorité. Signaler `haute` quand une surface Pi observable est absente, modifiée ou non prouvée; `moyenne` quand la preuve ne couvre pas les erreurs/annulation/persistance; `basse` pour un écart de traçabilité sans divergence observée. La baseline exige déjà des revues indépendantes end-user, fidelity et Rust-quality sur le même SHA: l’extension proposée est de rendre leurs attestations par surface et commande explicites. [Zedflow baseline](../../porting/BASELINE.md)

8. **[Sourcé] Les tests automatisés ne remplacent pas la validation manuelle de surfaces interactives.** La baseline impose les tests workspace, le manifeste sémantique strict et des parcours end-to-end (TUI par défaut, sorties print/text/json, RPC, sessions, outils, extensions, thèmes, gestion de paquets et Orchestrator); elle exige aussi la comparaison Pi des extensions. Ces surfaces constituent la liste minimale des validations manuelles lorsque l’assertion automatique ne démontre pas l’observation utilisateur ou le comportement terminal. [Zedflow baseline](../../porting/BASELINE.md)

## Application minimale proposée

| Étape | Sourced practice | Adaptation recommandée au port |
| --- | --- | --- |
| Décider | HITL ne peut être auto-résolu | HITL pour divergence sémantique, disposition, dépendance et promotion; conserver décision + approbateur dans l’issue. |
| Cartographier | Fog = question imprécise; frontier = ouverte/non bloquée/non réclamée | N’émettre que les questions de fidélité précisément formulées; laisser les inconnues dans le fog; ne pas créer de tickets « implémenter ». |
| Exécuter | AFK recherche; issue claim/comment/close | Une unité de port lie référence Pi, crate Rust, test/exécution et SHA; l’exécuteur ne modifie pas la carte. |
| Prouver | Review Standards/Spec séparées | Produire un tableau observation Pi → preuve Rust, avec commande et résultat; un échec ou une absence de preuve bloque. |
| Valider | Gate final et revues indépendantes | Rejouer les commandes sur le même SHA immuable; faire les parcours manuels indiqués par la baseline, documenter environnement et résultats. |

## Sources
- Kept: [Wayfinder SKILL](https://github.com/mattpocock/skills/blob/main/skills/engineering/wayfinder/SKILL.md) — source primaire des règles HITL/AFK, fog/frontier et fermeture.
- Kept: [GitHub tracker contract](https://raw.githubusercontent.com/mattpocock/skills/main/skills/engineering/setup-matt-pocock-skills/issue-tracker-github.md) — protocole GitHub de claim, dépendances, résolution et pointeur.
- Kept: [Code-review SKILL](https://github.com/mattpocock/skills/blob/main/skills/engineering/code-review/SKILL.md) — indépendance par axes Standards/Spec et point de comparaison fixe.
- Kept: [`AGENTS.md`](../../AGENTS.md) — contrat local one-to-one, contrôleur, arbitrag​​e humain et règles Stage 1.
- Kept: [`docs/porting/BASELINE.md`](../../porting/BASELINE.md) — gate local: SHA unique, revues indépendantes, couverture sémantique et parcours end-to-end.
- Dropped: résultats de blogs/agrégateurs sur les pratiques agentiques — non primaires ou sans autorité sur ce dépôt.

## Gaps
- Le worktree examiné ne contenait pas `docs/agents/issue-tracker.md`; le protocole GitHub ci-dessus provient donc du contrat primaire installé/public de mattpocock, à vérifier contre toute configuration locale ultérieure.
- Cette recherche ne vérifie pas l’état, les commentaires, les permissions GitHub ni la fermeture de l’issue #8; ces opérations nécessitent un contexte d’exécution `gh` autorisé.
- Les commandes exactes de validation par unité restent dans les enregistrements du contrôleur et les tests concernés; cette note établit le format de preuve, pas un nouveau gate.
