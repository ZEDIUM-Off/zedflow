export type AttachmentSlot = 'instructions' | 'skills' | 'files' | 'tools'
interface Item { id: string; enabled?: boolean }
export interface InstructionItem extends Item {activation?: 'always' | 'explicit'; source: {kind: 'text'; text: string} | {kind: 'file'; path: string} | {kind: 'workspace'}; mode?: 'literal' | 'template'}
export interface SkillItem extends Item {name?: string; source: {kind: 'workspace'} | {kind: 'file'; path: string}; activation?: 'always' | 'explicit'}
export interface FileItem extends Item {path: string; activation?: 'always' | 'explicit'; startLine?: number; endLine?: number; maxChars?: number}
export interface ToolItem extends Item {name: string}
export interface AgentAttachments {instructions?: {items: InstructionItem[]}; skills?: {items: SkillItem[]}; files?: {items: FileItem[]}; tools?: {items: ToolItem[]}}
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
