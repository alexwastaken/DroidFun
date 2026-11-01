import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
type Droid={board_id:string;model:string;firmware:string;wiring:string;state:string;checked_at?:string;pendingSync?:boolean;note?:string};
type Registry={connected:boolean;pending:number;droids:Droid[];next:string|null};

export default function WorkshopRegistry({visible,connectedIds}:{visible:boolean;connectedIds:string[]}){
 const [state,setState]=useState<Registry>({connected:false,pending:0,droids:[],next:null});
 const [code,setCode]=useState(""),[query,setQuery]=useState(""),[error,setError]=useState(""),[busy,setBusy]=useState(false),[connect,setConnect]=useState(false);
 const active=useRef(false),mounted=useRef(true),currentQuery=useRef("");currentQuery.current=query;
 const refresh=useRef<()=>Promise<void>>(async()=>{});
 async function sync(after:string|null=null){
  if(active.current)return;active.current=true;setBusy(true);setError("");
  try{const next=await invoke<Registry>("workshop_registry_sync",{query:currentQuery.current,after});if(mounted.current)setState(next);}
  catch(e){if(mounted.current){setError(String(e));try{setState(await invoke<Registry>("workshop_registry_state"));}catch{/* Keep cached rows. */}}}
  finally{active.current=false;if(mounted.current)setBusy(false);}
 }
 refresh.current=async()=>{try{const next=await invoke<Registry>("workshop_registry_state");if(!mounted.current)return;setState(next);if(next.connected)await sync();}catch(e){if(mounted.current)setError(String(e));}};
 useEffect(()=>{
  mounted.current=true;const handlers:(()=>void)[]=[];void refresh.current();
  void listen("workshop-registry-changed",()=>void refresh.current()).then(f=>{if(mounted.current)handlers.push(f);else f();});
  void listen<string>("workshop-registry-error",event=>{if(mounted.current)setError(event.payload);}).then(f=>{if(mounted.current)handlers.push(f);else f();});
  const timer=window.setInterval(()=>void refresh.current(),60000);
  return()=>{mounted.current=false;handlers.forEach(f=>f());window.clearInterval(timer);};
 },[]);
 async function pair(){
  if(active.current)return;active.current=true;setBusy(true);setError("");const value=code.trim();setCode("");
  try{setState(await invoke<Registry>("workshop_registry_connect",{code:value}));setConnect(false);}catch(e){setError(String(e));}
  finally{active.current=false;setBusy(false);}await refresh.current();
 }
 const connected=new Set(connectedIds.map(id=>id.toUpperCase()));
 const rows=state.droids.filter(d=>`${d.board_id} ${d.model}`.toLowerCase().includes(query.toLowerCase()));
 return <section className="workshop-card workshop-registry" hidden={!visible} aria-label="Droid list">
  <div className="workshop-section-heading"><h2>Your droids</h2><button className="workshop-secondary" disabled={busy||!state.connected} onClick={()=>void sync()}>{busy?"Syncing…":"Sync"}</button></div>
  <p>{state.connected?"Connected to your private inventory.":"Your local droid list. Connect your dashboard to sync it."} {state.pending>0?`${state.pending} waiting to sync.`:""}</p>
  <p>Connected here means plugged into this computer. Saved records show Workshop checks, not whether a customer is online.</p>
  <form onSubmit={e=>{e.preventDefault();if(state.connected)void sync();}}><label className="workshop-field">Find an ID or model<input value={query} onChange={e=>setQuery(e.target.value)} maxLength={80} placeholder="Board ID, Nimbus, Boo…"/></label>{state.connected&&<button className="workshop-secondary" disabled={busy}>Search inventory</button>}</form>
  {error&&<p className="workshop-warning" role="alert">{error}</p>}
  <ul className="workshop-droid-list">{rows.map(d=><li key={d.board_id}><div><strong>{d.model[0]?.toUpperCase()+d.model.slice(1)}</strong><span className={connected.has(d.board_id)?"registry-connected":""}>{connected.has(d.board_id)?"Connected here":d.state==="prepared"?"Prepared":d.state==="retired"?"Retired":"Needs checks"}</span></div><code>{d.board_id}</code><p>Firmware {d.firmware||"not recorded"} · {d.wiring||"unrecorded"} wiring{d.pendingSync?" · Waiting to sync":""}</p>{d.note&&<p>{d.note}</p>}</li>)}</ul>
  {!rows.length&&<p>{query?"No matching droids in this page. Search inventory to check saved records.":"Inspect a droid to add it here. For an older droid, choose its model and use Record existing droid."}</p>}
  {state.next&&<button className="workshop-secondary" disabled={busy} onClick={()=>void sync(state.next)}>Next 100</button>}
  <details open={connect||!state.connected} className="workshop-registry-connect"><summary onClick={e=>{e.preventDefault();setConnect(!connect);}}>{state.connected?"Manage dashboard connection":"Connect your dashboard"}</summary>
   <p>Open your owner dashboard, sign in, choose Droids → Create connection code, then paste the code below. Your owner password stays on the website.</p>
   <button className="workshop-secondary" onClick={()=>void invoke("workshop_registry_open_dashboard").catch(e=>setError(String(e)))}>Open owner dashboard</button>
   <form onSubmit={e=>{e.preventDefault();void pair();}}><label className="workshop-field">One-use connection code<input value={code} onChange={e=>setCode(e.target.value)} autoComplete="off" spellCheck={false} type="password" maxLength={64}/></label><button className="workshop-primary" disabled={busy||code.trim().length!==64}>Connect Workshop</button></form>
   {state.connected&&<button className="workshop-secondary" disabled={busy} onClick={async()=>{setBusy(true);try{setState(await invoke<Registry>("workshop_registry_disconnect"));}catch(e){setError(String(e));}finally{setBusy(false);}}}>Disconnect this Workshop</button>}
   <p>Your connection is private to this Workshop installation. You can revoke it from the dashboard. Local preparation works without it.</p>
  </details>
 </section>;
}
