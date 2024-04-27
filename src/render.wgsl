@group(0)
@binding(0)
var<uniform> screen_size: vec2<f32>;

@group(0)
@binding(1)
var<uniform> view: View;

struct View {
    offset: vec2<f32>,
    zoom: f32,
}

struct VertexOutput {
    @builtin(position) coord_in: vec4<f32>,
    @location(0) position: vec2<f32>,
    @location(1) radius: f32,
    @location(2) velocity: vec2<f32>,
}

@vertex
fn vertex(
    @builtin(vertex_index) in_vertex_index: u32,
    @location(0) _particle_position: vec2<f32>,
    @location(1) particle_velocity: vec2<f32>,
    @location(2) _particle_radius: f32,
    @location(3) particle_mass: f32,
    @location(4) position: vec2<f32>,
) -> VertexOutput {
    if particle_mass == 0.0 {
        return VertexOutput();
    }

    var pos = position + vec2<f32>(0.0, 0.38);
    // let angle = -atan2(particle_velocity.x, particle_velocity.y);
    // pos = vec2<f32>(
    //     pos.x * cos(angle) - pos.y * sin(angle),
    //     pos.x * sin(angle) + pos.y * cos(angle),
    // );

    let particle_position = (_particle_position + view.offset) * view.zoom;
    let particle_radius = _particle_radius * view.zoom * 1.7;
    pos = pos * particle_radius + particle_position;

    var result: VertexOutput;
    result.velocity = particle_velocity;
    result.radius = particle_radius;
    result.position = particle_position;
    result.coord_in = vec4<f32>((pos * 500) / screen_size, 0.0, 1.0);
    return result;
}

@fragment
fn fragment(result: VertexOutput) -> @location(0) vec4<f32> {
    if result.radius == 0.0 {
        discard;
    }

    var uv = (result.coord_in.xy - 0.5 * screen_size);
    let pos = result.position * vec2<f32>(0.5, -0.5) * 500;
    let radius = result.radius * 150;

    let dist = length(uv - pos);
    if dist > radius {
        discard;
        // return vec4<f32>(1.0);
    }

    var color = vec3<f32>(abs(result.velocity * 0.1) * 0.9 + 0.1, 0.1);
    color = aces_tone_map(color);
    return vec4<f32>(color, 1.0);
}


// https://sotrh.github.io/learn-wgpu/intermediate/tutorial13-hdr/#switching-to-hdr
//
// Maps HDR values to linear values
// Based on http://www.oscars.org/science-technology/sci-tech-projects/aces
fn aces_tone_map(hdr: vec3<f32>) -> vec3<f32> {
    let m1 = mat3x3<f32>(
        0.59719, 0.07600, 0.02840,
        0.35458, 0.90834, 0.13383,
        0.04823, 0.01566, 0.83777,
    );
    let m2 = mat3x3<f32>(
        1.60475, -0.10208, -0.00327,
        -0.53108, 1.10813, -0.07276,
        -0.07367, -0.00605, 1.07602,
    );
    let v = m1 * hdr;
    let a = v * (v + 0.0245786) - 0.000090537;
    let b = v * (0.983729 * v + 0.4329510) + 0.238081;
    return clamp(m2 * (a / b), vec3<f32>(0.0), vec3<f32>(1.0));
}
