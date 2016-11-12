use complex::Complex;

pub struct Buffer {
    pub buffer: Box<[u32]>,
    pub width: u64,
    pub height: u64,
    pub origin: Complex,
    pub zoom: f64,
}

impl Buffer {
    pub fn new(width: u64, height: u64, origin: Complex, zoom: f64) -> Self {
        Buffer {
            buffer: vec![0; (width * height) as usize].into_boxed_slice(),
            width: width,
            height: height,
            origin: origin,
            zoom: zoom,
        }
    }

    pub fn project(&self, point: Complex) -> (u64, u64) {
        let size = if self.width > self.height {
            self.height
        } else {
            self.width
        };
        let aspect = self.width as f64 / self.height as f64;
        let offset = point - self.origin;
        let x = ((offset.r * self.zoom + 0.5 * aspect) * size as f64) as u64;
        let y = ((offset.i * self.zoom + 0.5) * size as f64) as u64;
        (x, y)
    }

    pub fn increment(&mut self, point: Complex) -> bool {
        let (x, y) = self.project(point);
        if x >= self.width || y >= self.height {
            return false;
        }

        self.buffer[(x + y * self.width) as usize] += 1;
        true
    }

    pub fn check(&self, point: Complex) -> bool {
        let (x, y) = self.project(point);
        !(x >= self.width || y >= self.height)
    }
}
