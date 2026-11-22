import {readFileSync,writeFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
import {execFileSync} from 'node:child_process';
const root = resolve(process.argv[2] ?? '.');
const output = resolve(process.argv[3] ?? 'tests/fixtures/arrangement/legacy_shared_links.json');
const siteCommit = execFileSync('git',['-C',root,'rev-parse','HEAD'],{encoding:'utf8'}).trim();
if(siteCommit !== '016882fcaa3fba669fc87e8261187918248f84ca') throw Error(`Unexpected site commit ${siteCommit}`);
let nextId=0;
const ids=new Map();
function normalizeIds(value) {
 if(typeof value === 'string' && /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(value)) {
  if(!ids.has(value)) ids.set(value,`00000000-0000-4000-8000-${String(++nextId).padStart(12,'0')}`);
  return ids.get(value);
 }
 if(Array.isArray(value)) return value.map(normalizeIds);
 if(value && typeof value === 'object') return Object.fromEntries(Object.entries(value).map(([k,v])=>[k,normalizeIds(v)]));
 return value;
}
const site = await import(pathToFileURL(resolve(root, 'app/arranging-combination-music.ts')));
const acorde = await import(pathToFileURL(resolve(root, 'public/vendor/acorde-wasm-viewer/acorde_wasm.js')));
await acorde.default({module_or_path:readFileSync(resolve(root, 'public/vendor/acorde-wasm-viewer/acorde_wasm_bg.wasm'))});
const corpus=[];
for(const id of site.combinationTuneIds){
 const held={pattern:'held',harmony:'basic',voicing:'single'};
 const full=JSON.parse(acorde.parse_abc(site.combinationAbc(id,held)));
 full.parts=full.parts.slice(0,1); full.part_groups=[];full.views=[];
 const normalizedSource=normalizeIds(full);
 const regions=[];
 for(const [measure, notes] of site.combinationBars(id,held).entries()){
   for(const bass of notes.filter(n=>n.part===1)){
     const chord=notes.find(n=>n.part===2 && n.beat===bass.beat && n.length===bass.length);
     if(!chord) throw Error(`missing chord ${id} ${measure}`);
     const label=bass.label; const parsed=/^([A-G])([#b]?)(m|7)?$/.exec(label);
     if(!parsed) throw Error(`unsupported label ${label}`);
     const root=parsed[1]+parsed[2],kind=parsed[3]==='m'?'minor':parsed[3]==='7'?'dominant':'major';
     regions.push({measure,start_tick:Math.round(bass.beat*32),end_tick:Math.round((bass.beat+bass.length)*32),bass_midi:bass.pitches[0],chord_midi:chord.pitches,chord:{root,kind},phase_beats:0});
   }
 }
 for(const pattern of ['held','pulse']){
  const config={pattern,harmony:'basic',voicing:'single'};
  const expected=site.combinationBars(id,config).flatMap((notes,bar)=>notes.flatMap(n=>n.pitches.map(p=>[p,Math.round((bar*site.combinationTunes[id].beats+n.beat)*960),Math.round(n.length*960)]))).sort((a,b)=>a[1]-b[1]||a[0]-b[0]||a[2]-b[2]);
  corpus.push({link:`https://fisarmonica.org/ja/arranging#combine/${id}/${site.combinationId(config)}`,id,source:normalizedSource,request:{contract_version:1,pattern:pattern==='held'?'held':'bass-chord-pulse',regions,midi_program:21},expected});
 }
}
const tunes=[];
for(let i=0;i<corpus.length;i+=2){
 const a=corpus[i],b=corpus[i+1];
 tunes.push({id:a.id,source:a.source,regions:a.request.regions,cases:[{link:a.link,pattern:a.request.pattern,expected:a.expected},{link:b.link,pattern:b.request.pattern,expected:b.expected}]});
}
writeFileSync(output,JSON.stringify({site_commit:'016882fcaa3fba669fc87e8261187918248f84ca',generation:'site combinationBars and combinationAbc with basic harmony and single melody',quarter_note_ticks:32,expected_ticks_per_beat:960,tunes})+'\n');
console.log(`Recorded ${corpus.length} cases in ${output}`);
