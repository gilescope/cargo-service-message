#![feature(test)]
extern crate test;

mod service_messages {}

#[cfg(test)]
mod tests {
    use super::*;
    use test::Bencher;

    #[bench]
    fn example_bench_add_two(b: &mut Bencher) {
        b.iter(|| {
            print!("hi");
        });
    }
}
