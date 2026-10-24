// Complete XRay-Monolith R4 surface, shadow, GI radiosity, volumetric, and HDR10 WESL shader.

import bevy_pbr::render::forward_io::{Vertex, decompress_vertex};
import bevy_pbr::render::mesh_functions::{
    get_world_from_local,
    mesh_normal_local_to_world,
    mesh_position_local_to_world,
};
import bevy_pbr::render::mesh_view_bindings::view;
import bevy_pbr::render::view_transformations::{frag_coord_to_uv, position_world_to_clip};

struct MoonlyVertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) world_position: vec4<f32>,
    @location(1) world_normal: vec3<f32>,
}

struct MoonlyLitUniform {
    albedo_roughness: vec4<f32>,
    emissive_ao: vec4<f32>,
    sun_dir_shadow_softness: vec4<f32>,
    sun_color_wetness: vec4<f32>,
    hemi_color: vec4<f32>,
    ambient_fog_density: vec4<f32>,
    spot_pos_range: vec4<f32>,
    spot_dir_cos: vec4<f32>,
    spot_color_vol: vec4<f32>,
    gi_pos_range_0: vec4<f32>,
    gi_color_0: vec4<f32>,
    gi_pos_range_1: vec4<f32>,
    gi_color_1: vec4<f32>,
    gi_pos_range_2: vec4<f32>,
    gi_color_2: vec4<f32>,
    blocker_0: vec4<f32>,
    blocker_1: vec4<f32>,
    blocker_2: vec4<f32>,
    blocker_3: vec4<f32>,
    post_params: vec4<f32>, // x = exposure, y = contrast, z = vibrance, w = shaft_density
    mode_flags: vec4<u32>,  // x = tonemap_op, y = optics_mode, z = shadow_mode, w = ao_mode
}

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> mat: MoonlyLitUniform;

const INV_FOUR_PI: f32 = 0.07957747154594767;
const REC709_LUMA: vec3<f32> = vec3<f32>(0.212656, 0.715158, 0.072186);

fn colored_ao_activision(ao: f32, albedo: vec3<f32>) -> vec3<f32> {
    let a = 2.0404 * albedo - vec3<f32>(0.3324);
    let b = -4.7951 * albedo + vec3<f32>(0.6417);
    let c = 2.7552 * albedo + vec3<f32>(0.6903);
    let poly = ((vec3<f32>(ao) * a + b) * ao + c) * ao;
    return max(vec3<f32>(ao), poly);
}

fn eval_virtual_bounce(world_pos: vec3<f32>, n: vec3<f32>, pos_range: vec4<f32>, col: vec4<f32>) -> vec3<f32> {
    let to_l = pos_range.xyz - world_pos;
    let r = max(pos_range.w, 0.001);
    let d = length(to_l);
    if d >= r || d <= 0.0001 {
        return vec3<f32>(0.0);
    }
    let l_dir = to_l / d;
    let ndotl = max(dot(n, l_dir), 0.0);
    let atten = clamp(1.0 - d / r, 0.0, 1.0);
    return col.xyz * (ndotl * atten * atten);
}

fn eval_blocker_pcss(world_pos: vec3<f32>, n: vec3<f32>, light_dir: vec3<f32>, b: vec4<f32>, softness: f32) -> f32 {
    let origin = world_pos + n * 0.04;
    if b.w > 0.001 {
        let r = b.w;
        if length(world_pos - b.xyz) <= r + 0.08 {
            return 1.0;
        }
        let to_c = b.xyz - origin;
        let t = dot(to_c, light_dir);
        if t <= 0.05 {
            return 1.0;
        }
        let closest = origin + light_dir * t;
        let d = length(b.xyz - closest) - r;
        let w = max(0.035 + softness * 0.14 * t, 0.035);
        return smoothstep(-w * 0.5, w * 0.5, d);
    }
    if b.w < -0.001 {
        let r = -b.w;
        let top_y = b.y;
        let p_xz = vec2<f32>(b.x, b.z);
        if length(world_pos.xz - p_xz) <= r + 0.08 && world_pos.y <= top_y + 0.08 {
            return 1.0;
        }
        let dir_xz = light_dir.xz;
        let len_xz_sq = max(dot(dir_xz, dir_xz), 0.0001);
        let t = dot(p_xz - origin.xz, dir_xz) / len_xz_sq;
        if t <= 0.05 {
            return 1.0;
        }
        let hit = origin + light_dir * t;
        let d_rad = length(hit.xz - p_xz) - r;
        let d_top = hit.y - top_y;
        let d = max(d_rad, d_top);
        let w = max(0.035 + softness * 0.14 * t, 0.035);
        return smoothstep(-w * 0.5, w * 0.5, d);
    }
    return 1.0;
}

