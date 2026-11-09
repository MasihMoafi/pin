import {useEffect,useLayoutEffect,useReducer,useRef,useState,type ComponentProps,type CSSProperties,type ReactNode} from 'react';
import {Undo2,Redo2,Save,Check,Pin,Plus,Minus,X,MoreHorizontal,GripVertical,ChevronRight,ChevronDown,Trash2,ListPlus,Palette,Repeat2,CalendarDays,Copy,Scissors,ClipboardPaste,TextSelect} from 'lucide-react';
import {Button} from '@/components/ui/button';
import {Textarea} from '@/components/ui/textarea';
import {Checkbox} from '@/components/ui/checkbox';
import {Input} from '@/components/ui/input';
import {Badge} from '@/components/ui/badge';
import {Popover,PopoverTrigger,PopoverContent} from '@/components/ui/popover';
import {TooltipProvider,Tooltip,TooltipTrigger,TooltipContent} from '@/components/ui/tooltip';
import {Separator} from '@/components/ui/separator';
import {ContextMenu,ContextMenuTrigger,ContextMenuContent,ContextMenuItem,ContextMenuSeparator} from '@/components/ui/context-menu';
import * as host from './bridge';
import {History,blank,completedLast,end,foldKey,index,indent,localDay,moveBranch,normalize,shortcut,siblings,toggleDone,type Board,type Boot,type Disk,type Document,type Focus,type Snapshot,type Task} from './model';

function IconButton({label,children,...props}:ComponentProps<typeof Button>&{label:string;children:ReactNode}) {
  return <Tooltip><TooltipTrigger render={<Button variant="ghost" size="icon-sm" aria-label={label} title={label} data-no-drag {...props}>{children}</Button>}/><TooltipContent side="bottom">{label}</TooltipContent></Tooltip>;
}

type Runtime={loaded:boolean;notes:boolean;data:Document;disk:Disk;revision:number;savedRevision:number;saving:Promise<boolean>|null;conflict:boolean;error:string|null;pinned:boolean;folded:Set<string>;colors:[string,string][];history:History;lastFocus:Focus|null;before:Snapshot|null;today:string;status:'idle'|'saving'|'saved'};
const same=(a:Document,b:Document)=>JSON.stringify(a)===JSON.stringify(b);

