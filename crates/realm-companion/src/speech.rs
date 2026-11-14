//! Cosmetic dialogue on the primary pet window; no chat packets or pointer targets.
use bevy::prelude::*;
use benilla_app::companion_renderer::CompanionLocalGesture;
#[derive(Component)]struct Bubble;
#[derive(Component)]struct Line;
#[derive(Component)]struct SpeechCamera;
pub fn install(app:&mut App){app.add_systems(Startup,setup).add_systems(Update,update);}
fn setup(mut c:Commands,assets:Res<AssetServer>,ui:Res<crate::UiState>){
 // The full-size 3D target already clears transparent; no extra empty clear pass.
 let camera=c.spawn((SpeechCamera,Camera2d,Camera{order:100,is_active:false,clear_color:ClearColorConfig::None,..default()})).id();
 let font=benilla_formats::Chain::open(std::path::Path::new(&ui.active_data_dir)).ok().and_then(|chain|["Fonts/FZLBJW.TTF","Fonts/FZXHJW.TTF","Fonts/ARIALN.TTF"].into_iter().find(|name|chain.contains(name))).map(|name|assets.load(format!("mpq://{}",name.to_ascii_lowercase()))).unwrap_or_default();
 // Move the original speech band with the 96-unit effect padding; retain its 8px inset.
 c.spawn((Bubble,UiTargetCamera(camera),Node{position_type:PositionType::Absolute,top:percent(100.0 * 96.0 / 396.0),margin:UiRect::top(px(8)),width:percent(100),justify_content:JustifyContent::Center,..default()},Visibility::Hidden)).with_children(|c|{
  c.spawn((Node{padding:UiRect::axes(px(12),px(7)),border:UiRect::all(px(2)),max_width:percent(90),border_radius:BorderRadius::all(px(12)),..default()},BackgroundColor(Color::srgba(1.,0.98,0.90,0.97)),BorderColor::all(Color::srgb(0.18,0.12,0.06)))).with_children(|c|{
   c.spawn((Line,Text::new(""),TextFont{font,font_size:18.,..default()},TextColor(Color::srgb(0.12,0.08,0.03))));
  });
 });
}
fn update(gesture:Res<CompanionLocalGesture>,interaction:Res<crate::Interaction>,ui:Res<crate::UiState>,mut bubbles:Query<&mut Visibility,With<Bubble>>,mut lines:Query<&mut Text,With<Line>>,mut cameras:Query<&mut Camera,With<SpeechCamera>>){
 let show=ui.settings.show_speech_bubble&&interaction.dancing&&gesture.active&&gesture.selected.is_some()&&ui.page==crate::UiPage::Settings;
 for mut camera in &mut cameras{if camera.is_active!=show{camera.is_active=show;}}
 for mut visibility in &mut bubbles{let next=if show{Visibility::Inherited}else{Visibility::Hidden};if *visibility!=next{*visibility=next;}}
 if show{for mut text in &mut lines{let next=dialogue(gesture.selected.unwrap(),gesture.request);if text.0!=next{text.0=next.into();}}}
}
fn dialogue(animation:u16,request:u64)->&'static str{
 let lines=match animation{
  60=>["欢迎你呀～","见到你真好。"],64=>["恭喜，太棒了！","值得庆祝！"],65=>["嗯？怎么啦？","在想什么呢？"],
  66=>["很高兴见到你。","向你致意～"],67=>["嗨～我在这里！","你好呀！"],68=>["为了胜利！","加油，加油！"],69=>["来跳一小段吧～","跟上这个节奏！"],70=>["哈哈哈哈！","这也太有意思了！"],
  73=>["哼，才不怕呢！","略略略～"],74=>["嗷——！","气势不能输！"],75=>["让我歇一小会儿。","稍微休息一下～"],76=>["送你一个飞吻～","啾～"],77=>["呜……安慰我一下。","有一点点委屈。"],78=>["咕咕咕～","今天客串一下小鸡！"],79=>["拜托拜托～","就答应我嘛～"],80=>["漂亮！","给你鼓掌！"],81=>["大家看这里！","我准备好啦！"],82=>["看看我的力量！","今天状态不错！"],83=>["哎呀，别一直看我～","有一点害羞……"],84=>["看，那边！","目标就在前方！"],113=>["收到，向你敬礼！","随时待命！"],_=>["你好呀！","我在这里～"]};
 lines[(request as usize)%lines.len()]
}
#[cfg(test)]mod tests{use super::*;#[test]fn gesture_dialogue_is_stable_and_specific(){assert_eq!(dialogue(67,2),"嗨～我在这里！");assert_eq!(dialogue(67,2),dialogue(67,2));assert_ne!(dialogue(67,2),dialogue(80,2));assert_ne!(dialogue(67,2),dialogue(67,3));}}

#[cfg(test)]mod render_gate_tests {
 use super::*;
 #[test]fn speech_camera_only_runs_for_visible_local_dialogue(){
  let mut app=App::new();let mut ui=crate::UiState::default();ui.settings.show_speech_bubble=true;ui.page=crate::UiPage::Settings;
  app.insert_resource(ui).init_resource::<crate::Interaction>().init_resource::<CompanionLocalGesture>().add_systems(Update,update);
  let cam=app.world_mut().spawn((SpeechCamera,Camera::default())).id();app.world_mut().spawn((Bubble,Visibility::Hidden));app.world_mut().spawn((Line,Text::new("")));
  app.update();assert!(!app.world().get::<Camera>(cam).unwrap().is_active);
  app.world_mut().resource_mut::<crate::Interaction>().dancing=true;
  {let mut g=app.world_mut().resource_mut::<CompanionLocalGesture>();g.active=true;g.selected=Some(67);g.request=2;}
  app.update();assert!(app.world().get::<Camera>(cam).unwrap().is_active);
  app.world_mut().resource_mut::<crate::UiState>().settings.show_speech_bubble=false;
  app.update();assert!(!app.world().get::<Camera>(cam).unwrap().is_active);
 }
}
