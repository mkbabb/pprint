use crate::dtoa::utils::udivmod_1e19;

pub trait DigitCount {
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool;
}

macro_rules! impl_digit_count {
    ($($t:ty),*) => {
        $(
            impl DigitCount for $t {
                #[inline]
                fn len(&self) -> usize {
                    if *self == 0 {
                        return 1;
                    }

                    let neg = *self < 0;

                    let mut n = {
                        if neg {
                            (!(*self as u64)).wrapping_add(1)
                        } else {
                            *self as u64
                        }
                    };

                    let mut count = if neg { 1 } else { 0 };

                    // log10 approach:
                    // return count + 1 + (n as f64).log10().floor() as usize;


                    // if n >= 10_000_000_000_000_000_000 { return count + 20; }
                    // if n >= 1_000_000_000_000_000_000 { return count + 19; }
                    // if n >= 100_000_000_000_000_000 { return count + 18; }
                    // if n >= 10_000_000_000_000_000 { return count + 17; }
                    // if n >= 1_000_000_000_000_000 { return count + 16; }
                    // if n >= 100_000_000_000_000 { return count + 15; }
                    // if n >= 10_000_000_000_000 { return count + 14; }
                    // if n >= 1_000_000_000_000 { return count + 13; }
                    // if n >= 100_000_000_000 { return count + 12; }
                    // if n >= 10_000_000_000 { return count + 11; }
                    // if n >= 1_000_000_000 { return count + 10; }
                    // if n >= 100_000_000 { return count + 9; }
                    // if n >= 10_000_000 { return count + 8; }
                    // if n >= 1_000_000 { return count + 7; }
                    // if n >= 100_000 { return count + 6; }
                    // if n >= 10_000 { return count + 5; }
                    // if n >= 1_000 { return count + 4; }
                    // if n >= 100 { return count + 3; }
                    // if n >= 10 { return count + 2; }
                    // count + 1

                    // Binary search
                    if n < 10_000_000_000 {
                        if n < 100_000 {
                            if n < 100 {
                                if n < 10 { count + 1 }
                                else { count + 2 }
                            } else {
                                if n < 1_000 { count + 3 }
                                else if n < 10_000 { count + 4 }
                                else { count + 5 }
                            }
                        } else {
                            if n < 10_000_000 {
                                if n < 1_000_000 { count + 6 }
                                else { count + 7 }
                            } else {
                                if n < 100_000_000 { count + 8 }
                                else if n < 1_000_000_000 { count + 9 }
                                else { count + 10 }
                            }
                        }
                    } else {
                        if n < 10_000_000_000_000_000 {
                            if n < 100_000_000_000_000 {
                                if n < 100_000_000_000 { count + 11 }
                                else if n < 1_000_000_000_000 { count + 12 }
                                else if n < 10_000_000_000_000 { count + 13 }
                                else { count + 14 }
                            } else {
                                if n < 1_000_000_000_000_000 { count + 15 }
                                else { count + 16 }
                            }
                        } else {
                            if n < 1_000_000_000_000_000_000 {
                                if n < 100_000_000_000_000_000 { count + 17 }
                                else { count + 18 }
                            } else {
                                if n < 10_000_000_000_000_000_000 { count + 19 }
                                else { count + 20 }
                            }
                        }
                    }

                    // let mut buf = itoa::Buffer::new();
                    // let s = buf.format(*self);
                    // s.len()

                    // while n >= 10 {
                    //     n /= 10;
                    //     count += 1;
                    // }
                    // count + 1

                    // let digit_count = int_log10(n) + 1;
                    // if neg { digit_count + 1 } else { digit_count }
                }

                #[inline]
                fn is_empty(&self) -> bool {
                    false
                }
            }
        )*
    };
}
impl_digit_count!(i8, i16, i32, i64, isize, u8, u16, u32, u64, usize);

macro_rules! impl_digit_count_128 {
    ($($t:ty),*) => {
        $(
            impl DigitCount for $t {
                #[inline]
                fn len(&self) -> usize {
                    if *self == 0 {
                        return 1;
                    }

                    let neg = *self < 0;

                    let n = {
                        if neg {
                            (!(*self as u128)).wrapping_add(1)
                        } else {
                            *self as u128
                        }
                    };

                    let mut count = 0;

                    let (n, rem) = udivmod_1e19(n);
                    let len = rem.len();

                    count += len;

                    if (n != 0) {
                        let leading_zeros = 19 - len;
                        count += leading_zeros;

                        let (n, rem) = udivmod_1e19(n);
                        let len = rem.len();

                        count += len;

                        if (n != 0) {
                            let leading_zeros = 38 - count;
                            count += leading_zeros + 1;
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
    };
}
impl_digit_count_128!(i128, u128);
