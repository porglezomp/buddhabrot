extern crate sdl2;
extern crate image;
extern crate rand;

use std::time;
use std::env;
use rand::{Rand, Rng};
use rand::distributions::{Range, IndependentSample};
use std::thread;
use std::sync::mpsc::{Sender, channel};

use sdl2::event::Event;
use sdl2::render::{Texture, Renderer};

mod complex;
mod buffer;

use complex::Complex;
use buffer::Buffer;

const USE_METROPOLIS: bool = true;


fn mutate(value: Complex, zoom: f64) -> Complex {
    let mut rng = rand::thread_rng();

    let angle = Range::new(0.0, 2.0 * std::f64::consts::PI);
    let unit = Range::new(0.0, 1.0);
    let x = rng.gen::<u32>() % 5;

    if x == 0 || !USE_METROPOLIS {
        Complex::rand(&mut rand::thread_rng())
    } else {
        let r1 = 0.0001 / zoom;
        let r2 = 0.1 / zoom;
        let phi = angle.ind_sample(&mut rng);
        let r = r2 * (-(r2 / r1).ln() * unit.ind_sample(&mut rng)).exp();

        value + Complex::from_floats(r * phi.cos(), r * phi.sin())
    }
}

fn evaluate(c: Complex, iterations: u32, orbit: &mut Vec<Complex>) -> Option<u32> {
    orbit.clear();
    let mut z = c;
    for i in 0..iterations {
        orbit.push(z);
        z = z * z + c;
        if z.escaped() {
            return Some(i);
        }
    }
    None
}

fn accept_prob(length: u32,
               current: &Vec<Complex>,
               cur_contrib: f64,
               proposed: &Vec<Complex>,
               prop_contrib: f64)
               -> f64 {

    fn transition_prob(length: f64, from: &Vec<Complex>, to: &Vec<Complex>) -> f64 {
        (1.0 - (length - from.len() as f64) / length) / (1.0 - (length - to.len() as f64) / length)
    }

    // Tx = p(x' -> x)
    let t0 = transition_prob(length as f64, &proposed, &current);
    // Tx' = p(x -> x')
    let t1 = transition_prob(length as f64, &current, &proposed);

    // (Fx' * Tx') / (Fx * Tx)
    ((cur_contrib * t0) / (prop_contrib * t1)).min(1.0)
}

fn find_initial_sample(buf: &Buffer, origin: Complex, rad: f64, depth: u32) -> Option<Complex> {
    if depth > 500 {
        return None;
    }

    let mut rng = rand::thread_rng();
    let mut seed = Complex::default();

    let mut closest = 1e20;
    // TODO: Replace 50K
    let mut orbit = Vec::with_capacity(50000);
    for _ in 0..200 {
        let tmp = Complex::rand(&mut rng) * (rad * 0.5) + origin;
        if let None = evaluate(tmp, 50000, &mut orbit) {
            continue;
        }

        let contrib = orbit.iter().filter(|&&x| buf.check(x)).count();
        if contrib > 0 {
            return Some(tmp);
        }

        for &point in &orbit {
            let d = (point - buf.origin).norm2();
            if d < closest {
                closest = d;
                seed = tmp;
            }
        }
    }

    find_initial_sample(buf, seed, rad / 2.0, depth + 1)
}

fn build_initial_samples(buf: &Buffer, n_samples: u32) -> Vec<(Complex, f64)> {
    let iterations = 50000;
    let mut output = Vec::with_capacity(n_samples as usize);
    let mut orbit = Vec::with_capacity(iterations);
    for _ in 0..n_samples {
        match find_initial_sample(buf, Complex::default(), 2.0, 0) {
            Some(point) => {
                evaluate(point, iterations as u32, &mut orbit);
                let steps = orbit.iter().filter(|&&x| buf.check(x)).count();
                output.push((point, steps as f64 / iterations as f64));
            }
            None => {
                println!("Failed to find an initial sample");
                continue;
            }
        }
    }
    output
}

fn warmup(buf: &Buffer, samples: &mut Vec<(Complex, f64)>) {
    let limit = 50000;
    let range = Range::new(0.0, 1.0);
    let mut rng = rand::thread_rng();

    let mut current = Vec::with_capacity(limit as usize);
    let mut proposed = Vec::with_capacity(limit as usize);
    for _ in 0..10000 {
        for &mut (ref mut c, ref mut contrib) in samples.iter_mut() {
            evaluate(*c, limit, &mut current);
            let c2 = mutate(*c, buf.zoom);

            if let Some(_) = evaluate(c2, limit, &mut proposed) {
                let count = proposed.iter().filter(|x| buf.check(**x)).count();
                if count == 0 {
                    continue;
                }
                let proposed_contrib = count as f64 / limit as f64;

                let alpha = accept_prob(limit, &current, *contrib, &proposed, proposed_contrib);
                if range.ind_sample(&mut rng) < alpha {
                    *c = c2;
                    *contrib = proposed_contrib;
                }
            }
        }
    }
}

