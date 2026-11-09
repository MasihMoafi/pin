import {invoke} from '@tauri-apps/api/core';
import {getCurrentWindow} from '@tauri-apps/api/window';
import type {Boot,Disk,Document,Save} from './model';
const preview=import.meta.env.DEV && new URLSearchParams(location.search).has('preview');
let demo:Boot={notes:!new URLSearchParams(location.search).has('pin'),disk:'',canonical:'',document:{text:'A place for a thought.\n\nWrite it down while it is fresh.\n',board:null},pinned:true,folded:[],dark:false,colors:[['🟢','#22c55e'],['🔵','#3b82f6'],['🟣','#a855f7'],['🟠','#f97316'],['🟡','#eab308'],['🔴','#ef4444'],['#ec4899','#ec4899'],['#14b8a6','#14b8a6'],['#06b6d4','#06b6d4'],['#84cc16','#84cc16'],['#a16207','#a16207'],['#64748b','#64748b']]};
if(!demo.notes)demo.document={text:'',board:{preamble:'',tasks:[{id:1,depth:0,text:'Elpis — Refactor based on Deepseek. A longer task wraps naturally without clipping the following line.',color:'🟢',date:null,done:false,daily:false,completed:null},{id:2,depth:0,text:"Read One body Post Daily, like the deep-seek harness.",color:'🟡',date:null,done:false,daily:true,completed:null},{id:3,depth:1,text:'A subtask with a clear place in the list.',color:null,date:null,done:false,daily:false,completed:null},{id:4,depth:0,text:'Workflow → What is the purpose exactly? Build a working result and check it visually.',color:'#ec4899',date:null,done:false,daily:false,completed:null},{id:5,depth:0,text:'Finished task',color:null,date:null,done:true,daily:false,completed:null}]}};
export async function boot():Promise<Boot>{return preview?structuredClone(demo):invoke<Boot>('get_state');}
export async function readDisk():Promise<Disk>{return preview?demo.disk:invoke<Disk>('read_disk');}
export async function save(document:Document,expected:Disk):Promise<Save>{
  if(preview){demo={...demo,document:structuredClone(document),disk:JSON.stringify(document)};return {saved:true,disk:demo.disk,content:demo.disk!};}
  return invoke<Save>('write_document',{document,expected});
}
export async function pin(on:boolean):Promise<void>{if(!preview)await invoke('pin_window',{on});}
export async function saveView(folded:string[]):Promise<void>{if(!preview)await invoke('save_view',{folded});}
export async function close():Promise<void>{if(!preview)await getCurrentWindow().destroy();}
export async function minimize():Promise<void>{if(!preview)await getCurrentWindow().minimize();}
export async function drag():Promise<void>{if(!preview)await getCurrentWindow().startDragging();}
export async function maximize():Promise<void>{if(!preview)await getCurrentWindow().toggleMaximize();}
export async function resize(direction:'North'|'NorthEast'|'East'|'SouthEast'|'South'|'SouthWest'|'West'|'NorthWest'):Promise<void>{if(!preview)await getCurrentWindow().startResizeDragging(direction);}
export async function readClipboard():Promise<string>{return preview?navigator.clipboard.readText():invoke<string>('read_clipboard');}
export async function writeClipboard(text:string):Promise<void>{if(preview)await navigator.clipboard.writeText(text);else await invoke('write_clipboard',{text});}
export async function onClose(fn:()=>void):Promise<()=>void>{return preview?()=>{}:getCurrentWindow().onCloseRequested(e=>{e.preventDefault();fn();});}
export async function ready():Promise<void>{if(!preview)await invoke('show_ready');}

export async function onTheme(fn:(dark:boolean)=>void):Promise<()=>void>{if(preview)return ()=>{};const {listen}=await import('@tauri-apps/api/event');return listen<boolean>('desktop-theme',e=>fn(e.payload));}
