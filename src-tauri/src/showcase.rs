//! Private photography app. All remote readings are generated locally; no account APIs run.
use crate::{connection::{self, Link, RobotPort}, core::{self, DataCard, Engine, Playback, Settings, SourceConfig, Widget}, local_widgets};
use serde::Serialize;
use serde_json::{json, Value};
use std::{sync::Mutex, time::{Duration, Instant}};
use tauri::{Emitter, Manager, State};

struct Studio { active: bool, released: bool, engine: Engine, ports: Vec<RobotPort>, target: Option<String>, connection: String, moving: bool, started: Instant, codex: f64, claude: f64, path: std::path::PathBuf }
struct StudioState(Mutex<Studio>);
#[derive(Clone, Serialize)]
#[serde(rename_all="camelCase")]
struct Screen { widget: Widget, frame: Vec<u8>, reading: Option<DataCard> }
#[derive(Clone, Serialize)]
#[serde(rename_all="camelCase")]
struct View { screens: Vec<Screen>, selected: String, frame: Vec<u8>, brightness: u8, orientation: u16, cycling: bool, moving: bool, ports: Vec<RobotPort>, target: Option<String>, connection: String }
fn defaults() -> Settings {
    let mut engine = Engine::new(Settings::default(), Instant::now());
    for kind in ["text","image","clock","pomodoro","meeting","cpu","memory"] { engine.add_widget(kind).expect("known screen"); }
    for w in &mut engine.settings.widgets {
        w.id = w.kind.clone(); w.enabled = true; w.duration_sec = 8; w.config.mode = "live".into();
        if w.kind == "weather" { w.config.location = Some(core::Location { name:"San Diego".into(),latitude:32.72,longitude:-117.16,country:"US".into(),region:"CA".into() }); }
        w.footer.clear();
        w.config.symbol = "AAPL".into(); w.config.custom_text = "Hello, human!".into();
        w.config.timer_running = true; w.config.timer_started_at = core::unix_now();
        w.config.meeting_starts_at = core::unix_now() + 900;
        w.config.clock_seconds = true;
        if w.kind == "image" { w.config.image_pixels = core::render(None,true).iter().map(|v|format!("{v:02x}")).collect(); }
    }
    engine.settings.preferences.setup_complete = true;
    let mut settings=engine.settings;crate::workshop_clock::add_calendar(&mut settings);settings
}
fn card(kind: &str, headline: String, footer: String, values: Value) -> DataCard {
    let now = core::unix_now();
    DataCard { source_id:kind.into(), scope_id:"studio".into(), observed_at:now, fetched_at:now, stale_at:now+3600, expires_at:now+86400, state:"fresh".into(), headline, footer, detail:String::new(), values }
}
impl Studio {
    fn refresh(&mut self) {
        let phase = if self.moving { self.started.elapsed().as_secs_f64()/9.0 } else { 0.0 };
        for w in self.engine.settings.widgets.clone() {
            let d = match w.kind.as_str() {
                "codex"|"claude" => { let base = if w.kind=="codex" {self.codex} else {self.claude}; let left=(base+phase.sin()*2.0).clamp(0.0,100.0);
                    card(&w.kind,format!("{left:.0}%"), if w.config.window=="secondary"||w.config.window=="seven_day" {"7D ALLOWANCE"}else{"5H ALLOWANCE"}.into(),json!({"progress":left,"remainingPercent":left,"usedPercent":100.0-left,"context":"ALLOWANCE LEFT"})) },
                "weather" => { let t=if w.config.unit=="celsius" {22.0}else{72.0};card("weather",format!("{:.0} {}",t+phase.sin(),if w.config.unit=="celsius" {"C"}else{"F"}),if w.config.weather_layout=="high_low" {"H75 L62"}else{"PARTLY CLOUDY"}.into(),json!({"location":"SAN DIEGO","temperature":t,"weatherCode":2})) },
                "stock" => card("stock",format!("{:.2}",214.36+phase.sin()*1.7),format!("USD {:+.2}%",1.24+phase.cos()*0.2),json!({"symbol":w.config.symbol,"changePercent":1.24})),
                "cpu" => local_widgets::cpu_card(&w,core::unix_now(),(28.0+phase.sin()*12.0) as f32,&[12.0,24.0,18.0,35.0,21.0,28.0]),
                "memory" => local_widgets::memory_card(&w,core::unix_now(),10_522_669_875,6_657_199_309,17_179_869_184),
                _=>continue,
            };
            self.engine.set_data(&w.id,d);
        }
        self.engine.tick(Instant::now());
    }
    fn view(&self) -> View {
        let snapshot=self.engine.snapshot();
        View { screens:self.engine.settings.widgets.iter().map(|w|Screen{widget:w.clone(),frame:self.engine.render_widget(&w.id),reading:snapshot.data.get(&w.id).cloned()}).collect(),selected:snapshot.current_id,frame:snapshot.frame,brightness:self.engine.settings.brightness,orientation:self.engine.settings.preferences.orientation,cycling:matches!(snapshot.playback,Playback::Running),moving:self.moving,ports:self.ports.clone(),target:self.target.clone(),connection:self.connection.clone() }
    }
}
#[tauri::command]
fn showcase_state(state: State<StudioState>) -> View { state.0.lock().unwrap().view() }
#[tauri::command]
fn showcase_action(action:String,id:Option<String>,value:Option<Value>,state:State<StudioState>) -> Result<View,String> {
    let mut s=state.0.lock().unwrap();if !s.active{return Err("Open Screens before using its controls.".into());}let v=value.unwrap_or(Value::Null);let mut save=false;
    match action.as_str() {
        "select"=>{s.engine.pin(id.as_deref().ok_or("Choose a screen")?)?;save=true;},
        "cycle"=>{s.engine.toggle_pause();},
        "enabled"|"duration"|"move"=>{playlist_action(&mut s.engine,&action,id.as_deref().ok_or("Choose a screen")?,&v)?;save=true;},
        "values"=>{s.moving=!s.moving;},
        "connect"=>{let serial=id.ok_or("Choose a USB droid")?;if !s.ports.iter().any(|p|p.serial==serial){return Err("That droid is no longer plugged in".into());}s.target=Some(serial);},
        "disconnect"=>{s.target=None;s.connection="Not connected".into();},
        "brightness"=>{s.engine.set_brightness(v.as_u64().filter(|x|*x<=100).ok_or("Choose 0–100")? as u8)?;save=true;},
        "orientation"=>{let x=v.as_u64().filter(|x|[0,180].contains(x)).ok_or("Choose 0 or 180 degrees")?;s.engine.settings.preferences.orientation=x as u16;save=true;},
        "level"=>{let n=v.as_f64().filter(|x|x.is_finite()&&(0.0..=100.0).contains(x)).ok_or("Choose 0–100")?;match id.as_deref(){Some("codex")=>s.codex=n,Some("claude")=>s.claude=n,_=>return Err("Choose Codex or Claude".into())}},
        "configure"=>{let label=v["label"].as_str().ok_or("Add a screen name")?.to_string();let mut config:SourceConfig=serde_json::from_value(v["config"].clone()).map_err(|_|"Invalid screen settings")?;config.mode="live".into();config.cli_path.clear();config.session_id.clear();config.claude_profile.clear();config.claude_report_directory.clear();s.engine.configure_source(id.as_deref().ok_or("Choose a screen")?,label,config)?;save=true;},
        "timer"=>{s.engine.timer_action_at(id.as_deref().ok_or("Choose a timer")?,v.as_str().ok_or("Choose a timer action")?,core::unix_now())?;save=true;},
        _=>return Err("Unknown control".into()),
    }
    s.refresh();if save {crate::persistence::save(&s.path,&s.engine.settings)?;}Ok(s.view())
}
fn playlist_action(engine: &mut Engine, action: &str, id: &str, value: &Value) -> Result<(),String> {
    match action {
        "enabled" => {
            let enabled=value.as_bool().ok_or("Choose on or off")?;
            if !enabled && engine.settings.widgets.iter().filter(|w|w.enabled).count()==1 && engine.settings.widgets.iter().any(|w|w.id==id&&w.enabled) {return Err("Keep at least one screen on in the playlist.".into());}
            engine.set_enabled(id,enabled)
        },
        "duration" => engine.set_duration(id,value.as_u64().filter(|n|(3..=120).contains(n)).ok_or("Choose 3 to 120 seconds")? as u16),
        "move" => engine.move_widget(id,value.as_i64().filter(|n|[-1,1].contains(n)).ok_or("Choose up or down")? as i8),
        _ => Err("Unknown playlist control".into()),
    }
}
pub(crate) fn require_board_mode(app: &tauri::AppHandle) -> Result<(), String> {
    let state=app.state::<StudioState>();
    let s=state.0.lock().map_err(|_|"Screen controls are unavailable")?;
    if s.active || !s.released { return Err("Open Prepare a droid and wait for the screen connection to close.".into()); }
    Ok(())
}
#[tauri::command]
async fn showcase_mode(screens: bool, app: tauri::AppHandle) -> Result<(),String> {
    tauri::async_runtime::spawn_blocking(move || {
        if screens {
            let _usb=crate::UsbOperationGuard::acquire()?;
            if !crate::workshop::can_show_screens(&app) { return Err("Finish this board’s checks or choose Prepare another board before opening Screens.".into()); }
            let state=app.state::<StudioState>();let mut s=state.0.lock().map_err(|_|"Screen controls are unavailable")?;
            s.active=true;s.released=false;
        } else {
            app.state::<StudioState>().0.lock().map_err(|_|"Screen controls are unavailable")?.active=false;
            let until=Instant::now()+Duration::from_secs(12);
            loop {
                if app.state::<StudioState>().0.lock().map_err(|_|"Screen controls are unavailable")?.released {break;}
                if Instant::now()>=until {return Err("USB is still closing. Keep the app open and try Prepare a droid again.".into());}
                std::thread::sleep(Duration::from_millis(50));
            }
        }
        Ok(())
    }).await.map_err(|_|"Could not switch tools".to_string())?
}
#[cfg(not(test))]
pub fn run() {
    tauri::Builder::default().plugin(tauri_plugin_single_instance::init(|app,_,_|{if let Some(w)=app.get_webview_window("main"){let _=w.show();let _=w.set_focus();}}))
    .invoke_handler(tauri::generate_handler![showcase_state,showcase_action,showcase_mode,
        crate::workshop::workshop_status,crate::workshop::workshop_close_consumer,
        crate::workshop::workshop_scan,crate::workshop::workshop_inspect,
        crate::workshop::workshop_program,crate::workshop::workshop_wipe_reinstall,
        crate::workshop::workshop_verify_installed,crate::workshop::workshop_confirm_unplugged,
        crate::workshop::workshop_verify_reconnect,crate::workshop::workshop_confirm_screen,
        crate::workshop::workshop_next,crate::workshop::workshop_record_existing,
        crate::workshop_registry::workshop_registry_state,crate::workshop_registry::workshop_registry_open_dashboard,
        crate::workshop_registry::workshop_registry_connect,crate::workshop_registry::workshop_registry_sync,
        crate::workshop_registry::workshop_registry_disconnect,
        crate::workshop_live::workshop_live_state,crate::workshop_live::workshop_live_action,
        crate::app_update::get_app_update_status,crate::app_update::check_app_update,
        crate::app_update::install_app_update,crate::app_update::restart_app_after_update])
    .on_window_event(|window,event|{
        if let tauri::WindowEvent::CloseRequested{api,..}=event {
            if crate::workshop::busy(window.app_handle())||crate::app_update::installing(window.app_handle()){api.prevent_close();}
        }
    })
    .setup(|app|{
        crate::workshop::initialize(app)?;
        crate::workshop_live::initialize(app);
        crate::app_update::initialize(app.handle());
        let path=app.path().app_config_dir()?.join("studio-settings.json");
        // Import only local screen preferences from the previous photo app.
        // Workshop's existing identifier preserves its registry and pairing.
        let previous=app.path().app_config_dir()?.parent().map(|p|p.join("com.droidfun.studio/studio-settings.json"));
        let saved=std::fs::read(&path).ok().or_else(||previous.and_then(|p|std::fs::read(p).ok()));
        let mut settings=saved.and_then(|b|serde_json::from_slice::<Settings>(&b).ok()).filter(|s|s.validate().is_ok()&&s.preferences.bot_model.is_empty()).unwrap_or_else(defaults);
        crate::workshop_clock::add_calendar(&mut settings);
        let mut studio=Studio{active:true,released:false,engine:Engine::new(settings,Instant::now()),ports:Vec::new(),target:None,connection:"Preview only".into(),moving:true,started:Instant::now(),codex:78.0,claude:64.0,path};studio.refresh();app.manage(StudioState(Mutex::new(studio)));
        tauri::WebviewWindowBuilder::new(app,"main",tauri::WebviewUrl::App("index.html".into())).title("DroidFun Workshop").inner_size(1040.0,820.0).min_inner_size(720.0,580.0).on_navigation(|url|crate::web_security::local_navigation(url,false)).on_new_window(|_,_|tauri::webview::NewWindowResponse::Deny).build()?;
        let app=app.handle().clone();std::thread::spawn(move||{
            let mut link:Option<(String,Link,std::sync::Arc<crate::UsbOperationGuard>)>=None;let mut scan=Instant::now()-Duration::from_secs(3);let mut retry=Instant::now()-Duration::from_secs(3);
            loop {
                std::thread::sleep(Duration::from_millis(150));
                let active=app.state::<StudioState>().0.lock().unwrap().active;
                if !active {
                    // Drop the real serial handle before acknowledging the handoff.
                    link=None;
                    let state=app.state::<StudioState>();let mut s=state.0.lock().unwrap();s.target=None;s.connection="Preview only".into();s.released=true;
                    continue;
                }
                if scan.elapsed()>Duration::from_secs(2) {let ports=connection::robot_ports().unwrap_or_default();app.state::<StudioState>().0.lock().unwrap().ports=ports;scan=Instant::now();}
                let (target,port,frame,brightness,orientation)={let state=app.state::<StudioState>();let mut s=state.0.lock().unwrap();s.refresh();let view=s.view();(s.target.clone(),s.ports.iter().find(|p|Some(&p.serial)==s.target.as_ref()).cloned(),view.frame,view.brightness,view.orientation)};
                if link.as_ref().map(|(id,_,_)|Some(id)!=target.as_ref()).unwrap_or(false)||port.is_none(){link=None;}
                if link.is_none()&&port.is_some()&&retry.elapsed()>Duration::from_secs(3){
                    retry=Instant::now();let p=port.as_ref().unwrap();
                    if let Ok(usb)=crate::UsbOperationGuard::acquire(){
                        match Link::connect(p){Ok(l)=>{link=Some((p.serial.clone(),l,usb));app.state::<StudioState>().0.lock().unwrap().connection="Connected".into();},Err(e)=>app.state::<StudioState>().0.lock().unwrap().connection=e}
                    }
                }
                if let Some((_,l,_))=&mut link {if let Err(e)=l.update(&frame,true,brightness,orientation){app.state::<StudioState>().0.lock().unwrap().connection=e;link=None;}}
                if target.is_some()&&port.is_none(){app.state::<StudioState>().0.lock().unwrap().connection="Droid unplugged — reconnect USB".into();}
                let view=app.state::<StudioState>().0.lock().unwrap().view();
                let _=app.emit("studio-state",view);
            }
        });Ok(())
    }).build(tauri::generate_context!()).expect("DroidFun Workshop could not start")
    .run(|app,event|{if let tauri::RunEvent::ExitRequested{code,api,..}=event {if crate::workshop::busy(app)||(crate::app_update::installing(app)&&code!=Some(tauri::RESTART_EXIT_CODE)){api.prevent_exit();}}});
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_photo_screens_can_save_and_reload_without_accounts() {
        let settings=super::defaults();
        settings.validate().unwrap();
        assert_eq!(settings.widgets.len(),13);
        let bytes=serde_json::to_vec(&settings).unwrap();
        let restored:crate::core::Settings=serde_json::from_slice(&bytes).unwrap();
        restored.validate().unwrap();
        assert!(restored.preferences.bot_model.is_empty());
        assert!(restored.widgets.iter().all(|w|w.config.mode=="live"&&w.config.cli_path.is_empty()));
    }
}

