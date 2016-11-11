extern crate image;
extern crate rand;

use std::path::Path;
use std::ops::{Add, Sub, Mul};
use rand::{Rand, Rng};
use rand::distributions::{IndependentSample, Range};
use std::io::Write;


#[derive(PartialEq, Clone, Copy)]
struct Complex {
    r: f64,
    i: f64,
}


impl Rand for Complex {
    fn rand<R: Rng>(rand: &mut R) -> Self {
        let range = Range::new(-2.0, 2.0);
        Complex {
            r: range.ind_sample(rand),
            i: range.ind_sample(rand),
        }
    }
}

impl Default for Complex {
    fn default() -> Self {
        Complex::from_floats(0.0, 0.0)
    }
}

impl Complex {
    fn from_floats(i: f64, r: f64) -> Self {
        Complex { r: r, i: i }
    }

    fn escaped(&self) -> bool {
        self.r * self.r + self.i * self.i > 4.0
    }
}

impl Mul for Complex {
    type Output = Complex;
    fn mul(self, other: Complex) -> Complex {
        Complex {
            r: self.r * other.r - self.i * other.i,
            i: self.r * other.i + self.i * other.r,
        }
    }
}

impl Add for Complex {
    type Output = Complex;
    fn add(self, other: Complex) -> Complex {
        Complex {
            r: self.r + other.r,
            i: self.i + other.i,
        }
    }
}

impl Sub for Complex {
    type Output = Complex;
    fn sub(self, other: Complex) -> Complex {
        Complex {
            r: self.r - other.r,
            i: self.i - other.i,
        }
    }
}

struct Buffer {
    buffer: Box<[u32]>,
    width: usize,
    height: usize,
    origin: Complex,
    extent: Complex,
}

impl Buffer {
    fn new(width: usize, height: usize) -> Self {
        Buffer {
            buffer: vec![0; width * height].into_boxed_slice(),
            width: width,
            height: height,
            origin: Complex::from_floats(0.0, -0.3),
            extent: Complex::from_floats(1.6, 1.6),
        }
    }

    fn increment(&mut self, point: &Complex) {
        let offset = *point - self.origin;
        let x = ((offset.i / self.extent.i + 1.0) / 2.0 * self.width as f64) as usize;
        let y = ((offset.r / self.extent.r + 1.0) / 2.0 * self.width as f64) as usize;

        if x >= self.width || y >= self.height {
            return;
        }

        self.buffer[x + y * self.width] += 1;
    }
}

fn buddhabrot(buf: &mut Buffer, iterations: usize) {
    let mut positions = Vec::with_capacity(iterations);
    let c = Complex::rand(&mut rand::thread_rng());
    let mut z = Complex::default();

    let mut escaped = 0;
    for _ in 0..iterations {
        z = z * z + c;
        if z.escaped() { escaped += 1; }
        if escaped >= 6 { break; }
        positions.push(z);
    }

    if escaped > 0 {
        for pos in &positions {
            buf.increment(pos);
        }
    }
}


fn main() {
    let (r, g, b) = (20, 200, 2000);
    let width = 2048;
    let height = 2048;
    let n_iters = 1000000;
    let mut red = Buffer::new(width, height);
    let mut green = Buffer::new(width, height);
    let mut blue = Buffer::new(width, height);

    for i in 0..n_iters {
        for _ in 0..(1000/r)+1 {
            buddhabrot(&mut red, r);
        }
        for _ in 0..(1000/g)+1 {
            buddhabrot(&mut green, g);
        }
        for _ in 0..(1000/b)+1 {
            buddhabrot(&mut blue, b);
        }
        if i % 1000 == 0 {
            print!("\r{:3}%", i * 100 / n_iters);
            std::io::stdout().flush().unwrap();
        }
    }

    let zero = 0;
    let r_min = red.buffer.iter().min().unwrap_or(&zero);
    let r_max = red.buffer.iter().max().unwrap_or(&zero);
    let r_range = if r_min == r_max { 1 } else { r_max - r_min };
    let g_min = green.buffer.iter().min().unwrap_or(&zero);
    let g_max = green.buffer.iter().max().unwrap_or(&zero);
    let g_range = if g_min == g_max { 1 } else { g_max - g_min };
    let b_min = blue.buffer.iter().min().unwrap_or(&zero);
    let b_max = blue.buffer.iter().max().unwrap_or(&zero);
    let b_range = if b_min == b_max { 1 } else { b_max - b_min };

    let size = red.buffer.len();
    let mut buffer = Vec::with_capacity(3 * size);
    for i in 0..size {
        buffer.push(((red.buffer[i] - r_min) * 255 / r_range) as u8);
        buffer.push(((green.buffer[i] - g_min) * 255 / g_range) as u8);
        buffer.push(((blue.buffer[i] - b_min) * 255 / b_range) as u8);
    }

    image::save_buffer(Path::new("image.png"),
                       &buffer,
                       red.width as u32,
                       red.height as u32,
                       image::RGB(8))
        .unwrap();
    println!("\r100%");
    println!("R: {} {}\nG: {} {}\nB: {} {}",
             red.buffer.iter().min().unwrap_or(&0),
             red.buffer.iter().max().unwrap_or(&0),
             green.buffer.iter().min().unwrap_or(&0),
             green.buffer.iter().max().unwrap_or(&0),
             blue.buffer.iter().min().unwrap_or(&0),
             blue.buffer.iter().max().unwrap_or(&0));
}
