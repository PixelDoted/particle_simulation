@group(0)
@binding(0)
var<storage, read> particles: array<Particle>;

@group(0)
@binding(1)
var<storage, read_write> output: vec2<f32>;

struct Particle {
    position: vec2<f32>,
    velocity: vec2<f32>,
    radius: f32,
    mass: f32,
}


@compute
@workgroup_size(1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    for (var i = 0u; i < arrayLength(&particles); i++) {
        let particle = particles[i];
        output += particle.position * particle.mass;
    }

    output /= f32(arrayLength(&particles));
}