#[cfg(test)]
mod playlist_tests {
    use super::*;
    #[test]
    fn saved_playlist_controls_order_enabled_screens_and_each_duration() {
        let now=Instant::now();let mut e=Engine::new(defaults(),now);
        for w in e.settings.widgets.clone() {if !["clock","text"].contains(&w.id.as_str()) {playlist_action(&mut e,"enabled",&w.id,&json!(false)).unwrap();}}
        playlist_action(&mut e,"duration","clock",&json!(11)).unwrap();
        playlist_action(&mut e,"duration","text",&json!(3)).unwrap();
        while e.settings.widgets.iter().position(|w|w.id=="clock").unwrap()>e.settings.widgets.iter().position(|w|w.id=="text").unwrap() {playlist_action(&mut e,"move","clock",&json!(-1)).unwrap();}
        let saved=serde_json::to_vec(&e.settings).unwrap();let settings:Settings=serde_json::from_slice(&saved).unwrap();settings.validate().unwrap();
        let mut reopened=Engine::new(settings,now);assert_eq!(reopened.snapshot().current_id,"clock");
        reopened.tick(now+Duration::from_secs(10));assert_eq!(reopened.snapshot().current_id,"clock");
        reopened.tick(now+Duration::from_secs(11));assert_eq!(reopened.snapshot().current_id,"text");
        reopened.tick(now+Duration::from_secs(14));assert_eq!(reopened.snapshot().current_id,"clock");
        assert!(playlist_action(&mut reopened,"duration","clock",&json!(121)).is_err());
        playlist_action(&mut reopened,"enabled","text",&json!(false)).unwrap();
        assert!(playlist_action(&mut reopened,"enabled","clock",&json!(false)).is_err());
        assert!(reopened.settings.widgets.iter().any(|w|w.id=="clock"&&w.enabled&&w.duration_sec==11));
    }
}

