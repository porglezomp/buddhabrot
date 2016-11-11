extern crate sdl2;
extern crate image;
extern crate rand;

use std::io::{self, Read, Write};
use std::ops::{Add, Sub, Mul};
use rand::{Rand, Rng};
use rand::distributions::{IndependentSample, Range};
use std::thread;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{Sender, channel};

use sdl2::event::Event;

#[derive(PartialEq, Clone, Copy)]
struct Complex {
    r: f64,
    i: f64,
}


impl Rand for Complex {
    fn rand<R: Rng>(rand: &mut R) -> Self {
        let range = Range::new(-3.5, 3.5);
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
    width: u64,
    height: u64,
    origin: Complex,
    extent: Complex,
}

impl Buffer {
    fn new(width: u64, height: u64, origin: Complex, extent: Complex) -> Self {
        Buffer {
            buffer: vec![0; (width * height) as usize].into_boxed_slice(),
            width: width,
            height: height,
            origin: origin,
            extent: extent,
        }
    }

    fn increment(&mut self, point: &Complex) {
        let offset = *point - self.origin;
        let x = ((offset.i / self.extent.i + 1.0) / 2.0 * self.width as f64) as u64;
        let y = ((offset.r / self.extent.r + 1.0) / 2.0 * self.width as f64) as u64;

        if x >= self.width || y >= self.height {
            return;
        }

        self.buffer[(x + y * self.width) as usize] += 1;
    }
}

fn buddhabrot(buf: &mut Buffer, iterations: u64) {
    let mut positions = Vec::with_capacity(iterations as usize);
    let c = Complex::rand(&mut rand::thread_rng());
    let mut z = Complex::default();

    let mut escaped = 0;
    for _ in 0..iterations {
        z = z * z + c;
        if z.escaped() {
            escaped += 1;
        }
        if escaped >= 2 {
            break;
        }
        positions.push(z);
    }

    if escaped > 0 {
        for pos in &positions {
            buf.increment(pos);
        }
    }
}

fn worker(tx: Sender<Box<[u32]>>,
          limit: u32,
          width: u32,
          height: u32,
          origin: Complex,
          extent: Complex) {
    loop {
        let mut data = Buffer::new(width as u64, height as u64, origin, extent);

        for _ in 0..30000 {
            buddhabrot(&mut data, limit as u64);
        }

        match tx.send(data.buffer) {
            Ok(()) => (),
            Err(_) => break,
        }
    }
}

fn main() {
    let limit = 200;
    let width = 512;
    let height = 512;
    let n_threads = 4;
    let origin = Complex::from_floats(0.0, -1.0);
    let extent = Complex::from_floats(1.5, 1.5);
    // let origin = Complex::from_floats(-0.0443594, -0.9876749);
    // let extent = Complex::from_floats(0.015, 0.015);

    let window_width = width;
    let window_height = height;

    let ctx = sdl2::init().unwrap();
    let video_ctx = ctx.video().unwrap();
    let mut event_pump = ctx.event_pump().unwrap();

    let window = video_ctx.window("", window_width, window_height)
        .position_centered()
        .opengl()
        .build()
        .unwrap();

    let mut renderer: sdl2::render::Renderer = window.renderer().build().unwrap();

    let mut texture: sdl2::render::Texture =
        renderer.create_texture_streaming(sdl2::pixels::PixelFormatEnum::RGB24,
                                      window_width,
                                      window_height)
            .unwrap();

    let (tx, rx) = channel();

    for _ in 0..n_threads {
        let tx = tx.clone();
        thread::spawn(move || worker(tx, limit, width, height, origin, extent));
    }

    let mut buffer = vec![0u32; (width * height) as usize];
    let mut display_buffer = vec![0u8; (width * height) as usize * 3];
    let mut changed = true;
    'all: loop {
        while let Ok(data) = rx.try_recv() {
            for (target, elem) in buffer.iter_mut().zip(data.iter()) {
                *target += *elem;
            }
            changed = true;
        }

        if changed {
            changed = false;

            let min = buffer.iter().min().cloned().unwrap_or(0);
            let max = buffer.iter().max().cloned().unwrap_or(0);
            let range = if min == max { 1 } else { max - min };
            for (target, elem) in display_buffer.chunks_mut(3).zip(buffer.iter()) {
                let x = ((*elem - min) * 255 / range) as u8;
                target[0] = x;
                target[1] = x;
                target[2] = x;
            }

            texture.update(None, &display_buffer, width as usize * 3).unwrap();
            texture.set_blend_mode(sdl2::render::BlendMode::Blend);
            texture.set_alpha_mod(255);
            renderer.copy(&texture, None, None).unwrap();
            renderer.present();
            renderer.copy(&texture, None, None).unwrap();
        }

        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => break 'all,
                _ => (),
            }
        }
    }
}
