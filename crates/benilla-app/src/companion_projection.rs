//! Keep a full transparent render target, reserving the speech band in clip space.
use bevy::{prelude::*,camera::{CameraProjection,SubCameraView},math::Vec3A};
#[derive(Clone,Debug)]pub(crate) struct SpeechProjection {pub inner:Projection}
impl CameraProjection for SpeechProjection {
 fn get_clip_from_view(&self)->Mat4{
  let body=224./268.;let pad=1.-body;
  let mut remap=Mat4::IDENTITY;remap.y_axis.y=body;remap.w_axis.y=-pad;
  remap*self.inner.get_clip_from_view()
 }
 fn get_clip_from_view_for_sub(&self,_:&SubCameraView)->Mat4{self.get_clip_from_view()}
 fn update(&mut self,width:f32,height:f32){self.inner.update(width,height*224./268.);}
 fn far(&self)->f32{self.inner.far()}
 fn get_frustum_corners(&self,near:f32,far:f32)->[Vec3A;8]{self.inner.get_frustum_corners(near,far)}
}
