#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput
@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var source_sampler: sampler;
@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    // The 3D view only renders below the 44/268 speech band. The post-process
    // triangle spans the full target, so never sample undefined pixels outside it.
    // The separate, later UI camera paints the speech bubble over transparent pixels.
    if in.uv.y < (44.0 / 268.0) { return vec4<f32>(0.0); }
    let color = textureSample(source, source_sampler, in.uv);
#ifdef COMPANION_PREMULTIPLIED_SURFACE
    // Core Animation may cull alpha-zero pixels even when additive RGB is nonzero.
    // Keep the authored premultiplied RGB exactly; one 8-bit alpha quantum retains it.
    let emission = max(color.r, max(color.g, color.b));
    let alpha = select(color.a, 1.0 / 255.0, emission > 0.0 && color.a < 1.0 / 255.0);
    return vec4<f32>(color.rgb, alpha);
#else
    let coverage = clamp(max(color.a, max(color.r, max(color.g, color.b))), 0.0, 1.0);
    if coverage <= 0.00001 { return vec4<f32>(0.0); }
    return vec4<f32>(color.rgb / coverage, coverage);
#endif
}
