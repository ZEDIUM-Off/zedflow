<script setup lang="ts">
import { computed } from 'vue'
import { CodeBlock } from './ai-elements/code-block'
const props=defineProps<{value:unknown;renderer?:string;language?:string;tool?:string}>()
const fileResult=computed(()=>props.value && typeof props.value==='object' && !Array.isArray(props.value)?props.value as Record<string,any>:null)
const workspaceResult=computed(()=>['read','write','edit','exec'].includes(props.tool || '') || typeof fileResult.value?.diff==='string')
const content=computed(()=>fileResult.value?.output ?? fileResult.value?.content ?? fileResult.value?.text ?? fileResult.value?.message)
const diffLines=computed(()=>typeof fileResult.value?.diff==='string'?fileResult.value.diff.split('\n'):[])
const text=computed(()=>typeof props.value==='string'?props.value:JSON.stringify(props.value,null,2)??'—')
const rows=computed(()=>Array.isArray(props.value)?props.value:props.value && typeof props.value==='object'?Object.entries(props.value).map(([key,value])=>({champ:key,valeur:value})):[])
const columns=computed(()=>[...new Set(rows.value.flatMap(r=>r && typeof r==='object'?Object.keys(r):['valeur']))])
const language=computed(()=>['json','typescript','javascript','rust','python','bash','text'].includes(props.language||'')?props.language as 'json':'json')
function cell(row:any,key:string){const value=row && typeof row==='object'?row[key]:row;return typeof value==='string'?value:JSON.stringify(value)??'—'}
// Deliberately narrow Markdown: headings, list markers and emphasis, never arbitrary HTML.
const lines=computed(()=>text.value.split('\n').map(line=>({heading:line.startsWith('#'),bullet:/^[-*] /.test(line),parts:line.replace(/^#{1,6}\s/,'').replace(/^[-*] /,'').split(/(\*\*[^*]+\*\*)/g)})))
</script>
<template>
  <div v-if="workspaceResult && fileResult" class="workspace-tool-result"><div class="tool-result-meta"><code v-if="fileResult.path">{{fileResult.path}}</code><span v-if="fileResult.exitCode !== undefined || fileResult.exit_code !== undefined">Code de sortie {{fileResult.exitCode ?? fileResult.exit_code}}</span><span v-if="fileResult.truncated">Aperçu tronqué</span></div><pre v-if="diffLines.length" class="tool-diff"><span v-for="(line,index) in diffLines" :key="index" :class="{added:line.startsWith('+'),removed:line.startsWith('-')}">{{line + '\n'}}</span></pre><pre v-else-if="content !== undefined">{{typeof content==='string'?content:JSON.stringify(content,null,2)}}</pre><pre v-else>{{text}}</pre><p v-if="fileResult.fullOutputPath || fileResult.outputPath || fileResult.output_path" class="muted">Sortie complète : <code>{{fileResult.fullOutputPath || fileResult.outputPath || fileResult.output_path}}</code></p></div>
  <div v-else-if="renderer==='table' && rows.length" class="result-table"><table><thead><tr><th v-for="key in columns" :key="key">{{key}}</th></tr></thead><tbody><tr v-for="(row,i) in rows" :key="i"><td v-for="key in columns" :key="key">{{cell(row,key)}}</td></tr></tbody></table></div>
  <CodeBlock v-else-if="renderer==='code'" :code="text" :language="language" :show-line-numbers="true"/>
  <div v-else-if="renderer==='markdown'" class="result-markdown"><p v-for="(line,i) in lines" :key="i" :class="{heading:line.heading}"><span v-if="line.bullet">• </span><template v-for="(part,j) in line.parts" :key="j"><strong v-if="part.startsWith('**')">{{part.slice(2,-2)}}</strong><template v-else>{{part}}</template></template></p></div>
  <pre v-else>{{text}}</pre>
</template>
