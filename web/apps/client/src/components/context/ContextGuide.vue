<script setup lang="ts">
import { ArrowRight, BookOpen, X } from 'lucide-vue-next'
import type { ContextExample } from '../../contextEngine'

defineProps<{ open: boolean; disabled?: boolean }>()
const emit = defineEmits<{ 'update:open': [open: boolean]; example: [example: ContextExample] }>()
</script>

<template>
  <section class="ctx-guide" aria-label="Guide de composition du contexte">
    <div class="ctx-guide-heading">
      <span class="ctx-guide-path">Sources <ArrowRight :size="12" aria-hidden="true"/> Programme <ArrowRight :size="12" aria-hidden="true"/> Contexte du modèle</span>
      <button :aria-expanded="open" aria-controls="context-composition-guide" @click="emit('update:open', !open)">
        <X v-if="open" :size="13"/><BookOpen v-else :size="13"/>
        {{open ? 'Masquer le guide' : 'Comprendre les blocs'}}
      </button>
    </div>
    <div v-if="open" id="context-composition-guide" class="ctx-guide-content">
      <ol>
        <li><strong>Choisir les types de sources</strong><p>Ouvrez <b>Ajouter des types</b>, puis cochez les données attendues. Chaque source possède un nom et des champs typés. Dans le flow, le nœud Contexte les relie aux données disponibles. Une déclaration n’injecte aucun contenu.</p></li>
        <li><strong>Composer avec les champs</strong><p>Glissez un champ sur un emplacement compatible, ou sélectionnez cet emplacement puis utilisez le bouton d’insertion du champ. Les couleurs suivent les sources. Un <b>fragment</b> ajoute du contenu, une <b>condition</b> choisit une branche et <b>Pour chaque</b> traite les éléments d’une liste.</p></li>
        <li><strong>Vérifier ce qui sera envoyé</strong><p>Choisissez un jeu de données d’essai et ouvrez <b>Voir les valeurs</b> pour l’adapter. L’aperçu suit vos modifications. Cliquez sur un résultat pour retrouver son bloc et son itération. Enregistrez ensuite la stratégie et reliez-la à un nœud <b>Contexte → Modèle</b>.</p></li>
      </ol>
      <div class="ctx-guide-examples">
        <span>Essayer un exemple éditable</span>
        <button :disabled="disabled" @click="emit('example', 'structured')">Conversation, outils et documents <ArrowRight :size="12"/></button>
        <button :disabled="disabled" @click="emit('example', 'instructions')">Instructions et demande <ArrowRight :size="12"/></button>
        <button :disabled="disabled" @click="emit('example', 'tool-result')">Résultat d’outil conditionnel <ArrowRight :size="12"/></button>
        <small>Chaque exemple ouvre un nouveau brouillon. Votre travail actuel est conservé.</small>
      </div>
    </div>
  </section>
</template>
