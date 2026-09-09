'use strict';
const $ = id => document.getElementById(id);
const NS = 'http://www.w3.org/2000/svg';
let catalog = [], detail, current, runData, cursor = -1, selected, tab = 'state', timer, busy = false;
const savedRuns = new Map();
const labels = {node_start:'Entrée dans le nœud',node_end:'Nœud terminé',checkpoint:'État sauvegardé',state:'État confirmé',updates:'Mise à jour',route_dispatched:'Route choisie',done:'Exécution terminée',interrupted:'Pause',resumed:'Reprise',custom:'Événement agent',error:'Erreur',step_complete:'Étape terminée',message:'Message'};

function error(message) { $('error').textContent = message || ''; $('error').hidden = !message; }
async function api(path, options) {
  const response = await fetch(path, options);
  const body = await response.text();
  let data;
  try { data = JSON.parse(body); } catch { throw new Error(body || `HTTP ${response.status}`); }
  if (!response.ok) throw new Error(data.error || `HTTP ${response.status}`);
  return data;
}
function status(value) {
  const names = {ready:'Prêt',running:'Exécution…',completed:'Terminé',paused:'En pause',error:'Erreur',incomplete:'Incomplet'};
  $('status').textContent = names[value] || value;
  $('status').dataset.status = value;
}
function stop() { clearInterval(timer); timer = undefined; $('play').textContent = 'Lire le parcours'; }

// Keep playback inside its panel without moving the whole page away from the graph.
function reveal(container, element) {
  if (!element) return;
  const outer = container.getBoundingClientRect(), inner = element.getBoundingClientRect();
  if (inner.top < outer.top) container.scrollTop += inner.top - outer.top;
  else if (inner.bottom > outer.bottom) container.scrollTop += inner.bottom - outer.bottom;
}

async function choose(id) {
  if (busy) return;
  stop(); error(); busy = true;
  try {
    detail = await api(`/api/flows/${id}`);
    current = catalog.find(flow => flow.id === id);
    runData = savedRuns.get(id); selected = undefined;
    $('title').textContent = current.title; $('description').textContent = current.description;
    $('input').value = JSON.stringify(current.input, null, 2);
    $('project-field').hidden = id !== 'memory';
    $('limitations').textContent = detail.limitations;
    $('subgraph').hidden = id !== 'agent';
    $('channels').textContent = `Canaux déclarés : ${detail.channels.join(', ')}`;
    for (const button of $('flows').children) button.classList.toggle('active', button.dataset.id === id);
    renderTrace(); renderGraph(); renderInspector();
  } catch (e) { error(e.message); } finally { busy = false; }
}

