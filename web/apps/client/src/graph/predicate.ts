import type { Predicate } from '@zedflow/sdk'
export const comparisonOperators = [{value:'eq',label:'Est égal à'},{value:'ne',label:'Est différent de'},{value:'gt',label:'Est supérieur à'},{value:'gte',label:'Est supérieur ou égal à'},{value:'lt',label:'Est inférieur à'},{value:'lte',label:'Est inférieur ou égal à'},{value:'exists',label:'Est présent'},{value:'contains',label:'Contient'},{value:'in',label:'Est inclus dans'}] as const
export const defaultPredicate = (): Predicate => ({kind:'compare',field:'input',operator:'eq',value:'oui'})
export function predicateLabel(predicate: Predicate): string {
  if (predicate.kind !== 'compare') return `${predicate.kind==='all'?'ET':'OU'} · ${predicate.items.length} critères`
  const symbols = {eq:'=',ne:'≠',gt:'>',gte:'≥',lt:'<',lte:'≤',exists:'est présent',contains:'contient',in:'∈'}
  return `${predicate.field} ${symbols[predicate.operator]}${predicate.operator==='exists'?'':` ${JSON.stringify(predicate.value)}`}`
}
