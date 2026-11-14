//! Preserve the character pane's pixel scale while allowing effects into transparent margins.
use bevy::{
    camera::{CameraProjection, SubCameraView},
    math::Vec3A,
    prelude::*,
};

pub const BODY_SIZE: Vec2 = Vec2::new(233.0, 224.0);
pub const CANVAS_SIZE: Vec2 = Vec2::new(361.0, 396.0);
const BODY_ORIGIN: Vec2 = Vec2::new(64.0, 140.0);

#[derive(Clone, Debug)]
pub(crate) struct SpeechProjection {
    pub inner: Projection,
}
impl SpeechProjection {
    fn remap() -> Mat4 {
        let fraction = BODY_SIZE / CANVAS_SIZE;
        let center = (BODY_ORIGIN + BODY_SIZE * 0.5) / CANVAS_SIZE;
        let mut remap = Mat4::IDENTITY;
        remap.x_axis.x = fraction.x;
        remap.y_axis.y = fraction.y;
        remap.w_axis.x = center.x * 2.0 - 1.0;
        remap.w_axis.y = 1.0 - center.y * 2.0;
        remap
    }
}
impl CameraProjection for SpeechProjection {
    fn get_clip_from_view(&self) -> Mat4 {
        Self::remap() * self.inner.get_clip_from_view()
    }
    fn get_clip_from_view_for_sub(&self, _: &SubCameraView) -> Mat4 {
        self.get_clip_from_view()
    }
    fn update(&mut self, width: f32, height: f32) {
        self.inner.update(
            width * BODY_SIZE.x / CANVAS_SIZE.x,
            height * BODY_SIZE.y / CANVAS_SIZE.y,
        );
    }
    fn far(&self) -> f32 {
        self.inner.far()
    }
    fn get_frustum_corners(&self, near: f32, far: f32) -> [Vec3A; 8] {
        // Expand culling as well as projection: effects beyond the old body pane are visible.
        let corners = self.inner.get_frustum_corners(near, far);
        let inverse = Self::remap().inverse();
        std::array::from_fn(|i| {
            let plane = &corners[(i / 4) * 4..(i / 4) * 4 + 4];
            let center = (plane[0] + plane[1] + plane[2] + plane[3]) * 0.25;
            let right = (plane[0] + plane[1] - plane[2] - plane[3]) * 0.25;
            let up = (-plane[0] + plane[1] + plane[2] - plane[3]) * 0.25;
            let (x, y) = [(1.0, -1.0), (1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0)][i % 4];
            let old = inverse.project_point3(Vec3::new(x, y, 0.0));
            center + right * old.x + up * old.y
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn padding_preserves_body_pixel_scale() {
        let remap = SpeechProjection::remap();
        for (ndc, pixel) in [
            (Vec3::new(-1.0, 1.0, 0.5), BODY_ORIGIN),
            (Vec3::new(1.0, -1.0, 0.5), BODY_ORIGIN + BODY_SIZE),
        ] {
            let p = remap.project_point3(ndc);
            let actual = Vec2::new((p.x + 1.0) * 0.5, (1.0 - p.y) * 0.5) * CANVAS_SIZE;
            assert!((actual - pixel).length() < 0.001);
        }
    }
    #[test]
    fn expanded_culling_corners_match_padded_projection() {
        for inner in [
            Projection::Perspective(PerspectiveProjection::default()),
            Projection::Orthographic(OrthographicProjection::default_3d()),
        ] {
            let mut p = SpeechProjection { inner };
            p.update(CANVAS_SIZE.x, CANVAS_SIZE.y);
            let matrix = p.get_clip_from_view();
            for (i, corner) in p.get_frustum_corners(-0.1, -100.0).iter().enumerate() {
                let projected = matrix.project_point3((*corner).into());
                let (x, y) = [(1.0, -1.0), (1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0)][i % 4];
                assert!((projected.x - x).abs() < 0.001 && (projected.y - y).abs() < 0.001);
            }
        }
    }
}
