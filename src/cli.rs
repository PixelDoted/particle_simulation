use clap::Parser;

/// A Newtonian Gravity Particle Simulation
#[derive(Parser)]
#[command()]
pub struct Args {
    /// Total Particles, Must be a multiple of `64`
    pub particles: u32,

    /// The framerate the simulation will run at  
    ///
    /// if default the simulation will run as fast as possible  
    /// and capture is disabled
    #[arg(short, long)]
    pub framerate: Option<u32>,

    /// Gravitational Constant
    #[arg(short, long, default_value_t = 0.1f32)]
    pub gravity: f32,
}
