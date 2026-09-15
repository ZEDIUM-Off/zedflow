<script setup lang="ts">
import { computed, ref } from 'vue'
import { Activity, ChevronRight, Layers, Play } from 'lucide-vue-next'
import AppDialog from '../AppDialog.vue'
import ContextFixtures from './ContextFixtures.vue'
import ContextPreviewDocument from './ContextPreviewDocument.vue'
import ContextRequestPreview from './ContextRequestPreview.vue'
import { diagnosticBlock, type ContextDraft } from '../../contextEngine'
import type { ContextItem } from '@zedflow/sdk'

const props = defineProps<{ draft: ContextDraft; stale: boolean; selectedSource: string }>()
const emit = defineEmits<{ select: [id: string]; source: [name: string]; preview: [] }>()
const structure = ref(false), traceOpen = ref(false)
const evaluation = computed(() => props.draft.preview?.evaluation)
const countItems = (items: ContextItem[]): number => items.reduce((count, item) => count + (item.kind === 'fragment' ? 1 : countItems(item.items)), 0)
const count = computed(() => countItems(evaluation.value?.items || []))
const status = computed(() => props.draft.previewPending ? 'Calcul de l’aperçu…' : props.draft.error ? 'Aperçu à corriger' : props.stale ? 'Aperçu précédent' : !evaluation.value ? 'Aucun aperçu' : !evaluation.value.complete ? 'Données nécessaires' : 'Aperçu à jour')
</script>

<template>
  <div class="ctx-studio-preview">
    <header class="ctx-panel-heading"><div><h2>Aperçu</h2></div><ContextFixtures :draft="draft" compact/></header>
    <p class="ctx-preview-description">Aperçu calculé à partir des données d’essai fournies.</p>
    <nav class="ctx-preview-tabs" aria-label="Présentation de l’aperçu"><button :aria-pressed="!structure" @click="structure=false">Contexte</button><button :aria-pressed="structure" @click="structure=true">Structure</button></nav>
    <div class="ctx-preview-scroll" :aria-busy="!!draft.previewPending">
      <p v-if="stale" class="ctx-preview-stale">Le programme ou ses données ont changé. Ce résultat correspond au calcul précédent.</p>
      <ContextPreviewDocument v-if="evaluation" :items="evaluation.items" :requirements="draft.strategy.requirements" :trace="evaluation.trace" :selected="draft.selectedBlock" :occurrence="draft.selectedPreviewItem" :source="selectedSource" :structure="structure" @select="emit('select',$event)" @source="emit('source',$event)"/>
      <div v-if="!evaluation?.items.length&&!draft.error" class="ctx-preview-empty"><Layers :size="30"/><strong>{{ evaluation ? 'Aucun fragment émis' : 'Votre contexte apparaîtra ici' }}</strong><p>{{ evaluation ? 'Les branches retenues n’ajoutent aucun contenu pour ces données.' : 'Ajoutez des sources et composez un premier bloc. L’aperçu suivra vos modifications.' }}</p><button @click="emit('preview')"><Play :size="13"/>Calculer l’aperçu</button><small>Données d’essai uniquement. Aucun outil ou modèle exécuté.</small></div>
      <section v-if="evaluation?.needs.length" class="ctx-needs"><h3>Ressources nécessaires</h3><div v-for="need in evaluation.needs" :key="need.resource"><strong>{{ need.resource }}</strong><button v-for="id in need.requiredBy" :key="id" @click="emit('select',id)">Voir le bloc {{ id }}</button></div><small>Ces besoins sont signalés sans exécuter de producteur.</small></section>
      <details v-if="evaluation?.capabilities.length" class="ctx-preview-capabilities"><summary>{{ evaluation.capabilities.length }} capacités demandées</summary><p>{{ evaluation.capabilities.map(item=>item.id).join(', ') }}</p><small>Le flow vérifie les autorisations lorsqu’il utilise cette stratégie.</small></details>
      <section v-if="draft.diagnostics.length" class="ctx-diagnostics" aria-label="Diagnostics de stratégie"><h3>Diagnostics</h3><button v-for="(diagnostic,index) in draft.diagnostics" :key="index" @click="diagnosticBlock(draft.strategy,diagnostic.path)&&emit('select',diagnosticBlock(draft.strategy,diagnostic.path)!)"><strong>{{ diagnostic.message }}</strong><small>{{ diagnostic.path }} · {{ diagnostic.code }}</small></button></section>
      <ContextRequestPreview :draft="draft" :stale="stale"/>
    </div>
    <footer class="ctx-preview-footer"><span><Activity :size="14"/>{{ count }} fragment{{ count>1?'s':'' }} · {{ status }}</span><button :disabled="!evaluation?.trace?.length" @click="traceOpen=true">Voir la trace<ChevronRight :size="13"/></button></footer>
    <AppDialog v-model:open="traceOpen" title="Trace de composition du contexte" description="Blocs évalués, sources consultées et itérations pour cet aperçu." wide>
      <p v-if="stale" class="ctx-hint">Cette trace appartient à l’aperçu précédent.</p>
      <div class="ctx-evaluation-trace"><details v-for="(entry,index) in evaluation?.trace||[]" :key="`${entry.id}/${index}`"><summary><span>{{ index+1 }}</span><strong>{{ entry.blockId }}</strong><span v-if="entry.outcome!==undefined">{{ entry.outcome?'Vrai':'Faux' }}</span><small>{{ entry.sources.join(', ') || 'Sans source externe' }}</small></summary><dl><dt>Identité du résultat</dt><dd>{{ entry.id }}</dd><dt>Chemin</dt><dd>{{ entry.path }}</dd><template v-if="entry.iterations.length"><dt>Itérations</dt><dd>{{ entry.iterations.map(item=>`${item.blockId} · ${item.index+1}`).join(' / ') }}</dd></template></dl><button @click="emit('select',entry.id);traceOpen=false">Voir le bloc source</button></details></div>
    </AppDialog>
  </div>
