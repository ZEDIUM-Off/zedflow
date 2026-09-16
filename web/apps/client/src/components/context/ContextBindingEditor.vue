<script setup lang="ts">
import type { JsonValue, ResourceBinding, ReaderContract, SourceReference } from '@zedflow/sdk'

const client=useClient()
import { HttpError, requireNodeConfig } from '@zedflow/sdk'
import { httpDiagnostics } from '../../contextEngine'

import { useClient } from '@zedflow/vue'

import { computed, ref, shallowRef, watch } from 'vue'
import { ArrowUpRight, Check, RefreshCw } from 'lucide-vue-next'
import type { Composition, FlowNode } from '@zedflow/sdk'
import { textExpression, defaultContextValue, typeLabel } from '../../contextEngine'
import type { ContextTypesFile, ContextExpr, ContextLibraryFile, ContextFile, ContextType, ContextDiagnostic } from '@zedflow/sdk'
import ContextTypeEditor from './ContextTypeEditor.vue'
import ContextExpressionEditor from './ContextExpressionEditor.vue'
import ContextRuntimeSettings from './ContextRuntimeSettings.vue'
import { flowExports } from '../../compositionEngine'

import ContextLibrarySelection from './ContextLibrarySelection.vue'
import ContextTypesSelection from './ContextTypesSelection.vue'
import ContextValueEditor from './ContextValueEditor.vue'
const props=defineProps<{node:FlowNode;composition?:Composition;workspaceId?:string}>()
const emit=defineEmits<{edit:[key?:string]}>()
type Binding = ResourceBinding
type ReaderBinding = Extract<Binding, { kind: 'reader' }>
function referenceKey(reference: SourceReference | null | undefined) { return typeof reference === 'string' ? reference : reference?.key }
function referenceHash(reference: SourceReference | null | undefined) { return typeof reference === 'object' ? reference?.hash : undefined }
const files=shallowRef<ContextFile[]>([]),loading=ref(false),validating=ref(false),error=ref(''),notice=ref(''),diagnostics=shallowRef<ContextDiagnostic[]>([])
const typeName=ref(''),libraryFile=shallowRef<ContextLibraryFile>(),typesFile=shallowRef<ContextTypesFile>()
let intent=0
const readers=shallowRef<ReaderContract[]>([])
const config=computed(()=>requireNodeConfig(props.node.data.config))
const key=computed(()=>typeof config.value.contextStrategy==='string'?config.value.contextStrategy:referenceKey(config.value.contextStrategy)||'')
const selected=computed(()=>files.value.find(file=>file.key===key.value))
const producerBranches=computed(()=>Object.entries(flowExports(props.composition)?.contract.branches||{}).filter(([name,branch])=>branch.invocations.includes('context')&&flowExports(props.composition)?.branches[name]===props.node.id).map(([name])=>name))
const requirements=computed(()=>selected.value?.strategy?.requirements||{})
const bindings=computed<Record<string,Binding>>(()=>config.value.contextBindings||{})
const types=computed<Record<string,ContextType>>(()=>typesFile.value?.types||config.value.contextTypes||{})
const attachments=computed(()=>{
  const source=config.value.attachments||{}
  return [
    ...(source.instructions?.items||[]).map(item=>({id:item.id,label:`Instruction · ${item.id}`,enabled:item.enabled!==false})),
    ...(source.files?.items||[]).map(item=>({id:item.id,label:`Fichier · ${item.path||item.id}`,enabled:item.enabled!==false})),
    ...(source.skills?.items||[]).map(item=>({id:item.id,label:`Skill · ${item.name||item.id}`,enabled:item.enabled!==false})),
  ]
})
const libraryMismatch=computed(()=>!!libraryFile.value&&!!referenceHash(config.value.contextLibraryRef)&&libraryFile.value.hash!==referenceHash(config.value.contextLibraryRef))
const hasHashMismatch=computed(()=>selected.value&&typeof config.value.contextStrategy==='object'&&referenceHash(config.value.contextStrategy)&&referenceHash(config.value.contextStrategy)!==selected.value.hash)
async function refresh(){
  const version=++intent,workspace=props.workspaceId
  if(!workspace){files.value=[];return}
  loading.value=true;error.value=''
  try{const result=await client.context.list({workspaceId:workspace});if(version===intent)files.value=Array.isArray(result)?result:[]}
  catch(cause){if(version===intent)error.value=cause instanceof Error?cause.message:String(cause)}
  finally{if(version===intent)loading.value=false}
}
function choose(value:string){
  notice.value='';error.value='';diagnostics.value=[]
  if(!value){delete config.value.contextStrategy;delete config.value.contextProgram;return}
  const file=files.value.find(file=>file.key===value);if(!file?.strategy)return
  config.value.contextStrategy={key:file.key,hash:file.hash};delete config.value.contextProgram
  config.value.contextBindings=Object.fromEntries(Object.entries(bindings.value).filter(([name])=>Object.keys(file.strategy!.requirements).includes(name)))
}
function bind(name:string,kind:string){
  if (!['', 'conversation', 'attachments', 'reader', 'state', 'attachment', 'produced', 'entity'].includes(kind)) return
  config.value.contextBindings ||= {}
  if(!kind){delete config.value.contextBindings[name];return}
  config.value.contextBindings[name]=kind==='conversation'?{kind,historyField:'messages',inputField:'input'}:kind==='attachments'?{kind,slot:'instructions'}:kind==='reader'?{kind,reader:readers.value[0]?.id||'file.text',input:{kind:'literal',value:defaultContextValue(readers.value[0]?.input||{kind:'record',fields:{path:{kind:'text'}}})}}:kind==='state'?{kind,field:''}:kind==='attachment'?{kind,itemId:''}:kind==='produced'?{kind,producer:{branch:producerBranches.value[0]||'',routeId:'',input:textExpression()}}:{kind:'entity',scope:{kind:'runtime'},alias:''}
  notice.value='';error.value=''
}
function scope(binding:Binding,kind:string){if(binding.kind!=='entity')return;if(kind==='runtime')binding.scope={kind};else if(kind==='flow'||kind==='bridge')binding.scope={kind,id:''}}
function optional(binding:Binding,field:'pointer'|'encoding'|'skillName'|'revision',value:string){
  if (binding.kind === 'state' && field === 'pointer') { if(value)binding.pointer=value;else delete binding.pointer }
  if (binding.kind === 'state' && field === 'encoding') { if(value==='adkMessages')binding.encoding=value;else if(!value)delete binding.encoding }
  if (binding.kind === 'attachment' && field === 'skillName') { if(value)binding.skillName=value;else delete binding.skillName }
  if (binding.kind === 'entity' && field === 'revision') { if(value)binding.revision=value;else delete binding.revision }
}
function setType(name:string,type:ContextType){config.value.contextTypes ||= {}; config.value.contextTypes[name]=type}
function addType(){const name=typeName.value.trim();if(!name||Object.hasOwn(types.value,name)){error.value='Choisissez un nom de type unique.';return}config.value.contextTypes ||= {};config.value.contextTypes[name]={kind:'record',fields:{}};typeName.value=''}
async function validate(){
  if(!selected.value?.strategy||!props.workspaceId)return
  if(typesFile.value&&referenceHash(config.value.contextTypesRef)&&typesFile.value.hash!==referenceHash(config.value.contextTypesRef)){error.value='Le catalogue de types a changé. Sélectionnez sa nouvelle version explicitement.';return}
  if(libraryMismatch.value){error.value='La bibliothèque a changé depuis sa sélection. Adoptez explicitement la nouvelle version.';return}
  const workspace=props.workspaceId,node=props.node,configuration=JSON.stringify(config.value)
  error.value='';notice.value='';diagnostics.value=[]
  for(const [name,binding] of Object.entries(bindings.value)){
    if(binding.kind==='produced'&&(!binding.producer.branch||!binding.producer.routeId)){error.value=`${name} : choisissez un branchement producteur et sa route explicite sur ce nœud.`;return}
    if(binding.kind==='state'&&(!binding.field.trim()||(binding.pointer&&!binding.pointer.startsWith('/')))){error.value=`${name} : indiquez un champ d’état et un JSON Pointer commençant par /, ou laissez le pointer vide.`;return}
    if(binding.kind==='attachment'&&!attachments.value.some(item=>item.id===binding.itemId&&item.enabled)){error.value=`${name} : choisissez une pièce activée sur ce nœud.`;return}
    if(binding.kind==='entity'&&(!binding.alias.trim()||(binding.scope.kind!=='runtime'&&!binding.scope.id.trim()))){error.value=`${name} : indiquez l’alias et l’identité de sa portée.`;return}
  }
  validating.value=true
  try{
    const grants=[...(config.value.attachments?.tools?.items||[]).filter((item:{enabled?:boolean})=>item.enabled!==false).map((item:{name:string})=>item.name),...(config.value.capabilityGrants||[]).map((item:{id:string})=>item.id)]
    await client.context.validate({workspaceId:workspace,selection:{kind:'file',key:selected.value.key,hash:referenceHash(config.value.contextStrategy)||selected.value.hash},types:types.value,library:libraryFile.value?.library||config.value.contextLibrary||{projections:{},subprograms:{}},resources:requirements.value,grantedCapabilities:grants})
    if(node===props.node&&workspace===props.workspaceId&&configuration===JSON.stringify(config.value))notice.value='Contrats de types et capacités vérifiés. Les valeurs et ressources absentes seront vérifiées au passage du nœud.'
  }catch(cause){if(node===props.node&&workspace===props.workspaceId&&configuration===JSON.stringify(config.value)){error.value=cause instanceof Error?cause.message:String(cause);if(cause instanceof HttpError)diagnostics.value=httpDiagnostics(cause)}}
  finally{validating.value=false}
}
watch(()=>props.workspaceId,()=>{files.value=[];void refresh()},{immediate:true})
watch(()=>props.node.id,()=>{error.value='';notice.value='';diagnostics.value=[]})
let libraryIntent=0
watch(()=>[props.workspaceId,referenceKey(config.value.contextLibraryRef),referenceHash(config.value.contextLibraryRef)],async()=>{const version=++libraryIntent;libraryFile.value=undefined;const key=referenceKey(config.value.contextLibraryRef);if(!key||!props.workspaceId)return;try{const file=await client.context.readLibrary(key,{workspaceId:props.workspaceId});if(version===libraryIntent)libraryFile.value=file}catch(cause){if(version===libraryIntent)error.value=String(cause instanceof Error?cause.message:cause)}},{immediate:true})
function chooseLibrary(file:ContextLibraryFile|undefined){if(file){config.value.contextLibraryRef={key:file.key,hash:file.hash};libraryFile.value=file}else{delete config.value.contextLibraryRef;libraryFile.value=undefined}delete config.value.contextProgram}
watch(()=>props.workspaceId,async()=>{const workspace=props.workspaceId;if(!workspace)return;try{const value=await client.context.readers();if(workspace===props.workspaceId)readers.value=Array.isArray(value)?value:[]}catch{/* A reader is never added implicitly. */}},{immediate:true})
function readerType(binding:ReaderBinding){return readers.value.find(item=>item.id===binding.reader)?.input||{kind:'record',fields:{path:{kind:'text'}}} as ContextType}
function readerChoice(binding:ReaderBinding,id:string){binding.reader=id;binding.input={kind:'literal',value:defaultContextValue(readerType(binding))}}
function readerInput(binding:ReaderBinding,kind:string){binding.input=kind==='state'?{kind:'state',field:''}:{kind:'literal',value:defaultContextValue(readerType(binding))}}
let typesIntent=0
watch(()=>[props.workspaceId,referenceKey(config.value.contextTypesRef),referenceHash(config.value.contextTypesRef)],async()=>{const request=++typesIntent;typesFile.value=undefined;const key=referenceKey(config.value.contextTypesRef);if(!key||!props.workspaceId)return;try{const file=await client.context.readTypes(key,{workspaceId:props.workspaceId});if(request===typesIntent)typesFile.value=file}catch(cause){if(request===typesIntent)error.value=cause instanceof Error?cause.message:String(cause)}},{immediate:true})
function chooseTypes(file:ContextTypesFile|undefined){typesFile.value=file;if(file)config.value.contextTypesRef={key:file.key,hash:file.hash};else delete config.value.contextTypesRef;delete config.value.contextProgram}

