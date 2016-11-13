use std::fs::File;
use std::io::Read;
use std::env;

use toml;
use num_cpus;

use complex::Complex;

#[derive(Clone)]
pub struct Config {
    pub use_metropolis: bool,
    pub limits: [u32; 3],
    pub width: u32,
    pub height: u32,
    pub window_width: u32,
    pub window_height: u32,
    pub batch_steps: u32,
    pub n_threads: u32,
    pub warmup_count: u32,
    pub max_batches: Option<u32>,
    pub origin: Complex,
    pub zoom: f64,
    pub fname: Option<String>,
    pub save_raw: bool,
}

pub fn get_config() -> Config {
    fn get_conf(path: &str) -> Result<toml::Table, ()> {
        let mut text = String::new();
        if let Ok(ref mut f) = File::open(path) {
            if f.read_to_string(&mut text).is_err() {
                return Err(());
            }
        } else {
            return Err(());
        }
        toml::Parser::new(&text).parse().ok_or(())
    }

    let default = toml::Table::new();
    let conf = if let Some(fname) = env::args().nth(1) {
        get_conf(&fname).unwrap_or(default)
    } else {
        default
    };

    fn get_u32(table: &toml::Table, key: &str, val: u32) -> u32 {
        table.get(key).and_then(Value::as_integer).unwrap_or(val as i64) as u32
    }

    fn get_f64(table: &toml::Table, key: &str, val: f64) -> f64 {
        table.get(key).and_then(Value::as_float).unwrap_or(val)
    }

    use toml::Value;
    let keys = [
        "use_metropolis",
        "red_limit",
        "green_limit",
        "blue_limit",
        "width",
        "height",
        "window_width",
        "window_height",
        "batch_steps",
        "n_threads",
        "warmup_count",
        "max_batches",
        "r",
        "i",
        "zoom",
        "fname",
        "save_raw",
    ];

    for key in conf.keys() {
        if !keys.contains(&&key[..]) {
            println!("Unrecognized key `{}` in config.", key);
        }
    }

    Config {
        use_metropolis: conf.get("use_metropolis").and_then(Value::as_bool).unwrap_or(true),
        limits: [
            get_u32(&conf, "red_limit", 50000),
            get_u32(&conf, "green_limit", 5000),
            get_u32(&conf, "blue_limit", 500),
        ],
        width: get_u32(&conf, "width", 512),
        height: get_u32(&conf, "height", 512),
        window_width: get_u32(&conf, "window_width", 512),
        window_height: get_u32(&conf, "window_height", 512),
        batch_steps: get_u32(&conf, "batch_steps", 5000),
        n_threads: get_u32(&conf, "n_threads", num_cpus::get() as u32),
        warmup_count: get_u32(&conf, "warmup_count", 10),
        max_batches: conf.get("max_batches").and_then(Value::as_integer).map(|x| x as u32),
        origin: Complex::from_floats(
            get_f64(&conf, "r", -0.4),
            get_f64(&conf, "i", 0.0),
        ),
        zoom: get_f64(&conf, "zoom", 0.35),
        fname: conf.get("fname").and_then(Value::as_str).map(String::from),
        save_raw: conf.get("save_raw").and_then(Value::as_bool).unwrap_or(false),
    }
}
