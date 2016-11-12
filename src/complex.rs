use std::ops::{Add, Sub, Mul};
use rand::{Rand, Rng};
use rand::distributions::{IndependentSample, Range};

/// A complex number consisting of a real and imaginary component
#[derive(Default, PartialEq, Clone, Copy, Debug)]
pub struct Complex {
    pub r: f64,
    pub i: f64,
}

impl Rand for Complex {
    fn rand<R: Rng>(rand: &mut R) -> Self {
        let range = Range::new(-2.0, 2.0);
        loop {
            let c = Complex {
                r: range.ind_sample(rand),
                i: range.ind_sample(rand),
            };
            if !c.escaped() {
                return c;
            }
        }
    }
}

impl Complex {
    pub fn from_floats(r: f64, i: f64) -> Self {
        Complex { r: r, i: i }
    }

    pub fn escaped(&self) -> bool {
        self.norm2() > 4.0
    }

    pub fn norm2(&self) -> f64 {
        self.r * self.r + self.i * self.i
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

impl Mul<f64> for Complex {
    type Output = Complex;
    fn mul(self, other: f64) -> Complex {
        Complex {
            r: self.r * other,
            i: self.i * other,
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
