/// .NET Framework's System.Random (Knuth's subtractive generator), as in the Skate launcher's NetRandom.
pub struct NetRandom {
    seed: [i32; 56],
    inext: usize,
    inextp: usize,
}

impl NetRandom {
    pub fn new(seed: i32) -> NetRandom {
        let mut sa = [0i32; 56];
        let subtraction = if seed == i32::MIN { i32::MAX } else { seed.abs() };
        let mut mj = 161803398i32.wrapping_sub(subtraction);
        sa[55] = mj;
        let mut mk = 1i32;
        for i in 1..55 {
            let ii = (21 * i) % 55;
            sa[ii] = mk;
            mk = mj.wrapping_sub(mk);
            if mk < 0 { mk = mk.wrapping_add(i32::MAX); }
            mj = sa[ii];
        }
        for _ in 1..5 {
            for i in 1..56 {
                sa[i] = sa[i].wrapping_sub(sa[1 + (i + 30) % 55]);
                if sa[i] < 0 { sa[i] = sa[i].wrapping_add(i32::MAX); }
            }
        }
        NetRandom { seed: sa, inext: 0, inextp: 21 }
    }
    fn internal_sample(&mut self) -> i32 {
        self.inext = if self.inext + 1 >= 56 { 1 } else { self.inext + 1 };
        self.inextp = if self.inextp + 1 >= 56 { 1 } else { self.inextp + 1 };
        let mut r = self.seed[self.inext].wrapping_sub(self.seed[self.inextp]);
        if r == i32::MAX { r -= 1; }
        if r < 0 { r = r.wrapping_add(i32::MAX); }
        self.seed[self.inext] = r;
        r
    }
    pub fn next_double(&mut self) -> f64 { self.internal_sample() as f64 * (1.0 / i32::MAX as f64) }
    pub fn next_raw(&mut self) -> i32 { self.internal_sample() }
    /// Next(max).
    pub fn next(&mut self, max: i32) -> i32 { (self.next_double() * max as f64) as i32 }
    /// Next(min, max), for ranges that fit an int.
    pub fn next_range(&mut self, min: i32, max: i32) -> i32 {
        let range = max as i64 - min as i64;
        if range <= i32::MAX as i64 { return (self.next_double() * range as f64) as i32 + min; }
        ((self.next_double() * range as f64) as i64 + min as i64) as i32
    }
}

#[cfg(test)]
mod tests {
    /// .NET Framework 4.8: new Random(3), NextDouble() and Next(100), from the GdiProbe "random" op.
    #[test]
    fn matches_dotnet() {
        let want = include_str!("../tests/random3.txt");
        let (mut a, mut b) = (super::NetRandom::new(3), super::NetRandom::new(3));
        for line in want.lines() {
            let mut p = line.split(' ');
            let d: f64 = p.next().unwrap().parse().unwrap();
            let n: i32 = p.next().unwrap().trim().parse().unwrap();
            assert_eq!(a.next_double(), d);
            assert_eq!(b.next(100), n);
        }
    }
}
