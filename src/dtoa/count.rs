use crate::dtoa::utils::udivmod_1e19;

pub trait DigitCount {
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool;
}

/// Binary-search digit counting for u64 values.
#[inline]
fn count_u64_digits(n: u64) -> usize {
    if n < 10_000_000_000 {
        if n < 100_000 {
            if n < 100 {
                if n < 10 { 1 } else { 2 }
            } else if n < 1_000 {
                3
            } else if n < 10_000 {
                4
            } else {
                5
            }
        } else if n < 10_000_000 {
            if n < 1_000_000 { 6 } else { 7 }
        } else if n < 100_000_000 {
            8
        } else if n < 1_000_000_000 {
            9
        } else {
            10
        }
    } else if n < 10_000_000_000_000_000 {
        if n < 100_000_000_000_000 {
            if n < 100_000_000_000 {
                11
            } else if n < 1_000_000_000_000 {
                12
            } else if n < 10_000_000_000_000 {
                13
            } else {
                14
            }
        } else if n < 1_000_000_000_000_000 {
            15
        } else {
            16
        }
    } else if n < 1_000_000_000_000_000_000 {
        if n < 100_000_000_000_000_000 { 17 } else { 18 }
    } else if n < 10_000_000_000_000_000_000 {
        19
    } else {
        20
    }
}

macro_rules! impl_digit_count_signed {
    ($($t:ty),*) => {
        $(
            impl DigitCount for $t {
                #[inline]
                fn len(&self) -> usize {
                    if *self == 0 {
                        return 1;
                    }
                    let neg = *self < 0;
                    let n = if neg {
                        (!(*self as u64)).wrapping_add(1)
                    } else {
                        *self as u64
                    };
                    let count = if neg { 1 } else { 0 };
                    count + count_u64_digits(n)
                }

                #[inline]
                fn is_empty(&self) -> bool {
                    false
                }
            }
        )*
    };
}
impl_digit_count_signed!(i8, i16, i32, i64, isize);

macro_rules! impl_digit_count_unsigned {
    ($($t:ty),*) => {
        $(
            impl DigitCount for $t {
                #[inline]
                fn len(&self) -> usize {
                    if *self == 0 {
                        return 1;
                    }
                    count_u64_digits(*self as u64)
                }

                #[inline]
                fn is_empty(&self) -> bool {
                    false
                }
            }
        )*
    };
}
impl_digit_count_unsigned!(u8, u16, u32, u64, usize);

macro_rules! impl_digit_count_128 {
    (signed: $($st:ty),*; unsigned: $($ut:ty),*) => {
        $(
            impl DigitCount for $st {
                #[inline]
                fn len(&self) -> usize {
                    if *self == 0 {
                        return 1;
                    }
                    let neg = *self < 0;
                    let n = if neg {
                        (!(*self as u128)).wrapping_add(1)
                    } else {
                        *self as u128
                    };
                    let mut count = 0;
                    let (n, rem) = udivmod_1e19(n);
                    let rem_len = rem.len();
                    count += rem_len;
                    if n != 0 {
                        count += 19 - rem_len;
                        let (n, rem) = udivmod_1e19(n);
                        count += rem.len();
                        if n != 0 {
                            count += 38 - count + 1;
                        }
                    }
                    count + neg as usize
                }

                #[inline]
                fn is_empty(&self) -> bool {
                    false
                }
            }
        )*
        $(
            impl DigitCount for $ut {
                #[inline]
                fn len(&self) -> usize {
                    if *self == 0 {
                        return 1;
                    }
                    let n = *self as u128;
                    let mut count = 0;
                    let (n, rem) = udivmod_1e19(n);
                    let rem_len = rem.len();
                    count += rem_len;
                    if n != 0 {
                        count += 19 - rem_len;
                        let (n, rem) = udivmod_1e19(n);
                        count += rem.len();
                        if n != 0 {
                            count += 38 - count + 1;
                        }
                    }
                    count
                }

                #[inline]
                fn is_empty(&self) -> bool {
                    false
                }
            }
        )*
    };
}
impl_digit_count_128!(signed: i128; unsigned: u128);