fn worker(tx: Sender<Box<[[u32; 3]]>>,
          limits: [u32; 3],
          width: u32,
          height: u32,
          origin: Complex,
          zoom: f64) {
    let mut rng = rand::thread_rng();
    let range = Range::new(0.0, 1.0);

    let mut data = Buffer::new(width as u64, height as u64, origin, zoom);
    let mut samples = vec![(Complex::default(), 0.0)];

    if USE_METROPOLIS {
        samples = build_initial_samples(&data, 30);
        warmup(&data, &mut samples);
    }

    let max_limit = limits.iter().max().cloned().unwrap();
    let mut current = Vec::with_capacity(max_limit as usize);
    let mut proposed = Vec::with_capacity(max_limit as usize);

    loop {
        data = Buffer::new(width as u64, height as u64, origin, zoom);
        for _ in 0..1000 {
            for &mut (ref mut c, ref mut contrib) in &mut samples {
                for (i, &limit) in limits.iter().enumerate() {
                    evaluate(*c, limit, &mut current);
                    let c2 = mutate(*c, data.zoom);

                    if let Some(_) = evaluate(c2, limit, &mut proposed) {
                        let count = proposed.iter().filter(|x| data.check(**x)).count();
                        if count == 0 {
                            continue;
                        }
                        let proposed_contrib = count as f64 / limit as f64;

                        let alpha = accept_prob(limit, &current, *contrib, &proposed, proposed_contrib);
                        if range.ind_sample(&mut rng) < alpha || !USE_METROPOLIS {
                            *c = c2;
                            *contrib = proposed_contrib;
                            for &point in &current {
                                data.increment(i, point);
                            }
                        }
                    }
                }
            }
        }

        match tx.send(data.buffer) {
            Ok(()) => (),
            Err(_) => break,
        }
    }
}

fn update_texture(width: u32,
                  renderer: &mut Renderer,
                  texture: &mut Texture,
                  display_buffer: &mut [u8],
                  buffer: &[[u32; 3]]) {
    fn gain(x: f64, val: f64) -> u8 {
        fn clamp(x: f64) -> u8 {
            match x {
                x if x <= 0.0 => 0,
                x if x >= 255.0 => 255,
                x => x as u8,
            }
        }

        fn bias(x: f64, val: f64) -> f64 {
            if val > 0.0 { x.powf(val.log(0.5)) } else { 0.0 }
        }

        clamp(if x < 0.5 {
            bias(2.0 * x, 1.0 - val)
        } else {
            2.0 - bias(2.0 - 2.0 * x, 1.0 - val)
        } * 256.0)
    }

    let mut r_max = 0;
    let mut g_max = 0;
    let mut b_max = 0;
    for pix in buffer {
        if pix[0] > r_max {
            r_max = pix[0];
        }
        if pix[1] > g_max {
            g_max = pix[1];
        }
        if pix[2] > b_max {
            b_max = pix[2];
        }
    }

    for (target, elem) in display_buffer.chunks_mut(3).zip(buffer.iter()) {
        target[0] = gain(elem[0] as f64 / r_max as f64, 0.2);
        target[1] = gain(elem[1] as f64 / g_max as f64, 0.2);
        target[2] = gain(elem[2] as f64 / b_max as f64, 0.2);
    }

    texture.update(None, &display_buffer, width as usize * 3).unwrap();
    texture.set_blend_mode(sdl2::render::BlendMode::Blend);
    texture.set_alpha_mod(255);
    renderer.copy(&texture, None, None).unwrap();
    renderer.present();
    renderer.copy(&texture, None, None).unwrap();
}

fn main() {
    let start_time = time::SystemTime::now();
    let limits = [50000, 5000, 500];
    let width = 700;
    let height = 700;
    let n_threads = 4;
    // let origin = Complex::from_floats(-0.1592, -1.0317);
    // let origin = Complex::from_floats(0.0, 0.0);
    // let zoom = 0.3;
    // let extent = Complex::from_floats(1.5, 1.5);
    let origin = Complex::from_floats(-1.25275, -0.343);
    let zoom = 350.0;

    let window_width = width;
    let window_height = height;

    let ctx = sdl2::init().unwrap();
    let video_ctx = ctx.video().unwrap();
    let mut event_pump = ctx.event_pump().unwrap();

    let window = video_ctx.window("Warming Up...", window_width, window_height)
        .position_centered()
        .allow_highdpi()
        .opengl()
        .build()
        .unwrap();

    let mut renderer: Renderer = window.renderer().build().unwrap();

    let mut texture: Texture =
        renderer.create_texture_streaming(sdl2::pixels::PixelFormatEnum::RGB24,
                                      window_width,
                                      window_height)
        .unwrap();

    let (tx, rx) = channel();

    for _ in 0..n_threads {
        let tx = tx.clone();
        thread::spawn(move || worker(tx, limits, width, height, origin, zoom));
    }

    let mut buffer = vec![[0_u32; 3]; (width * height) as usize];
    let mut display_buffer = vec![0_u8; (width * height) as usize * 3];
    let mut changed = false;
    let mut number_batches = 0;
    'all: loop {
        let mut count = 0;
        while let Ok(data) = rx.try_recv() {
            for (target, elem) in buffer.iter_mut().zip(data.iter()) {
                target[0] += elem[0];
                target[1] += elem[1];
                target[2] += elem[2];
            }
            changed = true;
            count += 1;
            number_batches += 1;
            if count > 10 {
                break;
            }
        }

        if changed {
            changed = false;
            update_texture(width,
                           &mut renderer,
                           &mut texture,
                           &mut display_buffer,
                           &buffer);
            renderer.window_mut()
                .unwrap()
                .set_title(&format!("{} Batches in {} seconds",
                                    number_batches,
                                    time::SystemTime::now()
                                    .duration_since(start_time)
                                    .unwrap()
                                    .as_secs()))
                .unwrap();
        }

        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => break 'all,
                _ => (),
            }
        }
    }

    if let Some(fname) = env::args().nth(1) {
        image::save_buffer(fname, &display_buffer, width, height, image::RGB(8)).unwrap();
    }
}