export default function App(){
  const [,render]=useReducer((x:number)=>x+1,0);
  const runtime=useRef<Runtime>({loaded:false,notes:true,data:{text:'',board:null},disk:null,revision:0,savedRevision:0,saving:null,conflict:false,error:null,pinned:true,folded:new Set(),colors:[],history:new History(),lastFocus:null,before:null,today:localDay(),status:'idle'}).current;
  const editors=useRef(new Map<string,HTMLTextAreaElement>());
  const main=useRef<HTMLElement>(null);
  const pendingFocus=useRef<Focus|null>(null);
  const saveTimer=useRef<ReturnType<typeof setTimeout>|null>(null);
  const statusTimer=useRef<ReturnType<typeof setTimeout>|null>(null);
  const shown=useRef(false);
  const v=runtime;
  useLayoutEffect(()=>{if(!shown.current&&(v.loaded||v.error)){shown.current=true;void host.ready();}},[v.loaded,v.error]);

  function captureFocus():Focus|null{
    const field=document.activeElement;
    if(field instanceof HTMLTextAreaElement && field.dataset.editor){
      return {id:field.dataset.editor,start:field.selectionStart,end:field.selectionEnd,scroll:field.scrollTop,page:main.current?.scrollTop??0};
    }
    return v.lastFocus;
  }
  function snapshot():Snapshot{return {data:structuredClone(v.data),focus:captureFocus()};}
  function focus(id:string,start:number,end=start):void{pendingFocus.current={id,start,end,scroll:-1,page:main.current?.scrollTop??0};}
  useLayoutEffect(()=>{
    const f=pendingFocus.current;if(!f)return;
    const field=editors.current.get(f.id);if(!field)return;
    field.focus();field.setSelectionRange(Math.min(f.start,field.value.length),Math.min(f.end,field.value.length));
    if(f.scroll>=0)field.scrollTop=f.scroll;
    if(main.current)main.current.scrollTop=f.page;
    v.lastFocus=f;pendingFocus.current=null;
  });
  function schedule():void{
    if(saveTimer.current)clearTimeout(saveTimer.current);
    saveTimer.current=setTimeout(()=>void flush(),250);
  }
  function change(fn:(data:Document)=>void,selection:Focus|null=captureFocus(),typing?:string):void{
    const before=v.before??snapshot();v.before=null;
    const next=structuredClone(v.data);fn(next);
    if(same(next,v.data))return;
    v.history.record(before,typing);v.data=next;v.revision++;v.status='idle';
    if(!typing)pendingFocus.current=selection;
    schedule();render();
  }
  function undo(redo=false):void{
    if(!v.loaded)return;
    const next=v.history.restore(snapshot(),redo);if(!next)return;
    v.data=normalize(next.data,v.today);v.revision++;v.before=null;v.status='idle';
    pendingFocus.current=next.focus;v.lastFocus=next.focus;schedule();render();
  }
  async function flush(force=false):Promise<boolean>{
    if(saveTimer.current){clearTimeout(saveTimer.current);saveTimer.current=null;}
    if(!v.loaded||v.conflict)return false;
    if(v.saving){await v.saving;return flush(force);}
    if(!force&&v.savedRevision===v.revision)return true;
    const revision=v.revision,data=structuredClone(v.data),expected=v.disk;
    v.status='saving';v.error=null;render();
    v.saving=(async()=>{
      try{
        const result=await host.save(data,expected);
        if(!result.saved){v.conflict=true;v.status='idle';return false;}
        v.disk=result.content;v.savedRevision=revision;v.status='saved';
        if(statusTimer.current)clearTimeout(statusTimer.current);
        statusTimer.current=setTimeout(()=>{v.status='idle';render();},1400);
        return true;
      }catch(error){console.error(error);v.error='Couldn’t save. Check the folder’s permissions, then try Save again.';v.status='idle';return false;}
      finally{render();}
    })();
    const ok=await v.saving;v.saving=null;
    if(ok&&v.savedRevision!==v.revision)return flush();
    return ok;
  }
  function applyBoot(b:Boot,explicit=false):void{
    if(explicit)v.history.record(snapshot());else v.history.clear();
    const data=normalize(b.document,v.today);
    v.loaded=true;v.notes=b.notes;v.data=data;v.disk=b.disk;v.colors=b.colors;v.pinned=b.pinned;v.folded=new Set(b.folded);
    v.revision++;v.savedRevision=v.revision;
    if(b.canonical!==(b.disk??'')||!same(data,b.document)){v.savedRevision--;schedule();}
    v.conflict=false;v.error=null;v.before=null;
    document.documentElement.classList.toggle('dark',b.dark);document.title=b.notes?'Sticky Notes':'Pin';
    const id=b.notes?'notes':String(data.board?.tasks[0]?.id??'');
    if(id)focus(id,b.notes?data.text.length:0);
    render();
  }
  async function load(explicit=false):Promise<void>{
    try{applyBoot(await host.boot(),explicit);}catch(error){console.error(error);v.error='Couldn’t open the file. Check its folder permissions and retry.';render();}
  }
  async function keepMine():Promise<void>{
    try{v.disk=await host.readDisk();v.conflict=false;v.savedRevision=-1;await flush();if(v.lastFocus)pendingFocus.current=v.lastFocus;render();}
    catch(error){console.error(error);v.error='Couldn’t read the file. Check its folder permissions and retry.';render();}
  }
  async function requestClose():Promise<void>{if(!v.loaded||await flush())await host.close();}
  function add(parentId?:number):void{
    const b=v.data.board;if(!b)return;const p=parentId===undefined?-1:index(b,parentId);
    const task=blank(b,p<0?0:b.tasks[p].depth+1,p<0?b.tasks[0]?.color??null:b.tasks[p].color);
    if(p>=0)v.folded.delete(foldKey(b,p));
    change(data=>{data.board!.tasks.splice(p<0?0:p+1,0,task);completedLast(data.board!);},{id:String(task.id),start:0,end:0,scroll:0,page:main.current?.scrollTop??0});
  }
  function taskChange(id:number,fn:(task:Task,board:Board)=>void):void{change(data=>{const b=data.board!;const i=index(b,id);if(i<0)return;fn(b.tasks[i],b);completedLast(b);});}
  function remove(id:number):void{
    const b=v.data.board!;const i=index(b,id);if(i<0)return;
    const previous=b.tasks[i-1];change(data=>{const b=data.board!,i=index(b,id);b.tasks.splice(i,end(b,i)-i);},previous?{id:String(previous.id),start:previous.text.length,end:previous.text.length,scroll:0,page:main.current?.scrollTop??0}:null);
  }
  function fold(id:number):void{const b=v.data.board!,i=index(b,id),key=foldKey(b,i);if(!v.folded.delete(key))v.folded.add(key);void host.saveView([...v.folded]);render();}
  function togglePin():void{v.pinned=!v.pinned;void host.pin(v.pinned);render();}

  useEffect(()=>{
    let disposed=false;let off=()=>{};let offTheme=()=>{};
    void load();
    void host.onClose(()=>void requestClose()).then(f=>{if(disposed)f();else off=f;});
    void host.onTheme(dark=>document.documentElement.classList.toggle('dark',dark)).then(f=>{if(disposed)f();else offTheme=f;});
    const tick=setInterval(()=>{
      if(!v.loaded)return;
      const day=localDay();if(day!==v.today){v.today=day;const next=normalize(v.data,day);if(!same(next,v.data)){v.data=next;v.revision++;schedule();render();}}
      if(v.saving||v.conflict)return;
      void host.readDisk().then(disk=>{
        if(disposed||v.saving||disk===v.disk)return;
        if(v.savedRevision!==v.revision || (document.activeElement instanceof HTMLElement && !!document.activeElement.dataset.editor)){v.conflict=true;render();}
        else void load();
      }).catch(error=>{console.error(error);});
    },700);
    const keys=(e:KeyboardEvent)=>{
      const action=shortcut(e);if(!action)return;
      const target=e.target as HTMLElement|null;
      if((action==='undo'||action==='redo')&&target?.closest('[data-native-history]'))return;
      if(action==='new'&&v.notes)return;
      e.preventDefault();e.stopPropagation();
      if(action==='undo')undo();else if(action==='redo')undo(true);else if(action==='save')void flush(true);else add();
    };
    document.addEventListener('keydown',keys,true);
    return()=>{disposed=true;off();offTheme();clearInterval(tick);document.removeEventListener('keydown',keys,true);if(saveTimer.current)clearTimeout(saveTimer.current);if(statusTimer.current)clearTimeout(statusTimer.current);};
  },[]);

  function editorProps(id:string):Pick<ComponentProps<'textarea'>,'onFocus'|'onBlur'|'onSelect'|'onBeforeInput'|'onPaste'|'onCut'> {
    const before=()=>{v.before=snapshot();};
    const remember=()=>{v.lastFocus=captureFocus();};
    return {onFocus:remember,onBlur:remember,onSelect:remember,onBeforeInput:before,onPaste:before,onCut:before};
  }
  function keyTask(e:React.KeyboardEvent<HTMLTextAreaElement>,task:Task):void{
    const mod=e.ctrlKey||e.metaKey,b=v.data.board!,field=e.currentTarget;const i=index(b,task.id);
    const code=e.code;
    if(mod&&code==='KeyT'){e.preventDefault();add(task.id);}
    else if(mod&&code==='Period'){e.preventDefault();fold(task.id);}
    else if(mod&&code==='KeyD'){e.preventDefault();document.querySelector<HTMLButtonElement>(`[data-options="${task.id}"]`)?.click();}
    else if(mod&&e.shiftKey&&e.key==='Backspace'){e.preventDefault();remove(task.id);}
    else if(mod&&e.key==='Enter'){e.preventDefault();change(data=>toggleDone(data.board!,task.id));}
    else if(e.key==='Enter'&&!e.shiftKey){
      e.preventDefault();const n=blank(b,task.depth,task.color,task.text.slice(field.selectionEnd));
      change(data=>{const b=data.board!,i=index(b,task.id);b.tasks[i].text=task.text.slice(0,field.selectionStart);b.tasks.splice(end(b,i),0,n);completedLast(b);},{id:String(n.id),start:0,end:0,scroll:0,page:main.current?.scrollTop??0});
    }else if(e.key==='Tab'&&!mod){e.preventDefault();change(data=>indent(data.board!,task.id,e.shiftKey));}
    else if(e.key==='Backspace'&&!task.text&&end(b,i)===i+1){e.preventDefault();remove(task.id);}
    else if(e.altKey&&(e.key==='ArrowUp'||e.key==='ArrowDown')){
      e.preventDefault();const s=siblings(b,i),j=s.indexOf(i),target=s[j+(e.key==='ArrowUp'?-1:1)];if(target!==undefined)change(data=>moveBranch(data.board!,task.id,b.tasks[target].id,e.key==='ArrowDown'));
    }
  }
  function copy(cut=false):void{
    const f=v.lastFocus;if(!f)return;const field=editors.current.get(f.id);if(!field)return;
    const text=field.value.slice(f.start,f.end);if(!text)return;
    void host.writeClipboard(text).then(()=>{if(cut)replaceSelection('',f);});
  }
  function replaceSelection(text:string,f=v.lastFocus):void{
    if(!f)return;const field=editors.current.get(f.id);if(!field)return;
    const value=field.value.slice(0,f.start)+text+field.value.slice(f.end);
    change(data=>{if(f.id==='notes')data.text=value;else{const t=data.board!.tasks.find(t=>String(t.id)===f.id);if(t)t.text=value.replace(/[\r\n]/g,' ');}},{...f,start:f.start+text.length,end:f.start+text.length});
  }
  function selectAll():void{const f=v.lastFocus;if(!f)return;const field=editors.current.get(f.id);if(field){pendingFocus.current={...f,start:0,end:field.value.length};render();}}

  const b=v.data.board;
  const rows:Task[]=[];let hidden:number|null=null;
  if(b)for(let i=0;i<b.tasks.length;i++){const t=b.tasks[i];if(hidden!==null&&t.depth>hidden)continue;hidden=null;rows.push(t);if(v.folded.has(foldKey(b,i)))hidden=t.depth;}
  return <TooltipProvider delay={300}><div className="app border border-border bg-background text-foreground">
    <header className="flex h-12 shrink-0 items-center gap-0.5 border-b border-border px-3 select-none" onPointerDown={e=>{if(e.button===0&&!(e.target as HTMLElement).closest('[data-no-drag]'))void host.drag();}} onDoubleClick={e=>{if(!(e.target as HTMLElement).closest('[data-no-drag]'))void host.maximize();}}>
      <span className="mr-2 min-w-0 flex-1 text-[13px] font-semibold tracking-tight">{v.notes?'Notes':'Pin'}</span>
      {!v.notes&&<IconButton label="New task (Ctrl+N)" onClick={()=>add()}><Plus/></IconButton>}
      <IconButton label="Undo (Ctrl+Z)" disabled={!v.history.undoStack.length} onClick={()=>undo()}><Undo2/></IconButton>
      <IconButton label="Redo (Ctrl+Shift+Z)" disabled={!v.history.redoStack.length} onClick={()=>undo(true)}><Redo2/></IconButton>
      <IconButton label={v.status==='saved'?'Saved':'Save (Ctrl+S)'} onClick={()=>void flush(true)} disabled={!v.loaded}>{v.status==='saved'?<Check className="text-emerald-600 dark:text-emerald-400"/>:<Save/>}</IconButton>
      <Separator orientation="vertical" className="mx-1 h-4!"/>
      <IconButton label={v.pinned?'Unpin from top':'Keep on top'} aria-pressed={v.pinned} className={v.pinned?'text-foreground':'text-muted-foreground'} onClick={togglePin}><Pin className={v.pinned?'fill-current':''}/></IconButton>
      <IconButton label="Minimize" onClick={()=>void host.minimize()}><Minus/></IconButton>
      <IconButton label="Close" onClick={()=>void requestClose()}><X/></IconButton>
    </header>
    {(v.conflict||v.error)&&<div role="alert" className="border-b border-border bg-muted/50 px-4 py-3 text-xs leading-relaxed"><p>{v.conflict?'This file changed elsewhere. Choose which version to keep.':v.error}</p><div className="mt-2 flex gap-2">{v.conflict?<><Button size="xs" variant="outline" onClick={()=>void load(true)}>Use disk</Button><Button size="xs" onClick={()=>void keepMine()}>Keep mine</Button></>:<Button size="xs" variant="outline" onClick={()=>void(v.loaded?flush(true):load())}>Retry</Button>}</div></div>}
    <ContextMenu><ContextMenuTrigger render={<main ref={main} className={`min-h-0 flex-1 overflow-auto ${v.notes?'flex':'px-3 py-3'}`}/> }>
      {!v.loaded?<div className="p-5 text-sm text-muted-foreground">Opening…</div>:v.notes?<Textarea aria-label="Notes" data-editor="notes" placeholder="Write a note…" className="h-full min-h-0 resize-none rounded-none border-0 bg-transparent! px-5 py-4 text-[15px]! leading-7 shadow-none focus-visible:ring-0" value={v.data.text} ref={field=>{if(field)editors.current.set('notes',field);else editors.current.delete('notes');}} {...editorProps('notes')} onChange={e=>{const value=e.currentTarget.value;change(data=>{data.text=value;},null,'notes');}}/>:
      rows.length?rows.map(task=><TaskRow key={task.id} task={task} board={b!} colors={v.colors} folded={v.folded.has(foldKey(b!,index(b!,task.id)))} hasChildren={end(b!,index(b!,task.id))>index(b!,task.id)+1} register={field=>{if(field)editors.current.set(String(task.id),field);else editors.current.delete(String(task.id));}} editorProps={editorProps(String(task.id))} onText={text=>change(data=>{const t=data.board!.tasks.find(t=>t.id===task.id);if(t)t.text=text.replace(/[\r\n]/g,' ');},null,String(task.id))} onKey={e=>keyTask(e,task)} onToggle={()=>change(data=>toggleDone(data.board!,task.id))} onFold={()=>fold(task.id)} onSub={()=>add(task.id)} onRemove={()=>remove(task.id)} onProperty={fn=>taskChange(task.id,fn)} onDrop={(id,after)=>change(data=>moveBranch(data.board!,id,task.id,after))}/>):<div className="px-2 py-6 text-center"><p className="mb-3 text-sm text-muted-foreground">Keep your next priority here.</p><Button variant="outline" size="sm" onClick={()=>add()}><Plus/>New task</Button></div>}
    </ContextMenuTrigger><ContextMenuContent className="w-44"><ContextMenuItem disabled={!v.history.undoStack.length} onClick={()=>undo()}><Undo2/>Undo<span className="ml-auto text-xs text-muted-foreground">Ctrl Z</span></ContextMenuItem><ContextMenuItem disabled={!v.history.redoStack.length} onClick={()=>undo(true)}><Redo2/>Redo</ContextMenuItem><ContextMenuSeparator/><ContextMenuItem onClick={()=>copy()}><Copy/>Copy</ContextMenuItem><ContextMenuItem onClick={()=>copy(true)}><Scissors/>Cut</ContextMenuItem><ContextMenuItem onClick={()=>void host.readClipboard().then(text=>replaceSelection(text))}><ClipboardPaste/>Paste</ContextMenuItem><ContextMenuItem onClick={selectAll}><TextSelect/>Select all</ContextMenuItem></ContextMenuContent></ContextMenu>
    {(['North','NorthEast','East','SouthEast','South','SouthWest','West','NorthWest'] as const).map(direction=><div key={direction} className={`resize-edge ${direction}`} onPointerDown={e=>{if(e.button===0){e.preventDefault();void host.resize(direction);}}}/>)}
  </div></TooltipProvider>;
}

