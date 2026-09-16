//! Backend capability inventory. Available fields have an executable ADK mapping.
use serde_json::{Value, json};

pub fn catalog() -> Value {
    json!({
        "adkVersion":"2.2.0",
        "graph":{
            "settings":{
                "recursionLimit":{"type":"integer","default":100,"minimum":1,"maximum":10000,"label":"Étapes maximales","description":"Limite ADK des super-étapes ; une boucle reste visible dans le graphe."},
                "maxConcurrency":{"type":"integer","minimum":1,"maximum":128,"label":"Concurrence","description":"Nombre maximal de nœuds ADK exécutés simultanément."},
                "strictChannels":{"type":"boolean","default":false,"label":"Canaux stricts"},
                "timeoutMs":{"type":"integer","minimum":1,"maximum":3600000,"label":"Délai maximal par nœud (ms)"},
                "idleTimeoutMs":{"type":"integer","minimum":1,"maximum":3600000,"label":"Inactivité maximale (ms)","description":"Réinitialisée quand le nœud signale une progression ; une réponse modèle complète compte comme progression."},
                "retry":{"type":"object","label":"Reprises sur erreur","description":"Politique ADK par défaut ; peut réexécuter un effet externe. Une attente humaine n'est jamais retentée."}
            },
            "channelReducers":["overwrite","append","sum"],
            "builtInChannels":["__zedflow:context","hasSteering","hasFollowUp","input","output","response","messages","toolCalls","toolResults","hasToolCalls","modelResponse"],
            "retryFields":{"maxAttempts":{"default":1,"minimum":1,"maximum":20},"initialDelayMs":{"default":1000,"minimum":0,"maximum":600000},"maxDelayMs":{"default":60000,"minimum":0,"maximum":600000},"backoffFactor":{"default":2,"minimum":1,"maximum":16},"jitter":{"default":0,"minimum":0,"maximum":1},"retryOn":{"enum":["any","timeout"],"default":"any"}}
        },
        "nodes":[
            {"kind":"start","label":"Début","fields":[],"supported":true},
            {"kind":"end","label":"Fin","fields":[],"supported":true},
            {"kind":"context","label":"Préparer le contexte","fields":["modelNode","contextStrategy","contextBindings","contextCapabilities"],"supported":true,"description":"Évalue une stratégie explicite et conserve la fenêtre préparée pour le Modèle associé."},
            {"kind":"model","label":"Modèle","fields":["contextNode","modelBinding","provider","model","inputField","field","historyField","toolCallsField"],"supported":true,"description":"Consomme une préparation du nœud Contexte puis exécute une inférence."},
            {"kind":"agent","authorable":false,"label":"Agent historique","fields":["modelBinding","provider","model","reasoningEffort","instructions","globalInstructions","description","inputField","field","historyField","toolCallsField","tools","temperature","topP","topK","maxOutputTokens","stopSequences","responseFormat","responseSchema"],"supported":true,"description":"Lecture et exécution des anciennes définitions. Les nouveaux flows séparent Contexte et Modèle."},
            {"kind":"tool","label":"Outils","fields":["tool","arguments","inputField","field","historyField","toolCallsField","ui"],"supported":true,"description":"Appel FunctionTool autonome, ou exécution explicite des appels émis par un modèle."},
            {"kind":"input","label":"Attente","fields":["prompt","responseType","field"],"supported":true,"responseTypes":["text","confirmation"]},
            {"kind":"steering","label":"Steering","fields":[],"supported":true,"description":"Consomme une instruction à la frontière définie par le graphe, après le lot d’outils dans le harness."},
            {"kind":"inbox","label":"Prochain message","fields":["prompt"],"supported":true,"description":"Consomme steering ou follow-up, sinon attend via un checkpoint ADK."},
            {"kind":"route","label":"Point de routage","fields":["branch","invocation","routeId","inputField","field","fallback"],"supported":true,"description":"Emprunte une route de bridge ; une invocation conditionnelle sans route éligible suit la connexion locale."},
            {"kind":"await_route","label":"Attendre une visite","fields":["inputField","field"],"supported":true},
            {"kind":"set","label":"État","fields":["field","value"],"supported":true},
            {"kind":"output","label":"Réponse","fields":["text"],"supported":true},
            {"kind":"condition","label":"Condition","fields":["field","equals"],"supported":true},
            {"kind":"subgraph","label":"Sous-graphe","fields":["composition"],"supported":true,"description":"Composition embarquée isolée ; input vers input, response vers output."}
        ],
        "nodePolicies":["retry","fanIn"],
        "renderers":["json","table","code","markdown"],
        "tools":crate::operations::tool_declarations(),
        "toolDispatch":"execute_calls",
        "toolDispatchModes":["execute_calls","execute_next_call"],
        "harness":{"modelBindings":["fixed","runtime"],"queueKinds":["steering","followup"],"abort":true,"resume":true,"skills":"progressive","compaction":false,"filesystemScope":"os-user"},
        "fanIn":{"default":"all","enum":["all","any"],"description":"all conserve les jonctions ADK des arêtes directes ; any utilise des transitions ADK alternatives pour revenir au modèle depuis un outil ou une entrée."},
        "providers":[
            {"id":"fixture","label":"Démonstration locale","credentials":false,"supportsSampling":false,"description":"Fixture déterministe ; avec un outil déclaré, émet un vrai FunctionCall ADK puis lit sa FunctionResponse."},
            {"id":"gemini","label":"Gemini","credentials":"GOOGLE_API_KEY","supportsSampling":true,"supportsResponseSchema":true},
            {"id":"codex","label":"Abonnement Codex","credentials":"codex login","supportsSampling":false,"supportsResponseSchema":true,"fields":["model","reasoningEffort","reasoningSummary","textVerbosity"]}
        ],
        "unsupported":[
            {"id":"subgraphRecursionLimit","reason":"SubgraphNode 2.2.0 utilise sa propre ExecutionConfig avec 50 étapes ; les limites personnalisées sont réservées à la composition racine."},
            {"id":"nodeTimeoutOverride","reason":"Le builder CompiledGraph public 2.2.0 ne fournit pas de setter de timeout par nœud ; seule la politique par défaut est configurée."},
            {"id":"actions","reason":"Les backends ActionNode HTTP, base, email, code, RSS, notifications, fichiers ne sont pas encore exposés ; chaque backend nécessite ses dépendances et sa configuration."},
            {"id":"llmAgentPolicies","reason":"Les politiques internes LlmAgent (transfert, outils automatiques, guardrails, plugins, mémoire) ne sont pas appliquées à ce nœud modèle bas niveau."},
            {"id":"independentInterrupts","reason":"Les attentes simultanées de branches parallèles restent à qualifier. Les attentes séquentielles de sous-graphes sont prises en charge."},
            {"id":"deferredFanIn","reason":"Les nœuds deferred, politiques de fan-in et envois dynamiques ne sont pas encore configurables dans cet éditeur."},
            {"id":"cacheAndTimeTravel","reason":"Cache des nœuds, checkpoints delta, rétention et forks temporels ne sont pas exposés dans cette version."},
            {"id":"resourceBindings","reason":"Configuration de services mémoire, stores, artefacts et fournisseurs supplémentaires à venir."},
            {"id":"customRuntimeCode","reason":"Les renderers sont déclaratifs. Aucun JavaScript, composant Vue ou Rust arbitraire n'est exécuté depuis la configuration."}
        ]
    })
}
