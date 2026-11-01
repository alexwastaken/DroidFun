//! Private photography app. All remote readings are generated locally; no account APIs run.
use crate::{connection::{self, Link, RobotPort}, core::{self, DataCard, Engine, Playback, Settings, SourceConfig, Widget}, local_widgets};
use serde::Serialize;
use serde_json::{json, Value};
use std::{sync::Mutex, time::{Duration, Instant}};
use tauri::{Emitter, Manager, State};

struct Studio { engine: Engine, ports: Vec<RobotPort>, target: Option<String>, connection: String, moving: bool, started: Instant, codex: f64, claude: f64, path: std::path::PathBuf }
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
    engine.settings
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
    let mut s=state.0.lock().unwrap();let v=value.unwrap_or(Value::Null);let mut save=false;
    match action.as_str() {
        "select"=>{s.engine.pin(id.as_deref().ok_or("Choose a screen")?)?;save=true;},
        "cycle"=>{s.engine.toggle_pause();},
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
pub fn run() {
    tauri::Builder::default().plugin(tauri_plugin_single_instance::init(|app,_,_|{if let Some(w)=app.get_webview_window("main"){let _=w.show();let _=w.set_focus();}}))
    .invoke_handler(tauri::generate_handler![showcase_state,showcase_action])
    .setup(|app|{
        let path=app.path().app_config_dir()?.join("studio-settings.json");
        let settings=std::fs::read(&path).ok().and_then(|b|serde_json::from_slice::<Settings>(&b).ok()).filter(|s|s.validate().is_ok()&&s.preferences.bot_model.is_empty()).unwrap_or_else(defaults);
        let mut studio=Studio{engine:Engine::new(settings,Instant::now()),ports:Vec::new(),target:None,connection:"Not connected".into(),moving:true,started:Instant::now(),codex:78.0,claude:64.0,path};studio.refresh();let _=studio.engine.pin("codex");app.manage(StudioState(Mutex::new(studio)));
        tauri::WebviewWindowBuilder::new(app,"main",tauri::WebviewUrl::App("index.html".into())).title("DroidFun").inner_size(980.0,780.0).min_inner_size(680.0,520.0).on_navigation(|url|crate::web_security::local_navigation(url,false)).on_new_window(|_,_|tauri::webview::NewWindowResponse::Deny).build()?;
        let app=app.handle().clone();std::thread::spawn(move||{
            let mut link:Option<(String,Link)>=None;let mut scan=Instant::now()-Duration::from_secs(3);let mut retry=Instant::now()-Duration::from_secs(3);
            loop {
                std::thread::sleep(Duration::from_millis(150));
                if scan.elapsed()>Duration::from_secs(2) {let ports=connection::robot_ports().unwrap_or_default();app.state::<StudioState>().0.lock().unwrap().ports=ports;scan=Instant::now();}
                let (target,port,frame,brightness,orientation,view)={let state=app.state::<StudioState>();let mut s=state.0.lock().unwrap();s.refresh();let view=s.view();(s.target.clone(),s.ports.iter().find(|p|Some(&p.serial)==s.target.as_ref()).cloned(),view.frame.clone(),view.brightness,view.orientation,view)};
                if link.as_ref().map(|(id,_)|Some(id)!=target.as_ref()).unwrap_or(false)||port.is_none(){link=None;}
                if link.is_none()&&port.is_some()&&retry.elapsed()>Duration::from_secs(3){retry=Instant::now();let p=port.as_ref().unwrap();match Link::connect(p){Ok(l)=>{link=Some((p.serial.clone(),l));app.state::<StudioState>().0.lock().unwrap().connection="Connected".into();},Err(e)=>app.state::<StudioState>().0.lock().unwrap().connection=e}}
                if let Some((_,l))=&mut link {if let Err(e)=l.update(&frame,true,brightness,orientation){app.state::<StudioState>().0.lock().unwrap().connection=e;link=None;}}
                if target.is_some()&&port.is_none(){app.state::<StudioState>().0.lock().unwrap().connection="Droid unplugged — reconnect USB".into();}
                let _=app.emit("studio-state",view);
            }
        });Ok(())
    }).run(tauri::generate_context!()).expect("DroidFun Studio could not start");
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_photo_screens_can_save_and_reload_without_accounts() {
        let settings=super::defaults();
        settings.validate().unwrap();
        assert_eq!(settings.widgets.len(),12);
        let bytes=serde_json::to_vec(&settings).unwrap();
        let restored:crate::core::Settings=serde_json::from_slice(&bytes).unwrap();
        restored.validate().unwrap();
        assert!(restored.preferences.bot_model.is_empty());
        assert!(restored.widgets.iter().all(|w|w.config.mode=="live"&&w.config.cli_path.is_empty()));
    }
}
