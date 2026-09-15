<script setup lang="ts">
import { computed } from 'vue'
import { usePromptInput } from './ai-elements/prompt-input/context'
import type { SkillEntry } from '../harness'
const props = defineProps<{ skills: SkillEntry[];loading?:boolean;error?:string }>()
const emit = defineEmits<{retry:[]}>()
const { textInput, setTextInput } = usePromptInput()
const match = computed(() => textInput.value.match(/^\/skill:([^\s]*)$/))
const suggestions = computed(() => match.value ? props.skills.filter(skill => skill.name.toLowerCase().includes(match.value![1]!.toLowerCase())).slice(0, 8) : [])
function insert(name: string) { setTextInput(`/skill:${name} `) }
</script>
<template>
  <div v-if="match" class="skill-suggestions" aria-label="Suggestions de skills"><button v-for="skill in suggestions" :key="skill.path" type="button" @click="insert(skill.name)"><strong>/skill:{{ skill.name }}</strong><span>{{ skill.description }}</span></button><p v-if="loading" class="muted" role="status">Chargement du catalogue…</p><p v-else-if="error" class="detail-error">{{error}} <button type="button" @click="emit('retry')">Réessayer</button></p><p v-else-if="!suggestions.length" class="muted">Aucun skill correspondant.</p></div>
</template>
