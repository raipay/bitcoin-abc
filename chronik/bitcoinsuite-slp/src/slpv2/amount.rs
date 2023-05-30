use std::ops::{Add, Sub, AddAssign, SubAssign, Mul, Div, MulAssign, DivAssign};

pub type Int = i64;

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Amount(Int);

impl Amount {
    pub const ZERO: Amount = Amount::new(0);

    pub const fn new(amount: Int) -> Self {
        Amount(amount)
    }

    pub fn from_bytes(bytes: [u8; 6]) -> Self {
        let mut amount8 = [0u8; 8];
        amount8[..6].copy_from_slice(&bytes);
        Amount::new(Int::from_le_bytes(amount8))
    }

    pub const fn int(&self) -> Int {
        self.0
    }
}

impl Add for Amount {
    type Output = Amount;

    fn add(self, rhs: Self) -> Self::Output {
        Amount(self.0 + rhs.0)
    }
}

impl AddAssign for Amount {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0
    }
}

impl Sub for Amount {
    type Output = Amount;

    fn sub(self, rhs: Self) -> Self::Output {
        Amount(self.0 - rhs.0)
    }
}

impl SubAssign for Amount {
    fn sub_assign(&mut self, rhs: Self) {
        self.0 -= rhs.0;
    }
}

impl Mul<Amount> for Int {
    type Output = Amount;

    fn mul(self, rhs: Amount) -> Self::Output {
        Amount(self * rhs.0)
    }
}

impl MulAssign<Int> for Amount {
    fn mul_assign(&mut self, rhs: Int) {
        self.0 *= rhs;
    }
}

impl Div<Int> for Amount {
    type Output = Amount;

    fn div(self, rhs: Int) -> Self::Output {
        Amount(self.0 / rhs)
    }
}

impl DivAssign<Int> for Amount {
    fn div_assign(&mut self, rhs: Int) {
        self.0 /= rhs;
    }
}

impl std::fmt::Display for Amount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        <Int as std::fmt::Display>::fmt(&self.0, f)
    }
}
