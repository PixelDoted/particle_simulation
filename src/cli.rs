use clap::Parser;

/// A Newtonian Gravity Particle Simulation
#[derive(Parser)]
#[command()]
pub struct Args {
    /// Total Particles
    #[arg(short, long, default_value_t = 4096)]
    pub particles: u32,

    /// The framerate the simulation will run at  
    ///
    /// if `0` the simulation will run as fast as possible  
    /// and capture is disabled
    #[arg(short, long, default_value_t = 0)]
    pub framerate: u32,

    /// Gravitational Constant
    #[arg(short, long, default_value_t = 0.1f32)]
    pub gravity: f32,

    /// The time scale the simulation runs at
    ///
    /// Note: This WILL effect the simulation
    #[arg(short, long, default_value_t = 1.0/60.0)]
    pub time_scale: f32,
}