function svgElement(tag, attrs, text) {
  const element = document.createElementNS(NS, tag);
  for (const [key, value] of Object.entries(attrs || {})) element.setAttribute(key, value);
  if (text !== undefined) element.textContent = text;
  return element;
}
function renderGraph() {
  const svg = $('graph'); svg.replaceChildren();
  const topology = runData?.topology || detail.topology;
  const members = topology.members;
  const edges = topology.relationships;
  const levels = new Map([[topology.root, 0]]), queue = [topology.root];
  while (queue.length) {
    const from = queue.shift();
    for (const edge of edges.filter(edge => edge.from === from)) {
      if (!levels.has(edge.to)) { levels.set(edge.to, levels.get(from) + 1); queue.push(edge.to); }
    }
  }
  for (const node of members) if (!levels.has(node.name)) levels.set(node.name, levels.size);
  const groups = new Map();
  for (const node of members) {
    const level = levels.get(node.name);
    if (!groups.has(level)) groups.set(level, []);
    groups.get(level).push(node.name);
  }
  const width = 650, height = (Math.max(...levels.values()) + 1) * 94 + 30;
  svg.setAttribute('viewBox', `0 0 ${width} ${height}`);
  const positions = new Map();
  for (const [level, names] of groups) names.forEach((name, i) => positions.set(name, {x:width*(i+1)/(names.length+1), y:35+level*94}));
  const defs = svgElement('defs'); const marker = svgElement('marker',{id:'arrow',viewBox:'0 0 10 10',refX:9,refY:5,markerWidth:6,markerHeight:6,orient:'auto-start-reverse'});
  marker.append(svgElement('path',{d:'M 0 0 L 10 5 L 0 10 z',fill:'#8da497'})); defs.append(marker); svg.append(defs);
  for (const edge of edges) {
    const a = positions.get(edge.from), b = positions.get(edge.to); if (!a || !b) continue;
    const back = b.y <= a.y;
    const path = back ? `M ${a.x+102} ${a.y} C ${a.x+220} ${a.y}, ${b.x+220} ${b.y}, ${b.x+102} ${b.y}` : `M ${a.x} ${a.y+24} C ${a.x} ${a.y+58}, ${b.x} ${b.y-58}, ${b.x} ${b.y-25}`;
    svg.append(svgElement('path',{d:path,class:'edge','marker-end':'url(#arrow)'}));
  }
  const visited = new Set((runData?.events || []).slice(0,cursor+1).filter(e=>e.type==='node_end').map(e=>e.node));
  for (const member of members) {
    const pos = positions.get(member.name);
    const node = svgElement('g',{transform:`translate(${pos.x-102},${pos.y-24})`,class:`node${member.coordinator?' root':''}${visited.has(member.name)?' visited':''}${member.name===selected?' selected':''}`,tabindex:0,role:'button','aria-label':`Nœud ${member.name}`});
    node.append(svgElement('rect',{width:204,height:48,rx:7}));
    node.append(svgElement('text',{x:102,y:20,'text-anchor':'middle'},member.coordinator?'Entrée du flow':member.name));
    node.append(svgElement('text',{x:102,y:36,'text-anchor':'middle',class:'kind'},member.coordinator?'coordination ADK':(current.id==='agent'&&member.name==='research'?'sous-graphe de recherche':'nœud exécutable')));
    const select = () => { selected = member.name; tab = 'source'; renderGraph(); renderInspector(); };
    node.addEventListener('click',select); node.addEventListener('keydown',event=>{if(event.key==='Enter'||event.key===' '){event.preventDefault();select();}}); svg.append(node);
  }
  $('node-count').textContent = `${members.length-1} nœuds · ${edges.length} connexions`;
}

function renderTrace() {
  const events = runData?.events || []; cursor = events.length-1;
  $('events').replaceChildren();
  if (!events.length) {
    const message = document.createElement('p'); message.className='empty'; message.textContent='Exécute le flow, puis parcours ses événements pour voir les nœuds traversés et l’évolution de l’état.'; $('events').append(message);
  }
  events.forEach((event,index)=>{
    const button=document.createElement('button');
    for (const text of [String(index+1).padStart(2,'0'), labels[event.type]||event.type, event.node || event.source || (event.step!==undefined?`étape ${event.step}`:'')]) {
      const span=document.createElement('span');span.textContent=text;button.append(span);
    }
    button.lastChild.className='event-node';button.onclick=()=>{stop();setCursor(index);};$('events').append(button);
  });
  $('trace-count').textContent = events.length?`${events.length} événements`:'Aucun run';
  $('position').max = Math.max(0,events.length-1);
  for (const id of ['prev','next','play','position','download']) $(id).disabled = !events.length;
  $('resume').hidden = runData?.status!=='paused';
  status(runData?.status || 'ready'); setCursor(cursor);
}
function setCursor(index) {
  const events=runData?.events || [];cursor=Math.max(-1,Math.min(index,events.length-1));
  $('position').value=Math.max(0,cursor);$('counter').textContent=events.length?`${cursor+1} / ${events.length}`:'—';
  if (events[cursor]?.node) selected=events[cursor].node;
  [...$('events').children].forEach((element,i)=>element.classList.toggle('active',i===cursor));
  if (events.length) reveal($('events'), $('events').children[cursor]);
  if(detail){renderGraph();renderInspector();}
}
function renderInspector() {
  document.querySelectorAll('[data-tab]').forEach(button=>button.setAttribute('aria-selected',String(button.dataset.tab===tab)));
  const pane=$('inspect');pane.replaceChildren();
  if(tab==='source') {
    $('inspect-caption').textContent=`${detail.file} · source du binaire chargé`;
    detail.source.split('\n').forEach((line,i)=>{
      const span=document.createElement('span');span.className='source-line';
      if(selected&&line.includes(`"${selected}"`))span.classList.add('hot');
      const number=document.createElement('span');number.className='number';number.textContent=String(i+1);
      span.append(number,document.createTextNode(line||' '));pane.append(span);
    });
    reveal(pane, pane.querySelector('.hot')); return;
  }
  const events=runData?.events || [];
  if(tab==='event'){
    $('inspect-caption').textContent=events[cursor]?`Événement ${cursor+1} · ${labels[events[cursor].type]||events[cursor].type}`:'Aucun événement';
    pane.textContent=JSON.stringify(events[cursor]||{},null,2);return;
  }
  let state=runData?.input || current.input, step;
  for(const event of events.slice(0,cursor+1))if(event.state){state=event.state;step=event.step;}
  // A paused checkpoint may contain a more recent state than the last streamed snapshot.
  if(runData?.status==='paused'&&cursor===events.length-1&&runData.checkpoint){state=runData.checkpoint.state;step=runData.checkpoint.step;}
  $('inspect-caption').textContent=step!==undefined?`Dernier état confirmé · étape ${step}`:(runData?'Dernier état confirmé':'État initial');
  pane.textContent=JSON.stringify(state,null,2);
}

