<script setup lang="ts">
import { computed } from 'vue'
import type { ContextDraft } from '../../contextEngine'

const props=defineProps<{draft:ContextDraft;stale:boolean}>()
const profile=computed(()=>props.draft.previewProfile)
const result=computed(()=>props.draft.preview?.request)
const labels:Record<string,string>={codexHttpBody:'Corps JSON de l’adaptateur Codex',adkRequest:'Requête ADK · HTTP Gemini non exposé',fixtureInput:'Entrée du modèle fixture'}
function enable(){props.draft.previewProfile={provider:'',model:'',config:'{}',tools:'{}',media:'{}'}}
function download(){if(result.value?.raw===undefined)return;const url=URL.createObjectURL(new Blob([result.value.raw],{type:'application/json;charset=utf-8'}));const anchor=document.createElement('a');anchor.href=url;anchor.download='requete-essai.json';anchor.click();setTimeout(()=>URL.revokeObjectURL(url),1000)}
</script>
<template><section class="request-trial" aria-label="Profil de requête d’essai">
  <header><strong>Requête complète</strong><button v-if="!profile" @click="enable">Configurer l’aperçu</button><button v-else @click="draft.previewProfile=undefined">Retirer le profil</button></header>
  <p>Ce profil sert uniquement aux essais. Il ne modifie ni la stratégie, ni les modèles et outils de ses futurs flows.</p>
  <template v-if="profile"><details open><summary>Modèle, paramètres et déclarations</summary>
    <label>Frontière d’aperçu
      <select v-model="profile.provider" aria-label="Frontière d’aperçu">
        <option value="" disabled>Choisir une frontière</option>
        <option value="codex">Codex · corps HTTP</option>
        <option value="gemini">Gemini · requête ADK</option>
        <option value="fixture">Fixture · entrée reçue</option>
      </select>
    </label>
    <label>Identifiant du modèle d’essai<input v-model="profile.model" placeholder="Identifiant exact du modèle"/></label>
    <label>Paramètres ADK · JSON<textarea v-model="profile.config" rows="3" spellcheck="false"/></label>
    <details v-if="profile.provider==='codex'||profile.reasoningEffort||profile.reasoningSummary||profile.textVerbosity"><summary>Options Codex</summary><label>Effort de réflexion<input v-model="profile.reasoningEffort"/></label><label>Résumé de réflexion<input v-model="profile.reasoningSummary"/></label><label>Verbosité<input v-model="profile.textVerbosity"/></label></details>
    <label>Déclarations d’outils · JSON<textarea v-model="profile.tools" rows="4" spellcheck="false" placeholder='{"read":{"description":"Lire un fichier","parameters":{"type":"object","properties":{}}}}'/></label>
    <small>Une entrée par capacité sélectionnée, avec sa description et son schéma <code>parameters</code>. Aucune autorisation d’exécution n’est accordée ici.</small>
    <details><summary>Octets des médias d’essai</summary><label>Contenus par référence · JSON<textarea v-model="profile.media" rows="4" spellcheck="false"/></label><small>Chaque référence contient un objet <code>{"encoding":"base64","byteLength":3,"chunks":["YWJj"]}</code>. Aucun fichier ni URL n’est lu automatiquement.</small></details>
  </details>
  <p v-if="stale">Le profil ou les données ont changé. Le raw affiché reste celui de l’aperçu précédent.</p>
  <div v-for="diagnostic in result?.diagnostics||[]" :key="diagnostic.path+diagnostic.message" role="alert">{{diagnostic.message}}</div>
  <details v-if="result?.raw!==undefined"><summary>Raw · requête préparée, jamais envoyée</summary><p>{{labels[result.boundary||'']}} · {{result.byteLength}} octets</p><button @click="download">Télécharger les octets préparés</button><pre>{{result.raw}}</pre><small>SHA-256 : {{result.sha256}}</small></details>
  <p v-else>Aperçu d’essai : la requête complète sera disponible après validation du profil et des données.</p>
  </template>
</section></template>
<style scoped>.request-trial{border-top:1px solid #363941;padding-top:12px;margin-top:16px;font-size:11px}.request-trial header{display:flex;gap:8px;justify-content:space-between;align-items:center}.request-trial p,.request-trial small{color:#a7abb5;line-height:1.5}.request-trial label{display:flex;flex-direction:column;gap:6px;margin:12px 0}.request-trial textarea{width:100%;resize:vertical;font-family:monospace}.request-trial details{margin:12px 0}.request-trial pre{white-space:pre-wrap;overflow-wrap:anywhere;max-height:420px;overflow:auto}.request-trial [role=alert]{color:#e6b19f;padding:8px;border:1px solid #6a4943;margin:8px 0}.request-trial small{overflow-wrap:anywhere}</style>
