//! Opt-in, low-volume measurement; no names, URLs or full snapshots in output.
use bevy::prelude::*;
use benilla_app::companion_renderer::CompanionRenderCounters;
use std::time::Instant;
#[derive(Resource)]struct Meter{start:Instant,frames:u64,captures:u64,readbacks:u64}
pub fn install(app:&mut App){
 if std::env::var_os("REALM_COMPANION_PERF").is_some(){
  app.init_resource::<CompanionRenderCounters>();app.world_mut().resource_mut::<CompanionRenderCounters>().enabled=true;
  app.insert_resource(Meter{start:Instant::now(),frames:0,captures:0,readbacks:0}).add_systems(Update,measure);
 }
}
fn measure(mut meter:ResMut<Meter>,counts:Res<CompanionRenderCounters>){
 meter.frames+=1;let secs=meter.start.elapsed().as_secs_f64();
 if secs>=15.{info!("companion performance: seconds={secs:.2} update_fps={:.2} mask_fps={:.2} readback_fps={:.2}",meter.frames as f64/secs,(counts.mask_captures-meter.captures) as f64/secs,(counts.mask_readbacks-meter.readbacks) as f64/secs);meter.frames=0;meter.captures=counts.mask_captures;meter.readbacks=counts.mask_readbacks;meter.start=Instant::now();}
}