type RowProps={task:Task;board:Board;colors:[string,string][];folded:boolean;hasChildren:boolean;register:(field:HTMLTextAreaElement|null)=>void;editorProps:ComponentProps<'textarea'>;onText:(value:string)=>void;onKey:(event:React.KeyboardEvent<HTMLTextAreaElement>)=>void;onToggle:()=>void;onFold:()=>void;onSub:()=>void;onRemove:()=>void;onProperty:(fn:(task:Task,board:Board)=>void)=>void;onDrop:(id:number,after:boolean)=>void};
function TaskRow(p:RowProps){
  const field=useRef<HTMLTextAreaElement>(null);const width=useRef(0);const [drop,setDrop]=useState<'before'|'after'|null>(null);const [open,setOpen]=useState(false);
  function size(){const el=field.current;if(!el)return;el.style.height='0px';el.style.height=`${el.scrollHeight}px`;}
  useLayoutEffect(size,[p.task.text]);
  useEffect(()=>{const el=field.current;if(!el)return;const observer=new ResizeObserver(entries=>{const w=entries[0].contentRect.width;if(w!==width.current){width.current=w;size();}});observer.observe(el);return()=>observer.disconnect();},[]);
  const t=p.task,color=p.colors.find(([id])=>id===t.color)?.[1]??t.color;
  const date=t.date?formatDate(t.date):null;
  const day=t.date?.slice(0,10),today=localDay();
  return <div className={`task-row group relative mb-1 flex items-start gap-2 rounded-lg px-2 py-2 ${drop==='before'?'drop-before':drop==='after'?'drop-after':''}`} style={{marginInlineStart:`min(${Math.min(t.depth,8)*18}px, max(0px, calc(100% - 240px)))`,backgroundColor:color?`color-mix(in srgb, ${color} 13%, var(--background))`:undefined}} onDragOver={e=>{if(!e.dataTransfer.types.includes('application/x-pin-task'))return;e.preventDefault();setDrop(e.clientY<e.currentTarget.getBoundingClientRect().top+e.currentTarget.clientHeight/2?'before':'after');}} onDragLeave={()=>setDrop(null)} onDrop={e=>{const id=Number(e.dataTransfer.getData('application/x-pin-task'));if(!id)return;e.preventDefault();p.onDrop(id,drop==='after');setDrop(null);}}>
    <Button variant="ghost" size="icon-xs" className="-ml-1 mt-px size-5! shrink-0" aria-label={p.folded?'Show subtasks':'Hide subtasks'} disabled={!p.hasChildren} onClick={p.onFold}>{p.hasChildren?(p.folded?<ChevronRight/>:<ChevronDown/>):<span className="size-3"/>}</Button>
    <Checkbox className="mt-0.5 shrink-0" aria-label={`Complete ${t.text||'task'}`} checked={t.done} onCheckedChange={p.onToggle}/>
    <Textarea data-editor={String(t.id)} aria-label="Task text" placeholder="New task" rows={1} value={t.text} ref={el=>{field.current=el;p.register(el);}} className={`min-h-0 min-w-0 flex-1 resize-none overflow-hidden rounded-none border-0 bg-transparent! p-0 text-[14px]! leading-6 shadow-none focus-visible:ring-0 ${t.done?'text-muted-foreground line-through decoration-muted-foreground/50':''}`} {...p.editorProps} onChange={e=>p.onText(e.currentTarget.value)} onKeyDown={p.onKey}/>
    <div className="mt-0.5 shrink-0 text-right leading-4">{t.daily&&<Badge variant="secondary" className="mb-0.5 h-4 border-0 bg-background/40 px-1 text-[10px] font-normal"><Repeat2 className="size-2.5!"/>Daily</Badge>}{date&&<div className={`max-w-24 text-[11px] ${day===today?'text-emerald-800 dark:text-emerald-300':day!<today&&!t.done?'text-red-800 dark:text-red-300':'text-muted-foreground'}`}>{date}</div>}</div>
    <span draggable className="mt-1 cursor-grab text-muted-foreground/40 opacity-0 transition-opacity group-hover:opacity-100 group-focus-within:opacity-100" title="Drag to reorder (Alt+↑ / Alt+↓)" onDragStart={e=>{e.dataTransfer.setData('application/x-pin-task',String(t.id));e.dataTransfer.effectAllowed='move';}}><GripVertical className="size-3.5"/></span>
    <Popover open={open} onOpenChange={setOpen}><PopoverTrigger render={<Button variant="ghost" size="icon-xs" data-options={t.id} aria-label="Task options" className="mt-px shrink-0 text-muted-foreground"><MoreHorizontal/></Button>}/><PopoverContent align="end" className="w-64 gap-3 p-3">
      <label className="flex items-center gap-2 text-sm"><Checkbox aria-label="Repeat daily" checked={t.daily} onCheckedChange={checked=>p.onProperty(task=>{task.daily=checked;task.completed=checked&&task.done?localDay():null;})}/><Repeat2 className="size-3.5 text-muted-foreground"/>Repeat daily</label>
      <Separator/>
      <div><label className="mb-1.5 flex items-center gap-1.5 text-xs text-muted-foreground"><CalendarDays className="size-3.5"/>Date and time</label><div className="flex gap-1.5"><Input data-native-history aria-label="Date" type="date" className="min-w-0 flex-1 text-xs" value={t.date?.slice(0,10)??''} onChange={e=>p.onProperty(task=>{task.date=e.target.value?e.target.value+(task.date?.slice(10)??''):null;})}/><Input data-native-history aria-label="Time" type="time" className="w-22 text-xs" value={t.date?.slice(11)??''} onChange={e=>p.onProperty(task=>{if(task.date)task.date=task.date.slice(0,10)+(e.target.value?' '+e.target.value:'');})}/></div>{t.date&&<Button variant="link" size="xs" className="mt-1 h-5 px-0 text-muted-foreground" onClick={()=>p.onProperty(task=>{task.date=null;})}>Clear date</Button>}</div>
      <div><p className="mb-2 text-xs text-muted-foreground">Color</p><div className="grid grid-cols-6 gap-2">{p.colors.map(([id,hex])=><button key={id} className="size-6 rounded-full border border-foreground/10 ring-offset-2 ring-offset-popover outline-none focus-visible:ring-2 focus-visible:ring-ring" aria-label={`Color ${hex}`} aria-pressed={t.color===id} style={{backgroundColor:hex,boxShadow:t.color===id?'0 0 0 2px var(--popover), 0 0 0 3px var(--foreground)':undefined}} onClick={()=>p.onProperty(task=>{task.color=id;})}/>)}</div><div className="mt-2 flex gap-2"><Button variant="outline" size="xs" onClick={()=>p.onProperty(task=>{task.color=null;})}>No color</Button><label className="relative inline-flex h-6 cursor-pointer items-center gap-1 rounded-md border border-input px-2 text-xs"><Palette className="size-3"/>Custom<input aria-label="Custom color" type="color" className="absolute inset-0 size-full cursor-pointer opacity-0" value={color??'#ec4899'} onChange={e=>p.onProperty(task=>{task.color=e.target.value;})}/></label></div></div>
      <Separator/><div className="flex gap-1"><Button variant="ghost" size="xs" onClick={()=>{setOpen(false);p.onSub();}}><ListPlus/>Subtask</Button><Button variant="ghost" size="xs" className="ml-auto text-destructive" onClick={()=>{setOpen(false);p.onRemove();}}><Trash2/>Delete</Button></div>
    </PopoverContent></Popover>
  </div>;
}
function formatDate(value:string):string{const [day,time]=value.split(' ');const d=new Date(`${day}T${time??'00:00'}`);if(Number.isNaN(d.getTime()))return value;return new Intl.DateTimeFormat('en-GB',{weekday:'short',day:'numeric',month:'short'}).format(d)+(time?`, ${time}`:'');}
