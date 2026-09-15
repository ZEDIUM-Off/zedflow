import type { NodeActivity } from '@zedflow/sdk'
const indexes=new WeakMap<NodeActivity[],Map<string,NodeActivity[]>>()
export function passagesByNode(activities:NodeActivity[]=[]){
  const previous=indexes.get(activities);if(previous)return previous
  const index=new Map<string,NodeActivity[]>()
  for(const activity of activities){const path=activity.path||activity.node;const values=index.get(path)||[];values.push(activity);index.set(path,values)}
  indexes.set(activities,index);return index
}
