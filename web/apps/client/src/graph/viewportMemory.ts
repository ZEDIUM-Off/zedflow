import type { ViewportTransform } from '@vue-flow/core'
const cameras=new Map<string,ViewportTransform>()
export function rememberViewport(key:string,value:ViewportTransform){cameras.delete(key);cameras.set(key,{...value});if(cameras.size>200)cameras.delete(cameras.keys().next().value!)}
export function recalledViewport(key:string){const value=cameras.get(key);return value?{...value}:undefined}
