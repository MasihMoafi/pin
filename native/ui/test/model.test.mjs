import {test} from 'node:test';
import assert from 'node:assert/strict';
import {normalize,toggleDone,moveBranch,indent,History,shortcut} from '../src/model.ts';
const task=(id,text,depth=0,extra={})=>({id,text,depth,color:null,date:null,done:false,daily:false,completed:null,...extra});

test('daily completion survives a restart, reopens once on the next local day, preserves its time',()=>{
  const document={text:'',board:{preamble:'',tasks:[task(1,'Daily',0,{daily:true,done:true,completed:'2026-11-09',date:'2026-11-09 09:30'}),task(2,'One-off',0,{done:true})]}};
  assert.equal(normalize(document,'2026-11-09').board.tasks[0].done,true);
  const next=normalize(structuredClone(document),'2026-11-10');
  assert.deepEqual(next.board.tasks[0],task(1,'Daily',0,{daily:true,date:'2026-11-10 09:30'}));
  assert.equal(next.board.tasks[1].done,true);
  assert.deepEqual(normalize(next,'2026-11-10'),next);
});

test('finishing moves the whole branch to the bottom and undo restores order and status together',()=>{
 const b={preamble:'',tasks:[task(1,'First'),task(2,'Child',1),task(3,'Older done',0,{done:true}),task(4,'Second')]};
 const history=new History();const before={data:{text:'',board:b},focus:{id:'1',start:2,end:2,scroll:0,page:40}};
 history.record(before);toggleDone(b,1);
 assert.deepEqual(b.tasks.map(t=>t.text),['Second','Older done','First','Child']);
 const restored=history.restore({data:{text:'',board:b},focus:null},false);
 assert.deepEqual(restored.data.board.tasks.map(t=>t.text),['First','Child','Older done','Second']);
 assert.equal(restored.data.board.tasks[0].done,false);assert.equal(restored.focus.start,2);assert.equal(restored.focus.page,40);
 const redone=history.restore(restored,true);assert.equal(redone.data.board.tasks[2].done,true);
});

test('outdenting and reordering preserve the branch and dropping onto a descendant cancels',()=>{
 const b={preamble:'',tasks:[task(1,'Parent'),task(2,'Child',1),task(3,'Grandchild',2),task(4,'Other',1),task(5,'Last')]};
 const before=structuredClone(b);moveBranch(b,1,3,true);assert.deepEqual(b,before);
 indent(b,2,true);assert.deepEqual(b.tasks.map(t=>[t.text,t.depth]),[['Parent',0],['Other',1],['Child',0],['Grandchild',1],['Last',0]]);
 moveBranch(b,2,5,true);assert.deepEqual(b.tasks.slice(-2).map(t=>t.text),['Child','Grandchild']);
});

test('standard shortcuts use physical keys with Persian symbols too',()=>{
 const key=(code,key,shiftKey=false)=>({code,key,ctrlKey:true,metaKey:false,shiftKey});
 assert.equal(shortcut(key('KeyZ','ظ')),'undo');assert.equal(shortcut(key('KeyZ','ظ',true)),'redo');
 assert.equal(shortcut(key('KeyY','غ')),'redo');assert.equal(shortcut(key('KeyS','س')),'save');
});
