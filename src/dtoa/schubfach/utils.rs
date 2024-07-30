pub(crate) fn umul128_upper64(x: u64, y: u64) -> u64 {
    let p = x as u128 * y as u128;
    (p >> 64) as u64
}

pub(crate) fn umul192_upper64(x: u64, y: u128) -> u64 {
    let mut g0 = x as u128 * y.high() as u128;
    g0 += umul128_upper64(x, y.low()) as u128;
    g0.high()
}

pub(crate) fn umul192_middle64(x: u64, y: u128) -> u64 {
    let g01 = x.wrapping_mul(y.high());
    let g10 = umul128_upper64(x, y.low());
    g01.wrapping_add(g10)
}
