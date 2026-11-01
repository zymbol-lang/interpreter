//! `std/random`'s generator, for both Rust engines: xoshiro256++, its state
//! expanded from one 64-bit seed by splitmix64.
//!
//! Until v0.0.11 each engine had its own copy, seeded from the clock with no
//! way to choose the seed (`AGENTIC.md` G8). `random::semilla(n)` chooses it
//! now (G8, decided by the author on 2026-10-24, option A), and the same seed
//! has to give the same sequence in the tree-walker, the VM and zyjs — which
//! is why there is one implementation here and its mirror, step for step, in
//! `web/src/zymbol/zymbol.js` (`ZyRandom`). Without a seed the first draw takes
//! one from the caller (the clock, or a host a test installs), as before.
//!
//! The state is the process's, as the numeral mode is: a module that calls
//! `semilla` fixes the sequence of whoever imports it too.

use std::cell::Cell;

thread_local! {
    static STATE: Cell<Option<[u64; 4]>> = const { Cell::new(None) };
}

fn splitmix64(x: &mut u64) -> u64 {
    *x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// The four words of xoshiro's state for one seed.
pub fn state_from(seed: u64) -> [u64; 4] {
    let mut x = seed;
    [splitmix64(&mut x), splitmix64(&mut x), splitmix64(&mut x), splitmix64(&mut x)]
}

/// `random::semilla(n)`: the sequence starts again from `n`. A negative `n`
/// is taken as its 64-bit two's complement, as zyjs's `BigInt.asUintN` takes it.
pub fn seed(n: i64) {
    STATE.with(|s| s.set(Some(state_from(n as u64))));
}

/// The next 64 bits. `entropy` is asked for a seed only on the first draw of a
/// process that never called `seed`.
pub fn next(entropy: impl FnOnce() -> u64) -> u64 {
    STATE.with(|s| {
        let mut st = s.get().unwrap_or_else(|| state_from(entropy()));
        let result = st[0].wrapping_add(st[3]).rotate_left(23).wrapping_add(st[0]);
        let t = st[1] << 17;
        st[2] ^= st[0];
        st[3] ^= st[1];
        st[1] ^= st[2];
        st[0] ^= st[3];
        st[2] ^= t;
        st[3] = st[3].rotate_left(45);
        s.set(Some(st));
        result
    })
}

/// `random::entero(lo, hi)`: an Int in `[lo, hi]`, with `hi >= lo`.
pub fn entero(lo: i64, hi: i64, entropy: impl FnOnce() -> u64) -> i64 {
    let range = (hi - lo + 1) as u64;
    lo + (next(entropy) % range) as i64
}

/// `random::rango(n)`: an Int in `[0, n)`, with `n > 0`.
pub fn rango(n: i64, entropy: impl FnOnce() -> u64) -> i64 {
    (next(entropy) % n as u64) as i64
}

/// `random::peso_f64()`: a Float in `[-0.1, 0.1]`, in steps of 0.001.
pub fn peso_f64(entropy: impl FnOnce() -> u64) -> f64 {
    ((next(entropy) % 201) as f64 - 100.0) / 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The first draws after `seed(42)`, which `zymbol.js`'s `ZyRandom` must
    /// give too: the three engines agree on a seeded sequence or the seed
    /// repeats nothing across them.
    #[test]
    fn a_seed_fixes_the_sequence() {
        seed(42);
        let a: Vec<u64> = (0..3).map(|_| next(|| unreachable!())).collect();
        seed(42);
        let b: Vec<u64> = (0..3).map(|_| next(|| unreachable!())).collect();
        assert_eq!(a, b);
        seed(43);
        assert_ne!(a[0], next(|| unreachable!()));
    }

    #[test]
    fn the_ranges_hold() {
        seed(-7);
        for _ in 0..1000 {
            let e = entero(-3, 3, || 0);
            assert!((-3..=3).contains(&e));
            let r = rango(5, || 0);
            assert!((0..5).contains(&r));
            let p = peso_f64(|| 0);
            assert!((-0.1..=0.1).contains(&p));
        }
    }
}
