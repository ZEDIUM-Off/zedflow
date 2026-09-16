export type AttachmentSlot = 'instructions' | 'skills' | 'files' | 'tools'
import type { NodeAttachments as AgentAttachments } from '@zedflow/sdk'
export type { InstructionItem, SkillItem, FileItem, ToolItem, NodeAttachments as AgentAttachments } from '@zedflow/sdk'
export const attachmentSlots: {id: AttachmentSlot; label: string; hint: string}[] = [
  {id: 'instructions', label: 'Instructions', hint: 'Texte et fichiers AGENTS.md'},
  {id: 'skills', label: 'Skills', hint: 'Catalogue et skills sélectionnés'},
  {id: 'files', label: 'Fichiers', hint: 'Contexte, portions et limites'},
  {id: 'tools', label: 'Outils', hint: 'Primitives proposées au modèle'},
]
export const primitiveTools = ['read', 'write', 'edit', 'exec', 'inspect_json', 'format_text', 'delay']
export function instructionAttachments(text: string): AgentAttachments {
  return {instructions: {items: [{id: 'instructions', source: {kind: 'text', text}, mode: 'literal', activation: 'always'}]}}
}