async function execute(resume=false) {
  if(busy||!current)return;error();stop();
  let input;try{input=resume?{}:JSON.parse($('input').value);if(!input||Array.isArray(input)||typeof input!=='object')throw new Error('L’état d’entrée doit être un objet JSON.');}catch(e){error(e.message);return;}
  busy=true;$('run').disabled=true;$('resume').disabled=true;status('running');
  try{
    const body={input,project:$('project').value};if(resume)body.resume=runData.thread;
    const result=await api(`/api/flows/${current.id}/run`,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(body)});
    if(resume&&runData){result.events=[...runData.events,...result.events];result.input=runData.input;}
    runData=result;savedRuns.set(current.id,result);tab='state';renderTrace();
    const failed=result.events.findLast(e=>e.type==='error');if(failed)error(failed.message);
  }catch(e){error(e.message);status('error');}finally{busy=false;$('run').disabled=false;$('resume').disabled=false;}
}
$('run-form').onsubmit=event=>{event.preventDefault();execute();};$('resume').onclick=()=>execute(true);
$('prev').onclick=()=>{stop();setCursor(cursor-1);};$('next').onclick=()=>{stop();setCursor(cursor+1);};
$('position').oninput=()=>{stop();setCursor(Number($('position').value));};
$('play').onclick=()=>{if(timer){stop();return;}if(cursor>=runData.events.length-1)setCursor(0);$('play').textContent='Pause lecture';timer=setInterval(()=>{setCursor(cursor+1);if(cursor>=runData.events.length-1)stop();},650);};
$('download').onclick=()=>{const url=URL.createObjectURL(new Blob([JSON.stringify(runData,null,2)],{type:'application/json'}));const link=document.createElement('a');link.href=url;link.download=`${runData.thread.replace(':','-')}.json`;link.click();setTimeout(()=>URL.revokeObjectURL(url),1000);};
document.querySelectorAll('[data-tab]').forEach(button=>button.onclick=()=>{tab=button.dataset.tab;renderInspector();});
$('subgraph').onclick=()=>choose('research');
(async()=>{try{catalog=await api('/api/flows');for(const flow of catalog){const button=document.createElement('button');button.dataset.id=flow.id;const title=document.createElement('strong');title.textContent=flow.title;const small=document.createElement('small');small.textContent=flow.id;button.append(title,small);button.onclick=()=>choose(flow.id);$('flows').append(button);}await choose('agent');}catch(e){error(e.message);}})();
