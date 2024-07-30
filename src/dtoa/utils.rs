const TEN_19: u64 = 10_000_000_000_000_000_000_u64;

/// Multiply unsigned 128 bit integers, return upper 128 bits of the result
#[inline]
#[cfg_attr(feature = "no-panic", no_panic)]
pub fn u128_mulhi(x: u128, y: u128) -> u128 {
    let x_lo = x as u64;
    let x_hi = (x >> 64) as u64;
    let y_lo = y as u64;
    let y_hi = (y >> 64) as u64;

    // handle possibility of overflow
    let carry = (x_lo as u128 * y_lo as u128) >> 64;
    let m = x_lo as u128 * y_hi as u128 + carry;
    let high1 = m >> 64;

    let m_lo = m as u64;
    let high2 = (x_hi as u128 * y_lo as u128 + m_lo as u128) >> 64;

    x_hi as u128 * y_hi as u128 + high1 + high2
}

/// Divide `n` by 1e19 and return quotient and remainder
///
/// Integer division algorithm is based on the following paper:
///
///   T. Granlund and P. Montgomery, “Division by Invariant Integers Using Multiplication”
///   in Proc. of the SIGPLAN94 Conference on Programming Language Design and
///   Implementation, 1994, pp. 61–72
///
#[inline]
#[cfg_attr(feature = "no-panic", no_panic)]
pub fn udivmod_1e19(n: u128) -> (u128, u64) {
    let quot = if n < 1 << 83 {
        ((n >> 19) as u64 / (TEN_19 >> 19)) as u128
    } else {
        u128_mulhi(n, 156927543384667019095894735580191660403) >> 62
    };

    let rem = (n - quot * TEN_19 as u128) as u64;

    debug_assert_eq!(quot, n / TEN_19 as u128);
    debug_assert_eq!(rem as u128, n % TEN_19 as u128);

    (quot, rem)
}

const POWERS_OF_10: [u64; 20] = [
    1,
    10,
    100,
    1000,
    10000,
    100000,
    1000000,
    10000000,
    100000000,
    1000000000,
    10000000000,
    100000000000,
    1000000000000,
    10000000000000,
    100000000000000,
    1000000000000000,
    10000000000000000,
    100000000000000000,
    1000000000000000000,
    10000000000000000000,
];

#[inline]
const fn int_pow(base: u64, mut n: u32) -> u64 {
    let mut result = 1;
    let mut base = base;
    while n != 0 {
        if n & 1 == 1 {
            result *= base;
        }
        base *= base;
        n >>= 1;
    }
    result
}

#[inline]
const fn int_log2(mut x: u64) -> u32 {
    let mut result = 0;
    while x != 0 {
        x >>= 1;
        result += 1;
    }
    result - 1
}

#[inline]
const fn int_log10(x: u64) -> usize {
    let approx = ((int_log2(x) + 1) * 1233) >> 12;
    approx as usize - (x < POWERS_OF_10[approx as usize]) as usize
}
