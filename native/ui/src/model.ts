export interface Task { id: number; depth: number; text: string; color: string | null; date: string | null; done: boolean; daily: boolean; completed: string | null }
export interface Board { preamble: string; tasks: Task[] }
export interface Document { text: string; board: Board | null }
export interface Focus { id: string; start: number; end: number; scroll: number; page: number }
export interface Snapshot { data: Document; focus: Focus | null }
export type Disk = string | null;
export interface Boot { notes: boolean; disk: Disk; canonical: string; document: Document; pinned: boolean; folded: string[]; colors: [string, string][]; dark: boolean }
export interface Save { saved: boolean; disk: Disk; content: string }

export function localDay(): string { const d = new Date(); return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2,'0')}-${String(d.getDate()).padStart(2,'0')}`; }
export function index(b: Board, id: number): number { return b.tasks.findIndex(t => t.id === id); }
export function end(b: Board, i: number): number { const j = b.tasks.findIndex((t,j) => j > i && t.depth <= b.tasks[i].depth); return j < 0 ? b.tasks.length : j; }
export function parent(b: Board, i: number): number { for (let j=i-1;j>=0;j--) if (b.tasks[j].depth < b.tasks[i].depth) return j; return -1; }
export function siblings(b: Board, i: number): number[] { const p = parent(b,i); const stop = p < 0 ? b.tasks.length : end(b,p); return b.tasks.flatMap((t,j) => j > p && j < stop && t.depth === b.tasks[i].depth ? [j] : []); }
export function completedLast(b: Board): void {
  function sort(tasks: Task[]): Task[] {
    const groups: Task[][]=[];
    for (const t of tasks) { if (!groups.length || t.depth === tasks[0].depth) groups.push([]); groups.at(-1)!.push(t); }
    groups.sort((a,b) => Number(a[0].done)-Number(b[0].done));
    return groups.flatMap(g=>[g[0],...sort(g.slice(1))]);
  }
  b.tasks=sort(b.tasks);
}
export function normalize(data: Document, today = localDay()): Document {
  const next = structuredClone(data);
  if (next.board) {
    for (const t of next.board.tasks) {
      if (t.daily && t.done && t.completed && t.completed < today) {
        t.done=false;t.completed=null;
        if (t.date && t.date.slice(0,10)<today) t.date=today+t.date.slice(10);
      } else if(t.daily && t.done && !t.completed) t.completed=today;
    }
    completedLast(next.board);
  }
  return next;
}
export function blank(b: Board, depth=0, color: string|null=null, text=''): Task {
  return {id:Math.max(0,...b.tasks.map(t=>t.id))+1,depth,text,color,date:null,done:false,daily:false,completed:null};
}
export function toggleDone(b: Board,id:number):void {
  const i=index(b,id);if(i<0)return;
  const t=b.tasks[i];t.done=!t.done;t.completed=t.daily&&t.done?localDay():null;
  if(t.done){const stop=end(b,i),p=parent(b,i),dest=p<0?b.tasks.length:end(b,p);const group=b.tasks.splice(i,stop-i);b.tasks.splice(dest-group.length,0,...group);}
  completedLast(b);
}
export function moveBranch(b:Board,id:number,target:number,after:boolean):void {
  const i=index(b,id),j=index(b,target);if(i<0||j<0)return;const stop=end(b,i);if(j>=i&&j<stop)return;
  const delta=b.tasks[j].depth-b.tasks[i].depth;const dest=after?end(b,j):j;
  const group=b.tasks.splice(i,stop-i);for(const t of group)t.depth+=delta;
  b.tasks.splice(dest>=stop?dest-group.length:dest,0,...group);completedLast(b);
}
export function indent(b:Board,id:number,out:boolean):void {
  const i=index(b,id);if(i<0)return;const stop=end(b,i),p=parent(b,i);
  if(out){if(p<0)return;const dest=end(b,p),group=b.tasks.splice(i,stop-i);for(const t of group)t.depth--;b.tasks.splice(dest-group.length,0,...group);}
  else {if(siblings(b,i)[0]===i)return;for(let j=i;j<stop;j++)b.tasks[j].depth++;}
  completedLast(b);
}
export function foldKey(b:Board,i:number):string { const keys=[b.tasks[i].text];for(let p=parent(b,i);p>=0;p=parent(b,p))keys.unshift(b.tasks[p].text);return keys.join('\x1f'); }
export function shortcut(e: Pick<KeyboardEvent,'code'|'key'|'ctrlKey'|'metaKey'|'shiftKey'>):'undo'|'redo'|'save'|'new'|null {
  if(!e.ctrlKey&&!e.metaKey)return null;
  const code=e.code || `Key${e.key.toUpperCase()}`;
  if(code==='KeyZ')return e.shiftKey?'redo':'undo';
  if(code==='KeyY')return 'redo';
  if(code==='KeyS')return 'save';
  if(code==='KeyN')return 'new';
  return null;
}

export class History {
  undoStack: Snapshot[]=[];
  redoStack: Snapshot[]=[];
  private typing: {id:string;at:number}|null=null;
  record(before:Snapshot,id?:string):void {
    const now=performance.now();
    if(!id || !this.typing || this.typing.id!==id || now-this.typing.at>600) {
      this.undoStack.push(structuredClone(before));if(this.undoStack.length>300)this.undoStack.shift();
    }
    this.redoStack=[];this.typing=id?{id,at:now}:null;
  }
  restore(current:Snapshot,redo:boolean):Snapshot|null {
    const next=(redo?this.redoStack:this.undoStack).pop();if(!next)return null;
    (redo?this.undoStack:this.redoStack).push(structuredClone(current));this.typing=null;return next;
  }
  clear():void { this.undoStack=[];this.redoStack=[];this.typing=null; }
}
