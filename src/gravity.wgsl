@group(0)
@binding(0)
var<storage, read> particles: array<Particle>;

@group(0)
@binding(1)
var<storage, read_write> output: array<Particle>;

@group(0)
@binding(2)
var<uniform> params: PhysicsParams;

struct PhysicsParams {
    delta_time: f32,
    gravitational_constant: f32,
}

struct Particle {
    position: vec2<f32>,
    velocity: vec2<f32>,
    radius: f32,
    mass: f32,
}

@compute
@workgroup_size(64)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let total_particles = arrayLength(&particles);
    let index = global_id.x;
    if index >= total_particles {
        return;
    }

    var current = particles[index];
    if current.mass == 0.0 {
        output[index] = current;
        return;
    }

    let pre_vel = current.velocity;
    var offset = vec2<f32>(0.0);

    var i: u32 = 0;
    loop {
        if i >= total_particles {
            break;
        }
        if i == index {
            continue;
        }
    
        let other = particles[i];
        if other.mass == 0.0 {
            continue;
        }

        let oc = other.position - current.position;
        let rr = current.radius + other.radius;
        let oc_sqr_len = dot(oc, oc);
        let oc_len = sqrt(oc_sqr_len);
        let normal = oc / oc_len;
        if oc_len == 0.0 {
            continue;
        }
        
        // Newtonian
        let force = current.mass * other.mass / max(oc_sqr_len, 0.5) * params.gravitational_constant;
        current.velocity += normal * force;

        continuing {
            i = i + 1u;
        }
    }
    
    current.position += current.velocity * params.delta_time;
    output[index] = current;
}