fn eval_sun_shadow_pcss(world_pos: vec3<f32>, n: vec3<f32>, light_dir: vec3<f32>, softness: f32) -> f32 {
    let v0 = eval_blocker_pcss(world_pos, n, light_dir, mat.blocker_0, softness);
    let v1 = eval_blocker_pcss(world_pos, n, light_dir, mat.blocker_1, softness);
    let v2 = eval_blocker_pcss(world_pos, n, light_dir, mat.blocker_2, softness);
    let v3 = eval_blocker_pcss(world_pos, n, light_dir, mat.blocker_3, softness);
    return min(min(v0, v1), min(v2, v3));
}

fn tonemap_dispatch(rgb: vec3<f32>, op: u32) -> vec3<f32> {
    let x = max(rgb, vec3<f32>(0.0));
    if op == 0u {
        return (x * (vec3<f32>(1.0) + x / 2.89)) / (x + vec3<f32>(1.0));
    }
    if op == 3u {
        return clamp((x * (2.51 * x + vec3<f32>(0.03))) / (x * (2.43 * x + vec3<f32>(0.59)) + vec3<f32>(0.14)), vec3<f32>(0.0), vec3<f32>(1.0));
    }
    let v = vec3<f32>(
        0.59719 * x.x + 0.35458 * x.y + 0.04823 * x.z,
        0.07600 * x.x + 0.90834 * x.y + 0.01566 * x.z,
        0.02840 * x.x + 0.13383 * x.y + 0.83777 * x.z,
    );
    let a = v * (v + vec3<f32>(0.0245786)) - vec3<f32>(0.000090537);
    let b = v * (0.983729 * v + vec3<f32>(0.4329510)) + vec3<f32>(0.238081);
    let rrt = a / b;
    let out = vec3<f32>(
        1.60475 * rrt.x - 0.53108 * rrt.y - 0.07367 * rrt.z,
        -0.10208 * rrt.x + 1.10813 * rrt.y - 0.00605 * rrt.z,
        -0.00327 * rrt.x - 0.07276 * rrt.y + 1.07602 * rrt.z,
    );
    return clamp(out, vec3<f32>(0.0), vec3<f32>(1.0));
}

fn apply_optics(rgb: vec3<f32>, uv: vec2<f32>, mode: u32) -> vec3<f32> {
    if mode == 1u || mode == 2u {
        let luma = max(dot(rgb, REC709_LUMA), 0.0);
        let amp = clamp(pow(luma * 3.8, 0.78), 0.0, 1.4);
        let d = length(uv - vec2<f32>(0.5));
        let vig = pow(1.0 - clamp((d - 0.42) / 0.16, 0.0, 1.0), 2.0);
        let phos = select(vec3<f32>(0.18, 0.96, 0.28), vec3<f32>(0.68, 0.92, 0.98), mode == 2u);
        return phos * (amp * vig);
    }
    if mode == 3u {
        let h = clamp(dot(rgb, REC709_LUMA), 0.0, 1.0);
        let cold = vec3<f32>(0.05, 0.08, 0.35);
        let mid = vec3<f32>(0.78, 0.14, 0.52);
        let hot = vec3<f32>(1.0, 0.92, 0.25);
        return select(mix(mid, hot, (h - 0.5) * 2.0), mix(cold, mid, h * 2.0), h < 0.5);
    }
    return rgb;
}

@vertex
fn vertex(packed: Vertex) -> MoonlyVertexOut {
    let v = decompress_vertex(packed, packed.instance_index);
    let world_from_local = get_world_from_local(packed.instance_index);
    let world_pos = mesh_position_local_to_world(world_from_local, vec4<f32>(v.position, 1.0));
    let world_norm = mesh_normal_local_to_world(v.normal, packed.instance_index);
    var out: MoonlyVertexOut;
    out.position = position_world_to_clip(world_pos.xyz);
    out.world_position = world_pos;
    out.world_normal = world_norm;
    return out;
}

