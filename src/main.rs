extern crate sdl2;
extern crate image;
extern crate rand;
extern crate flate2;
extern crate bincode;
extern crate rustc_serialize;
extern crate toml;

use std::time;
use std::env;
use rand::{Rand, Rng};
use rand::distributions::{Range, IndependentSample};
use std::thread;
use std::sync::mpsc::{Sender, channel};

use bincode::rustc_serialize::encode_into;

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

fn worker(tx: Sender<Box<[[u32; 3]]>>, config: &Config) {
    let mut rng = rand::thread_rng();
    let range = Range::new(0.0, 1.0);

    let mut data = Buffer::new(config.width as u64, config.height as u64, config.origin, config.zoom);
    let mut samples = vec![(Complex::default(), 0.0)];

    if USE_METROPOLIS {
        samples = build_initial_samples(&data, config.warmup_count);
        warmup(&data, &mut samples);
    }

    let max_limit = config.limits.iter().max().cloned().unwrap();
    let mut current = Vec::with_capacity(max_limit as usize);
    let mut proposed = Vec::with_capacity(max_limit as usize);

    loop {
        data = Buffer::new(config.width as u64, config.height as u64, config.origin, config.zoom);
        for _ in 0..config.batch_steps {
            for &mut (ref mut c, ref mut contrib) in &mut samples {
                for (i, &limit) in config.limits.iter().enumerate() {
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

fn update_texture(width: u32, height: u32,
                  window_width: u32, window_height: u32,
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

    let skip_x = (width / window_width) as usize;
    let skip_y = (height / window_height) as usize;

    // Skip rows and columns in order to down-sample appropriately
    let pix = buffer.chunks(width as usize * skip_y)
        .flat_map(|part| part[..width as usize]
                  .chunks(skip_x)
                  .map(|x| x[0]));

    for (target, elem) in display_buffer.chunks_mut(3).zip(pix) {
        target[0] = gain(elem[0] as f64 / r_max as f64, 0.2);
        target[1] = gain(elem[1] as f64 / g_max as f64, 0.2);
        target[2] = gain(elem[2] as f64 / b_max as f64, 0.2);
    }

    texture.update(None, &display_buffer, window_width as usize * 3).unwrap();
    texture.set_blend_mode(sdl2::render::BlendMode::Blend);
    texture.set_alpha_mod(255);
    renderer.copy(&texture, None, None).unwrap();
    renderer.present();
    renderer.copy(&texture, None, None).unwrap();
}

#[derive(Clone, Copy)]
struct Config {
    limits: [u32; 3],
    width: u32,
    height: u32,
    window_width: u32,
    window_height: u32,
    batch_steps: u32,
    n_threads: u32,
    warmup_count: u32,
    max_batches: Option<u32>,
    origin: Complex,
    zoom: f64,
}

fn main() {
    let start_time = time::SystemTime::now();
    let config = Config {
        limits: [50000, 5000, 500],
        width: 512,
        height: 512,
        window_width: 512,
        window_height: 512,
        batch_steps: 5000,
        n_threads: 4,
        warmup_count: 30,
        max_batches: None,
        origin: Complex::from_floats(-0.45, 0.0),
        zoom: 0.3,
    };

    // let extent = Complex::from_floats(1.5, 1.5);

    // let origin = Complex::from_floats(-0.1592, -1.0317);
    // let zoom = 80.5;
    // let origin = Complex::from_floats(-0.529854097, -0.667968575);
    // let zoom = 80.5;
    // let origin = Complex::from_floats(-0.657560793, 0.467732884);
    // let zoom = 70.5;
    // let origin = Complex::from_floats(-1.185768799, 0.302592593);
    // let zoom = 90.5;
    // let origin = Complex::from_floats(0.443108035, 0.345012263);
    // let zoom = 4000.0;
    // let origin = Complex::from_floats(-0.647663050, 0.380700837);
    // let zoom = 1275.0;

    // let origin = Complex::from_floats(-1.25275, -0.343);
    // let zoom = 350.0;

    let ctx = sdl2::init().unwrap();
    let video_ctx = ctx.video().unwrap();
    let mut event_pump = ctx.event_pump().unwrap();

    let window = video_ctx.window("Warming Up...", config.window_width, config.window_height)
        .position_centered()
        .allow_highdpi()
        .opengl()
        .build()
        .unwrap();

    let mut renderer: Renderer = window.renderer().build().unwrap();

    let mut texture: Texture =
        renderer.create_texture_streaming(sdl2::pixels::PixelFormatEnum::RGB24,
                                          config.window_width,
                                          config.window_height)
        .unwrap();

    let (tx, rx) = channel();

    for _ in 0..config.n_threads {
        let tx = tx.clone();
        thread::spawn(move || worker(tx, &config));
    }

    let mut buffer = vec![[0_u32; 3]; (config.width * config.height) as usize];
    let mut display_buffer = vec![0_u8; (config.window_width * config.window_height) as usize * 3];
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

        if let Some(max_count) = config.max_batches {
            if number_batches >= max_count {
                break 'all;
            }
        }

        if changed {
            changed = false;
            update_texture(config.width, config.height,
                           config.window_width, config.window_height,
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

    let mut image_buffer = vec![0_u8; (config.width * config.height) as usize * 3];
    update_texture(config.width, config.height,
                   config.width, config.height,
                   &mut renderer,
                   &mut texture,
                   &mut image_buffer,
                   &buffer);

    if let Some(fname) = env::args().nth(1) {
        println!("Saving image...");
        image::save_buffer(&fname, &image_buffer, config.width, config.height, image::RGB(8)).unwrap();

        #[derive(RustcEncodable, RustcDecodable)]
        struct RawBuf {
            width: u32,
            height: u32,
            content: Vec<[u32; 3]>,
        }

        let buf = RawBuf {
            width: config.width,
            height: config.height,
            content: buffer,
        };

        println!("Saving raw...");
        let file = std::fs::File::create(&format!("{}.raw", fname)).unwrap();
        let mut e = flate2::write::GzEncoder::new(file, flate2::Compression::Default);
        encode_into(&buf, &mut e, bincode::SizeLimit::Infinite).unwrap();
    }
}