</template>

<style scoped>
.ctx-studio-preview{display:flex;flex-direction:column;min-height:0;height:100%}.ctx-studio-preview>.ctx-panel-heading{align-items:center;margin:0 0 4px}.ctx-preview-description{font-size:11px;color:#9c9fa9;margin:4px 0 14px}.ctx-preview-tabs{padding:0;display:flex;gap:6px;border-bottom:1px solid #34373d;margin:0 0 16px;flex:none}.ctx-preview-tabs button{width:auto;flex:none;font-size:12px;border:0;border-radius:4px 4px 0 0;background:transparent;padding:9px 14px;color:#aeb2bb;border-bottom:2px solid transparent}.ctx-preview-tabs [aria-pressed=true]{background:#25282e;color:#e9ebef;border-bottom-color:#8fa9c6}.ctx-preview-scroll{overflow:auto;min-height:0;flex:1;padding:1px 1px 16px;scrollbar-width:thin;scrollbar-color:#484b54 transparent}.ctx-preview-empty{display:flex;align-items:center;text-align:center;flex-direction:column;gap:12px;margin:54px auto 24px;max-width:270px;color:#969ba7}.ctx-preview-empty strong{font-size:13px;color:#d3d6df;font-weight:500}.ctx-preview-empty p{font-size:12px;margin:0;line-height:1.6}.ctx-preview-empty button{display:flex;align-items:center;gap:6px;margin:5px 0}.ctx-preview-empty small{font-size:10px}.ctx-preview-stale{font-size:11px;color:#c7ad8e;border:1px solid #5b5041;padding:8px;border-radius:5px;margin:0 0 12px}.ctx-preview-capabilities{border-top:1px solid #363941;margin-top:16px;padding-top:12px;font-size:11px}.ctx-preview-capabilities summary{cursor:pointer}.ctx-preview-capabilities p{overflow-wrap:anywhere}.ctx-preview-footer{display:flex;align-items:center;justify-content:space-between;gap:10px;border-top:1px solid #34373d;min-height:42px;padding:8px 0 0;flex:none}.ctx-preview-footer>span{display:flex;align-items:center;gap:6px;font-size:10px;color:#b9bec9}.ctx-preview-footer button{display:flex;align-items:center;gap:4px;padding:2px 0;background:transparent;border:0;font-size:11px;color:#a8bed6}.ctx-preview-footer button:disabled{opacity:.4}.ctx-evaluation-trace{max-height:65vh;overflow:auto;font-size:12px}.ctx-evaluation-trace details{border-bottom:1px solid #3b3d45;padding:12px 0}.ctx-evaluation-trace summary{display:flex;gap:12px;cursor:pointer;flex-wrap:wrap}.ctx-evaluation-trace summary small{margin-left:auto}.ctx-evaluation-trace dl{display:grid;grid-template-columns:140px 1fr;gap:10px;margin:16px 0}.ctx-evaluation-trace dt{color:#9c9fab}.ctx-evaluation-trace dd{margin:0;overflow-wrap:anywhere;font-family:monospace}.ctx-studio-preview .ctx-diagnostics button{border-left:0;border:1px solid #6a4943;border-radius:5px}
</style>
