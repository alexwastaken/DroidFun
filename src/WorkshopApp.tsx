import {useEffect,useState} from 'react';
import {invoke} from '@tauri-apps/api/core';
import {listen} from '@tauri-apps/api/event';
import Studio from './Studio';
import Workshop from './Workshop';
import WorkshopLive from './WorkshopLive';
import './workshop.css';
import './workshop-app.css';
type Section='screens'|'prepare'|'list'|'live';
export default function WorkshopApp(){
 const [section,setSection]=useState<Section>('screens'),[changing,setChanging]=useState(false),[error,setError]=useState('');
 const [working,setWorking]=useState(false),[clean,setClean]=useState(false);
 useEffect(()=>{let disposed=false,off:(()=>void)|undefined;const update=(s:{busy:boolean;phase:string})=>setWorking(s.busy||!['idle','error','complete'].includes(s.phase));
 void listen<{busy:boolean;phase:string}>('workshop-progress',e=>{if(!disposed)update(e.payload);}).then(f=>{if(disposed)f();else off=f;});
 void invoke<{busy:boolean;phase:string}>('workshop_status').then(s=>{if(!disposed)update(s);}).catch(e=>setError(String(e)));
 return()=>{disposed=true;off?.();};},[]);
 async function change(next:Section){if(next===section||changing)return;setChanging(true);setError('');try{if(section==='live')await invoke('workshop_live_action',{action:'stop',value:null});await invoke('showcase_mode',{screens:next==='screens'});setSection(next);}catch(e){setError(String(e));}finally{setChanging(false);}}
 return <div className={`workshop-app ${clean?'is-clean':''}`}>
  {!clean&&<><header className="workshop-app-header"><div className="topline"><span className="brand-dot"/><strong>droidfun</strong><span className="workshop-app-label">Workshop</span></div><span className="workshop-version">0.4.1</span></header>
  <nav className="workshop-app-nav" aria-label="App sections">{(['screens','prepare','list','live'] as const).map(id=><button key={id} disabled={changing||(id==='screens'&&working)} aria-pressed={id===section} onClick={()=>void change(id)}>{id==='screens'?'Screens':id==='prepare'?'Prepare a droid':id==='list'?'Droid list':'Live text'}</button>)}</nav>
  {changing&&<p role="status" className="workshop-app-note">{section==='screens'?'Releasing the screen connection…':'Opening screens…'}</p>}
  {error&&<p role="alert" className="workshop-app-error">{error}</p>}
  {working&&section!=='screens'&&<p className="workshop-app-note">Finish this board’s checks, or choose Prepare another board, to return to Screens.</p>}</>}
  {section==='screens'?<Studio onCleanChange={setClean}/>:section==='live'?<WorkshopLive onDroidList={()=>void change('list')}/>:<Workshop section={section}/>}
 </div>;
}
