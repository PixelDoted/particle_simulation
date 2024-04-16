@group(0)
@binding(0)
var<storage, read> particles: array<Particle>;

@group(0)
@binding(1)
var<storage, read_write> output: Output;

struct Output {
    center_of_mass: vec2<f32>,
    min_position: vec2<f32>,
    max_position: vec2<f32>,
    avg_velocity: vec2<f32>,
}

struct Particle {
    position: vec2<f32>,
    velocity: vec2<f32>,
    radius: f32,
    mass: f32,
}


@compute
@workgroup_size(1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    output.min_position = particles[0].position;
    output.max_position = output.min_position;
    
    var particle_count = 0;
    for (var i = 0u; i < arrayLength(&particles); i++) {
        let particle = particles[i];
        if particle.mass == 0.0 {
            continue;
        }
        
        output.center_of_mass += particle.position;
        output.avg_velocity += particle.velocity;
        
        output.min_position = min(output.min_position, particle.position);
        output.max_position = max(output.max_position, particle.position);
        particle_count += 1;
    }

    output.center_of_mass /= f32(particle_count);
    output.avg_velocity /= f32(particle_count);
}
