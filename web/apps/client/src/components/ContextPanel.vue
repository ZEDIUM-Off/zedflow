<script setup lang="ts">
import type { RunContext } from '@zedflow/sdk'

import { X, FileText, BookOpen } from 'lucide-vue-next'

defineProps<{ context?: RunContext | null; snapshot?: boolean }>()
const emit = defineEmits<{ close: [] }>()
</script>
<template>
  <section class="harness-panel context-panel" aria-label="Contexte du workspace">
    <header><div><strong>Contexte</strong><small>{{ snapshot ? 'Contexte conservé pour cette exécution' : 'Instructions et skills du workspace' }}</small></div><button aria-label="Fermer le contexte" @click="emit('close')"><X :size="16"/></button></header>
    <h3><FileText :size="14"/> Instructions · {{ context?.instructions.length || 0 }}</h3>
    <details v-for="instruction in context?.instructions || []" :key="instruction.path"><summary>{{ instruction.path }}</summary><small v-if="instruction.hash">Empreinte {{ instruction.hash }}</small><pre>{{ instruction.content }}</pre></details>
    <p v-if="!context?.instructions.length" class="muted">Aucun fichier d’instructions chargé.</p>
    <h3><BookOpen :size="14"/> Skills disponibles · {{ context?.skills.length || 0 }}</h3>
    <div v-for="skill in context?.skills || []" :key="skill.path" class="context-skill"><strong>/skill:{{ skill.name }}</strong><span v-if="skill.manualOnly" class="context-label">Invocation explicite</span><p>{{ skill.description }}</p><small>{{ skill.path }}</small></div>
    <p v-if="!context?.skills.length" class="muted">Aucun skill découvert.</p>
    <h3>Skills chargés · {{ context?.loadedSkills?.length || 0 }}</h3>
    <div v-for="skill in context?.loadedSkills || []" :key="`${skill.path}:${skill.hash || ''}`" class="context-skill"><strong>{{ skill.name }}</strong><span v-if="skill.truncated" class="context-label">Chargement partiel</span><small>{{ skill.path }}</small><small v-if="skill.hash">Empreinte du contenu chargé {{ skill.hash }}</small><small v-if="skill.sourceHash && skill.sourceHash!==skill.hash">Empreinte du fichier source {{ skill.sourceHash }}</small></div>
    <p v-for="(diagnostic, index) in context?.diagnostics || []" :key="index" class="context-diagnostic">{{ diagnostic }}</p>
  </section>
</template>
