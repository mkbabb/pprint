#[cfg(test)]
mod range_tests {
    use pprint::count::DigitCount;
    use rand::{distributions::uniform::SampleUniform, Rng};
    use std::fmt::Display;

    // Generic test function for a range of values with stochastic sampling
    fn test_range<T>(start: T, end: T, n_samples: usize)
    where
        T: DigitCount + Display + PartialOrd + Copy + SampleUniform,
        rand::distributions::Standard: rand::distributions::Distribution<T>,
    {
        let mut rng = rand::thread_rng();

        // Always test the start and end of the range
        assert_eq!(start.len(), start.to_string().len(), "Start: {}", start);
        assert_eq!(end.len(), end.to_string().len(), "End: {}", end);

        // Stochastically sample the range
        for _ in 0..n_samples {
            let sample: T = rng.gen_range(start..=end);
            assert_eq!(sample.len(), sample.to_string().len(), "Sample: {}", sample);
        }
    }

    // Range tests for various integer types
    #[test]
    fn test_i8_range() {
        test_range(i8::MIN, i8::MAX, 100);
    }

    #[test]
    fn test_u8_range() {
        test_range(u8::MIN, u8::MAX, 100);
    }

    #[test]
    fn test_i16_range() {
        test_range(i16::MIN, i16::MAX, 1000);
    }

    #[test]
    fn test_u16_range() {
        test_range(u16::MIN, u16::MAX, 1000);
    }

    #[test]
    fn test_i32_range() {
        test_range(i32::MIN, i32::MAX, 10000);
    }

    #[test]
    fn test_u32_range() {
        test_range(u32::MIN, u32::MAX, 10000);
    }

    #[test]
    fn test_i64_range() {
        test_range(i64::MIN, i64::MAX, 100000);
    }

    #[test]
    fn test_u64_range() {
        test_range(u64::MIN, u64::MAX, 100000);
    }

    #[test]
    fn test_isize_range() {
        test_range(isize::MIN, isize::MAX, 100000);
    }

    #[test]
    fn test_usize_range() {
        test_range(usize::MIN, usize::MAX, 100000);
    }

    #[test]
    fn test_i128_range() {
        test_range(i128::MIN, i128::MAX, 1000000);
    }

    #[test]
    fn test_u128_range() {
        test_range(u128::MIN, u128::MAX, 1000000);
    }
}