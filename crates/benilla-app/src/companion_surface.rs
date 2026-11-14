//! Match the companion surface to its native desktop compositor contract.
//! On Metal, preserve premultiplied RGB and retain emission with one alpha quantum.
use bevy::image::BevyDefault;
use bevy::{
    asset::embedded_asset,
    core_pipeline::{
        core_3d::graph::{Core3d, Node3d},
        FullscreenShader,
    },
    ecs::query::QueryItem,
    prelude::*,
    render::{
        extract_component::{ExtractComponent, ExtractComponentPlugin},
        render_graph::{
            NodeRunError, RenderGraphContext, RenderGraphExt, RenderLabel, ViewNode, ViewNodeRunner,
        },
        render_resource::{
            binding_types::{sampler, texture_2d},
            *,
        },
        renderer::{RenderContext, RenderDevice},
        view::ViewTarget,
        RenderApp, RenderStartup,
    },
};
#[derive(Component, Clone, Copy, ExtractComponent)]
pub(crate) struct DesktopSurface;
pub(crate) struct DesktopSurfacePlugin;
impl Plugin for DesktopSurfacePlugin {
    fn build(&self, app: &mut App) {
        // Metal/Core Animation consumes premultiplied RGB despite wgpu 27's label.
        // The Mac shader only retains zero-alpha emission with one alpha quantum.
        // Its RGB is never unpremultiplied (wgpu#9922; owned-window probe verified).
        embedded_asset!(app, "companion_surface.wgsl");
        app.add_plugins(ExtractComponentPlugin::<DesktopSurface>::default());
        if let Some(render) = app.get_sub_app_mut(RenderApp) {
            render
                .add_systems(RenderStartup, init)
                .add_render_graph_node::<ViewNodeRunner<SurfaceNode>>(Core3d, SurfaceLabel)
                .add_render_graph_edges(
                    Core3d,
                    (
                        Node3d::Tonemapping,
                        SurfaceLabel,
                        Node3d::EndMainPassPostProcessing,
                    ),
                );
        }
    }
}
#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
struct SurfaceLabel;
#[derive(Default)]
struct SurfaceNode;
#[derive(Resource)]
struct SurfacePipeline {
    layout: BindGroupLayoutDescriptor,
    sampler: Sampler,
    pipelines: std::collections::HashMap<TextureFormat, CachedRenderPipelineId>,
}
impl ViewNode for SurfaceNode {
    type ViewQuery = (&'static ViewTarget, &'static DesktopSurface);
    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render: &mut RenderContext,
        (target, _): QueryItem<Self::ViewQuery>,
        world: &World,
    ) -> Result<(), NodeRunError> {
        let pipeline = world.resource::<SurfacePipeline>();
        let cache = world.resource::<PipelineCache>();
        let Some(id) = pipeline.pipelines.get(&target.main_texture_format()) else {
            return Ok(());
        };
        let Some(ready) = cache.get_render_pipeline(*id) else {
            return Ok(());
        };
        let write = target.post_process_write();
        let bindings = render.render_device().create_bind_group(
            "companion_surface",
            &cache.get_bind_group_layout(&pipeline.layout),
            &BindGroupEntries::sequential((write.source, &pipeline.sampler)),
        );
        let mut pass = render.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("companion_desktop_coverage"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: write.destination,
                depth_slice: None,
                resolve_target: None,
                ops: Operations::default(),
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_render_pipeline(ready);
        pass.set_bind_group(0, &bindings, &[]);
        pass.draw(0..3, 0..1);
        Ok(())
    }
}
fn init(
    mut c: Commands,
    device: Res<RenderDevice>,
    assets: Res<AssetServer>,
    fullscreen: Res<FullscreenShader>,
    cache: Res<PipelineCache>,
) {
    let layout = BindGroupLayoutDescriptor::new(
        "companion_surface",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                texture_2d(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
            ),
        ),
    );
    let shader = assets.load("embedded://benilla_app/companion_surface.wgsl");
    let sampler = device.create_sampler(&SamplerDescriptor::default());
    let pipelines = [TextureFormat::bevy_default(), TextureFormat::Rgba16Float]
        .into_iter()
        .map(|format| {
            let id = cache.queue_render_pipeline(RenderPipelineDescriptor {
                label: Some("companion_surface".into()),
                layout: vec![layout.clone()],
                vertex: fullscreen.to_vertex_state(),
                fragment: Some(FragmentState {
                    shader: shader.clone(),
                    shader_defs: if cfg!(target_os = "macos") {
                        vec!["COMPANION_PREMULTIPLIED_SURFACE".into()]
                    } else {
                        vec![]
                    },
                    targets: vec![Some(ColorTargetState {
                        format,
                        blend: None,
                        write_mask: ColorWrites::ALL,
                    })],
                    ..default()
                }),
                ..default()
            });
            (format, id)
        })
        .collect();
    c.insert_resource(SurfacePipeline {
        layout,
        sampler,
        pipelines,
    });
}
