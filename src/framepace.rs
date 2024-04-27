use std::time::{Duration, Instant};

pub struct Framepacer {
    instants: [Instant; 2],
    current: usize,
}

impl Framepacer {
    pub fn new() -> Self {
        Self {
            instants: [Instant::now(); 2],
            current: 0,
        }
    }

    pub fn frametime(&self) -> f32 {
        self.instants[self.current].elapsed().as_secs_f32()
    }

    pub fn framerate(&self) -> f32 {
        1.0 / self.frametime()
    }

    pub fn begin_frame(&mut self) {
        self.instants[self.next()] = Instant::now();
    }

    pub fn end_frame(&mut self, limit_frametime: f32) {
        if limit_frametime > f32::EPSILON && limit_frametime.is_finite() {
            const ACCURACY: f32 = 0.0001; // 100 microseconds
            let sleep_time = limit_frametime - self.frametime() - ACCURACY;

            if sleep_time > 0.0 {
                std::thread::sleep(Duration::from_secs_f32(sleep_time));

                while self.frametime() < limit_frametime {
                    std::thread::yield_now();
                }
            }
        }

        self.current = self.next();
    }

    fn next(&self) -> usize {
        (self.current + 1) % 2
    }
}
