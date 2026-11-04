// Draws a spectrogram from a static mesh whose heights and colours come from a data texture
// with the (min, max) of every cell. Only the texture changes when new data arrives.

import bevy_pbr::render::mesh_functions::{
    get_world_from_local, mesh_position_local_to_clip, mesh_normal_local_to_world
};

struct Params {
    // cells with data: (time, frequency)
    dims: vec2<u32>,
    value_min: f32,
    value_max: f32,
    // frequency cells including a partial last one (bands / bands_per_cell)
    freq_extent: f32,
    // 0: cubes, 1: surface
    mode: u32,
    // 0: columns up to the maximum, 1: boxes from minimum to maximum
    cell_value: u32,
    // local x range that is inside the selection; the rest is discarded
    clip_x: vec2<f32>,
};

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> params: Params;
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var data: texture_2d<f32>;

const PLACEHOLDER_HEIGHT: f32 = 0.01;
const PLACEHOLDER_COLOR: vec3<f32> = vec3<f32>(0.33, 0.33, 0.35);
const LIGHT: vec3<f32> = vec3<f32>(0.35, 0.85, 0.4);

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    // x and z in cells, y selects the bottom (0) or top (1) of a cube
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) cell: vec2<u32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) color: vec3<f32>,
    @location(2) local_x: f32,
};

fn normalized(value: f32) -> f32 {
    let span = params.value_max - params.value_min;
    if span <= 0.0 {
        return 0.0;
    }
    return clamp((value - params.value_min) / span, 0.0, 1.0);
}

// (min, max) of a cell; min > max marks a cell without data
fn cell_value(cell: vec2<u32>) -> vec2<f32> {
    if cell.x >= params.dims.x || cell.y >= params.dims.y {
        return vec2<f32>(1.0, -1.0);
    }
    return textureLoad(data, vec2<i32>(cell), 0).xy;
}

fn height(cell: vec2<i32>) -> f32 {
    let clamped = clamp(cell, vec2<i32>(0), vec2<i32>(params.dims) - vec2<i32>(1));
    let value = cell_value(vec2<u32>(clamped));
    return select(normalized(value.y), 0.0, value.x > value.y);
}

// viridis, polynomial fit; keep in sync with colormap.rs
fn viridis(t: f32) -> vec3<f32> {
    let c0 = vec3<f32>(0.2777273, 0.005407344, 0.3340998);
    let c1 = vec3<f32>(0.10509304, 1.4046135, 1.3845902);
    let c2 = vec3<f32>(-0.33086183, 0.21484756, 0.09509516);
    let c3 = vec3<f32>(-4.6342305, -5.799101, -19.332441);
    let c4 = vec3<f32>(6.22827, 14.179933, 56.69055);
    let c5 = vec3<f32>(4.776385, -13.745145, -65.353035);
    let c6 = vec3<f32>(-5.435456, 4.6458526, 26.312435);
    return clamp(c0 + t * (c1 + t * (c2 + t * (c3 + t * (c4 + t * (c5 + t * c6))))), vec3<f32>(0.0), vec3<f32>(1.0));
}

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    return pow(c, vec3<f32>(2.2));
}

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    var cell = vertex.cell;
    var position = vertex.position;
    let outside = cell.x >= params.dims.x || cell.y >= params.dims.y;
    if params.mode == 1u && outside {
        // surface vertices outside of the grid move onto the last cell, so the triangles that
        // connect them have no area (collapsing to a point would stretch them across the view)
        cell = min(cell, max(params.dims, vec2<u32>(1u)) - vec2<u32>(1u));
        position = vec3<f32>(vec2<f32>(cell) + 0.5, 1.0).xzy;
    }
    let value = cell_value(cell);
    let empty = value.x > value.y;
    let top = normalized(value.y);

    var normal = vertex.normal;
    if params.mode == 0u {
        let bottom = select(0.0, normalized(value.x), params.cell_value == 1u);
        if empty {
            position.y = position.y * PLACEHOLDER_HEIGHT;
        } else {
            position.y = mix(bottom, max(top, bottom + PLACEHOLDER_HEIGHT), position.y);
        }
    } else {
        position.y = select(top, 0.0, empty);
        // normal from the neighbouring heights; local x and z are in cells
        let at = vec2<i32>(cell);
        let dx = height(at + vec2<i32>(1, 0)) - height(at - vec2<i32>(1, 0));
        let dz = height(at + vec2<i32>(0, 1)) - height(at - vec2<i32>(0, 1));
        normal = vec3<f32>(-dx * 0.5, 1.0, -dz * 0.5);
    }
    position.z = min(position.z, params.freq_extent);

    // cubes outside of the grid collapse to a point
    if params.mode == 0u && outside {
        position = vec3<f32>(0.0);
    }

    out.clip_position = mesh_position_local_to_clip(
        get_world_from_local(vertex.instance_index),
        vec4<f32>(position, 1.0),
    );
    out.normal = mesh_normal_local_to_world(normal, vertex.instance_index);
    out.color = select(viridis(top), PLACEHOLDER_COLOR, empty);
    out.local_x = position.x;
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    if in.local_x < params.clip_x.x || in.local_x > params.clip_x.y {
        discard;
    }
    let light = 0.45 + 0.55 * abs(dot(normalize(in.normal), normalize(LIGHT)));
    return vec4<f32>(srgb_to_linear(in.color) * light, 1.0);
}