</script>
<template>
  <details class="settings-group ctx-binding-editor" open><summary>Stratégie de contexte</summary><label>Stratégie Rust<select aria-label="Stratégie Rust" :disabled="loading||!workspaceId" :value="key" @change="choose(($event.target as HTMLSelectElement).value)"><option value="">{{node.data.kind==='context'?'Choisir une stratégie':'Pièces du nœud · configuration actuelle'}}</option><option v-for="file in files" :key="file.key" :value="file.key" :disabled="!file.strategy">{{file.strategy?.name||file.key}}{{file.strategy?'':' · source invalide'}}</option><option v-if="key&&!selected" :value="key">{{key}} · introuvable</option></select></label><div class="ctx-binding-actions"><button :disabled="loading" aria-label="Actualiser les stratégies du nœud" @click="refresh"><RefreshCw :size="13"/></button><button @click="emit('edit',key||undefined)">Ouvrir le studio de contexte <ArrowUpRight :size="13"/></button></div><template v-if="selected?.strategy"><p class="muted">Cette stratégie prépare toute la fenêtre du modèle. Reliez ses besoins aux ressources ci-dessous. Les sources et capacités accordées restent configurables à la fin du panneau.</p><p v-if="hasHashMismatch" role="alert" class="field-error">Le fichier a changé depuis sa sélection. <button @click="choose(selected.key)">Adopter cette version</button></p><div v-for="(type,name) in requirements" :key="name" class="ctx-binding" :data-context-binding="name"><strong>{{name}} <small>{{typeLabel(type)}}</small></strong><label>Source de {{name}}<select :aria-label="`Source de ${name}`" :value="bindings[name]?.kind||''" @change="bind(name,($event.target as HTMLSelectElement).value)"><option value="">Absente · besoin géré par le programme</option><option value="conversation">Conversation et nouvelle demande</option><option value="state">Champ de l’état</option><option value="attachments">Ensemble des instructions, skills ou fichiers</option><option value="attachment">Une ressource précise</option><option value="entity">Entité du graphe résolu</option><option value="reader">Lecteur de ressource</option><option value="produced">Producteur par route de contexte</option></select></label><template v-if="bindings[name]?.kind==='conversation'"><label>Canal de la conversation<input :aria-label="`Historique de ${name}`" v-model="(bindings[name] as Extract<Binding,{kind:'conversation'}>).historyField"/></label><label>Canal de la nouvelle demande<input :aria-label="`Nouvelle demande de ${name}`" v-model="(bindings[name] as Extract<Binding,{kind:'conversation'}>).inputField"/></label><small>Compile l’historique et ajoute la demande courante une seule fois, y compris au premier passage.</small></template><template v-else-if="bindings[name]?.kind==='attachments'"><label>Ensemble de ressources<select :aria-label="`Ressources de ${name}`" v-model="(bindings[name] as Extract<Binding,{kind:'attachments'}>).slot"><option value="instructions">Instructions activées</option><option value="skills">Catalogue et skills activés</option><option value="files">Fichiers sélectionnés</option></select></label><small>Réunit les ressources accordées sur ce nœud, selon leur activation. Le programme choisit ensuite leur projection.</small></template><template v-else-if="bindings[name]?.kind==='reader'"><label>Lecteur<select :aria-label="`Lecteur de ${name}`" :value="(bindings[name] as ReaderBinding).reader" @change="readerChoice(bindings[name] as ReaderBinding,($event.target as HTMLSelectElement).value)"><option v-for="reader in readers" :key="reader.id" :value="reader.id">{{reader.id}} · {{reader.version}}</option></select></label><label>Paramètres du lecteur<select :aria-label="`Entrée du lecteur ${name}`" :value="(bindings[name] as ReaderBinding).input.kind" @change="readerInput(bindings[name] as ReaderBinding,($event.target as HTMLSelectElement).value)"><option value="literal">Paramètres explicites</option><option value="state">Depuis un champ d’état</option></select></label><ContextValueEditor v-if="(bindings[name] as ReaderBinding).input.kind==='literal'" v-model="((bindings[name] as ReaderBinding).input as {kind:'literal';value:JsonValue}).value" :type="readerType(bindings[name] as ReaderBinding)" :label="`Paramètres de ${name}`"/><template v-else><label>Champ des paramètres<input :aria-label="`Champ du lecteur ${name}`" v-model="((bindings[name] as ReaderBinding).input as {kind:'state';field:string}).field"/></label><label>JSON Pointer facultatif<input v-model="((bindings[name] as ReaderBinding).input as {kind:'state';field:string;pointer?:string}).pointer" placeholder="/source"/></label></template><small>La lecture a lieu uniquement lorsque le programme consulte cette ressource. Sa provenance est capturée pour le passage.</small></template><template v-else-if="bindings[name]?.kind==='produced'"><label>Branchement producteur<select :aria-label="`Producteur de ${name}`" v-model="(bindings[name] as Extract<Binding,{kind:'produced'}>).producer.branch"><option value="" disabled>Choisir un branchement exposé</option><option v-for="branch in producerBranches" :key="branch">{{branch}}</option></select></label><p v-if="!producerBranches.length" class="muted">Exposez un branchement de contexte sur ce nœud dans les contrats du Départ.</p><ContextExpressionEditor v-model="(bindings[name] as Extract<Binding,{kind:'produced'}>).producer.input" :resources="requirements" :types="types" label="Entrée du producteur"/><label>Route précise · requise<input v-model="(bindings[name] as Extract<Binding,{kind:'produced'}>).producer.routeId" placeholder="bridge/connexion"/></label><label>Champ du résultat · JSON Pointer facultatif<input v-model="(bindings[name] as Extract<Binding,{kind:'produced'}>).producer.outputPointer" placeholder="/document"/></label></template><template v-else-if="bindings[name]?.kind==='state'"><label>Champ d’état pour {{name}}<input :aria-label="`Champ d’état pour ${name}`" v-model="(bindings[name] as Extract<Binding,{kind:'state'}>).field" placeholder="input"/></label><label>JSON Pointer · facultatif<input :value="(bindings[name] as Extract<Binding,{kind:'state'}>).pointer" @input="optional(bindings[name],'pointer',($event.target as HTMLInputElement).value)" placeholder="/document/title"/></label><label>Encodage<select aria-label="Encodage" :value="(bindings[name] as Extract<Binding,{kind:'state'}>).encoding||''" @change="optional(bindings[name],'encoding',($event.target as HTMLSelectElement).value)"><option value="">Valeur typée</option><option value="adkMessages">Messages ADK sérialisés</option></select></label></template><template v-else-if="bindings[name]?.kind==='attachment'"><label>Pièce<select aria-label="Pièce" v-model="(bindings[name] as Extract<Binding,{kind:'attachment'}>).itemId"><option value="" disabled>Choisir une pièce</option><option v-for="item in attachments" :key="item.id" :value="item.id" :disabled="!item.enabled">{{item.label}}{{item.enabled?'':' · désactivée'}}</option></select></label><label>Nom du skill · si catalogue<input :value="(bindings[name] as Extract<Binding,{kind:'attachment'}>).skillName" @input="optional(bindings[name],'skillName',($event.target as HTMLInputElement).value)"/></label></template><template v-else-if="bindings[name]?.kind==='entity'"><label>Portée<select aria-label="Portée de l’entité" :value="(bindings[name] as Extract<Binding,{kind:'entity'}>).scope.kind" @change="scope(bindings[name],($event.target as HTMLSelectElement).value)"><option value="runtime">RuntimeGraph courant</option><option value="flow">Flow</option><option value="bridge">Bridge activé</option></select></label><label v-if="(bindings[name] as Extract<Binding,{kind:'entity'}>).scope.kind!=='runtime'">Identité de la portée<input v-model="((bindings[name] as Extract<Binding,{kind:'entity'}>).scope as {id:string}).id"/></label><label>Alias<input v-model="(bindings[name] as Extract<Binding,{kind:'entity'}>).alias"/></label><label>Révision · facultative<input :value="(bindings[name] as Extract<Binding,{kind:'entity'}>).revision" @input="optional(bindings[name],'revision',($event.target as HTMLInputElement).value)" placeholder="Dernière révision disponible"/></label></template></div><p v-if="libraryMismatch" role="alert" class="field-error">La bibliothèque sélectionnée a changé. <button @click="chooseLibrary(libraryFile)">Adopter la version de la bibliothèque</button></p><ContextLibrarySelection :workspace-id="workspaceId||''" :file="libraryFile" @select="chooseLibrary" @edit="emit('edit',key||undefined)"/><ContextTypesSelection :workspace-id="workspaceId||''" :file="typesFile" @select="chooseTypes" @edit="emit('edit',key||undefined)"/><p v-if="typesFile&&referenceHash(config.contextTypesRef)!==typesFile.hash" role="alert" class="field-error">Le catalogue de types a changé. <button @click="chooseTypes(typesFile)">Adopter ces types</button></p><details v-if="!config.contextTypesRef"><summary>Registre de types du flow</summary><ContextTypeEditor v-for="(_,name) in types" :key="name" :model-value="types[name]" @update:model-value="setType(name,$event)" :label="name" :names="Object.keys(types)"/><div class="ctx-add-field"><input v-model="typeName" aria-label="Nouveau type du flow" placeholder="Nom du type" @keydown.enter.prevent="addType"/><button aria-label="Ajouter le type au flow" @click="addType">+</button></div></details><ContextRuntimeSettings :node="node" :composition="composition"/><button :disabled="loading||validating" @click="validate"><Check :size="13"/>Vérifier les contrats</button></template><p v-if="error" role="alert" class="field-error">{{error}}</p><p v-for="(diagnostic,index) in diagnostics" :key="index" class="muted">{{diagnostic.path}} · {{diagnostic.message}}</p><p v-if="notice" role="status" class="muted">{{notice}}</p></details>
</template>
<style scoped>
.ctx-binding-actions{display:flex;gap:5px;margin:8px 0}.ctx-binding-actions button{display:flex;align-items:center;gap:5px;font-size:11px}.ctx-binding{display:flex;flex-direction:column;gap:8px;border-left:2px solid #555560;margin:16px 0;padding-left:10px}.ctx-binding>strong{font-size:12px;display:flex;align-items:center;justify-content:space-between}.ctx-binding small{font-weight:400;color:#92929f}.ctx-binding-editor details{margin:14px 0}.ctx-binding-editor details>summary{font-size:11px;cursor:pointer}.ctx-binding-editor>button{display:flex;align-items:center;gap:6px;font-size:12px}
</style>