@fragment
fn fragment(mesh: MoonlyVertexOut) -> @location(0) vec4<f32> {
    let w_pos = mesh.world_position.xyz;
    let n = normalize(mesh.world_normal);
    let view_vec = view.world_position - w_pos;
    let view_dist = max(length(view_vec), 0.001);
    let v_dir = view_vec / view_dist;

    let wet = clamp(mat.sun_color_wetness.w * clamp(n.y, 0.0, 1.0), 0.0, 1.0);
    let albedo = mat.albedo_roughness.xyz * mix(1.0, 0.58, wet);
    let roughness = mix(mat.albedo_roughness.w, 0.08, wet);
    let spec_power = mix(128.0, 8.0, roughness * roughness);
    let gloss = 1.0 - roughness * 0.85;

    let l_sun = normalize(-mat.sun_dir_shadow_softness.xyz);
    let sun_ndotl = max(dot(n, l_sun), 0.0);
    let sun_vis = eval_sun_shadow_pcss(w_pos, n, l_sun, mat.sun_dir_shadow_softness.w);
    let h_sun = normalize(l_sun + v_dir);
    let sun_spec = pow(max(dot(n, h_sun), 0.0), spec_power) * gloss * 0.45;
    let direct_sun = mat.sun_color_wetness.xyz * ((albedo * sun_ndotl + vec3<f32>(sun_spec)) * sun_vis);

    let to_spot = mat.spot_pos_range.xyz - w_pos;
    let spot_dist = max(length(to_spot), 0.001);
    let spot_l = to_spot / spot_dist;
    let spot_atten = clamp(1.0 - spot_dist / max(mat.spot_pos_range.w, 0.01), 0.0, 1.0);
    let spot_cos = dot(-spot_l, normalize(mat.spot_dir_cos.xyz));
    let spot_cone = smoothstep(mat.spot_dir_cos.w, min(mat.spot_dir_cos.w + 0.15, 1.0), spot_cos);
    let spot_ndotl = max(dot(n, spot_l), 0.0);
    let h_spot = normalize(spot_l + v_dir);
    let spot_spec = pow(max(dot(n, h_spot), 0.0), spec_power) * gloss * 0.6;
    let direct_spot = mat.spot_color_vol.xyz * ((albedo * spot_ndotl + vec3<f32>(spot_spec)) * spot_cone * spot_atten * spot_atten);

    let gi_sum = eval_virtual_bounce(w_pos, n, mat.gi_pos_range_0, mat.gi_color_0)
        + eval_virtual_bounce(w_pos, n, mat.gi_pos_range_1, mat.gi_color_1)
        + eval_virtual_bounce(w_pos, n, mat.gi_pos_range_2, mat.gi_color_2);
    let indirect_gi = albedo * gi_sum * 1.4;

    let ground_occ = clamp((w_pos.y + 0.4) * 0.65, 0.25, 1.0) * mat.emissive_ao.w;
    let ao_tint = select(vec3<f32>(ground_occ), colored_ao_activision(ground_occ, albedo), mat.mode_flags.w >= 3u);
    let hemi_factor = 0.5 + 0.5 * n.y;
    let ambient_term = (mat.ambient_fog_density.xyz + mat.hemi_color.xyz * (hemi_factor * mat.hemi_color.w)) * albedo * ao_tint;

    let cos_ray_sun = dot(-v_dir, l_sun);
    let g = 0.68;
    let mie = ((1.0 - g * g) * INV_FOUR_PI) / pow(max(1.0 + g * g - 2.0 * g * cos_ray_sun, 0.001), 1.5);
    let shaft = mat.sun_color_wetness.xyz * (mie * min(view_dist, 40.0) * mat.post_params.w * 0.025);
    let fog_f = clamp(1.0 - exp(-view_dist * mat.ambient_fog_density.w), 0.0, 0.75);

    var hdr = direct_sun + direct_spot + indirect_gi + ambient_term + mat.emissive_ao.xyz + shaft;
    hdr = mix(hdr, mat.hemi_color.xyz * 0.55, fog_f);
    hdr = hdr * mat.post_params.x;

    let uv = frag_coord_to_uv(mesh.position.xy);
    var mapped = tonemap_dispatch(hdr, mat.mode_flags.x);
    let luma = dot(mapped, REC709_LUMA);
    mapped = mix(vec3<f32>(luma), mapped, 1.0 + mat.post_params.z);
    mapped = apply_optics(mapped, uv, mat.mode_flags.y);
    return vec4<f32>(mapped, 1.0);
}
