#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput
@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var source_sampler: sampler;
// Model and particle passes produce authored gamma-space bytes. Decode once before the
// sRGB surface encodes them, just as the game's FFX BOOTH exit does.
fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let lo = c / 12.92;
    let hi = pow((c + vec3<f32>(0.055)) / 1.055, vec3<f32>(2.4));
    return select(hi, lo, c <= vec3<f32>(0.04045));
}
@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let raw = textureSample(source, source_sampler, in.uv);
    let color = vec4<f32>(srgb_to_linear(clamp(raw.rgb, vec3<f32>(0.0), vec3<f32>(1.0))), raw.a);
#ifdef COMPANION_PREMULTIPLIED_SURFACE
    // Core Animation may cull alpha-zero pixels even when additive RGB is nonzero.
    // Presentation encodes the decoded premultiplied RGB once; one alpha quantum retains it.
    let emission = max(raw.r, max(raw.g, raw.b));
    let alpha = select(color.a, 1.0 / 255.0, emission > 0.0 && color.a < 1.0 / 255.0);
    return vec4<f32>(color.rgb, alpha);
#else
    let coverage = clamp(max(raw.a, max(raw.r, max(raw.g, raw.b))), 0.0, 1.0);
    if coverage <= 0.00001 { return vec4<f32>(0.0); }
    // Straight-alpha surfaces need unpremultiplication in gamma before the nonlinear decode.
    let straight = clamp(raw.rgb / coverage, vec3<f32>(0.0), vec3<f32>(1.0));
    return vec4<f32>(srgb_to_linear(straight), coverage);
#endif
}
