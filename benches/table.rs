//! The measurements behind the figures the README and the crate documentation state. Run with
//! `cargo bench`; it needs no dependency beyond the standard library.
//!
//! Each workload runs for a fixed number of operations, seven times over, and the fastest of the
//! seven is reported in nanoseconds per operation: the fastest run is the one the rest of the
//! machine disturbed least. `black_box` keeps the compiler from folding the work away.
//!
//! The workloads:
//!
//! - insert and remove one entry in an empty table, at N of 16 and 4,096: the cycle a slot repeats
//!   until it retires, so it sets how fast a slot can retire;
//! - `get` of a live name in a full table of 4,096;
//! - insert into a table of 16 or 4,096 whose only free slot is the last, then remove that entry
//!   again: the first-fit scan at its longest.
//!
//! The same workloads also run against a second layout kept only here, one array of
//! `(generation, Option<T>)` pairs in place of the crate's two arrays, so the choice between the
//! two layouts can be measured again on any machine.

use std::hint::black_box;
use std::time::Instant;

use generational_table::{EntryName, Table};

const RUNS: usize = 7;

/// The fastest of `RUNS` runs of `operations` calls of `step`, in nanoseconds per call.
fn nanoseconds_per_operation(operations: u32, mut step: impl FnMut()) -> f64 {
    let mut fastest = f64::INFINITY;
    for _ in 0..RUNS {
        let start = Instant::now();
        for _ in 0..operations {
            step();
        }
        let elapsed = start.elapsed().as_nanos() as f64 / f64::from(operations);
        fastest = fastest.min(elapsed);
    }
    fastest
}

/// The table's operations the workloads need, so each workload runs against both layouts.
trait Layout<const N: usize>: Sized {
    fn empty() -> Box<Self>;
    fn insert(&mut self, value: u64) -> Option<u64>;
    fn get(&self, name: u64) -> Option<&u64>;
    fn remove(&mut self, name: u64) -> Option<u64>;
}

impl<const N: usize> Layout<N> for Table<u64, N> {
    fn empty() -> Box<Self> {
        Box::new(Table::new())
    }
    fn insert(&mut self, value: u64) -> Option<u64> {
        Table::insert(self, value).map(EntryName::to_bits)
    }
    fn get(&self, name: u64) -> Option<&u64> {
        Table::get(self, EntryName::from_bits(name))
    }
    fn remove(&mut self, name: u64) -> Option<u64> {
        Table::remove(self, EntryName::from_bits(name))
    }
}

/// The alternative layout: each slot's generation beside its entry, in one array. It keeps the
/// crate's bookkeeping (the live count, the walk bound and the scan cursor) and names exactly as
/// the crate names, so only the layout differs.
struct Interleaved<const N: usize> {
    slots: [(u32, Option<u64>); N],
    live: usize,
    top: usize,
    scan_from: usize,
}

impl<const N: usize> Layout<N> for Interleaved<N> {
    fn empty() -> Box<Self> {
        Box::new(Interleaved {
            slots: [(0, None); N],
            live: 0,
            top: 0,
            scan_from: 0,
        })
    }
    fn insert(&mut self, value: u64) -> Option<u64> {
        let found = self
            .slots
            .iter()
            .enumerate()
            .skip(self.scan_from)
            .find_map(|(slot, (g, s))| (s.is_none() && *g != u32::MAX).then_some(slot));
        let Some(slot) = found else {
            self.scan_from = N;
            return None;
        };
        let entry = &mut self.slots[slot];
        entry.1 = Some(value);
        self.live += 1;
        self.scan_from = slot + 1;
        if slot >= self.top {
            self.top = slot + 1;
        }
        Some((u64::from(entry.0) << 32) | slot as u64)
    }
    fn get(&self, name: u64) -> Option<&u64> {
        let slot = usize::try_from(name as u32).ok()?;
        let (generation, entry) = self.slots.get(slot)?;
        if *generation != (name >> 32) as u32 {
            return None;
        }
        entry.as_ref()
    }
    fn remove(&mut self, name: u64) -> Option<u64> {
        self.get(name)?;
        let slot = name as u32 as usize;
        let entry = &mut self.slots[slot];
        let value = entry.1.take();
        entry.0 = entry.0.saturating_add(1);
        self.live -= 1;
        self.scan_from = self.scan_from.min(slot);
        if slot + 1 == self.top {
            let mut top = self.top;
            for _ in 0..N {
                if top == 0 || self.slots[top - 1].1.is_some() {
                    break;
                }
                top -= 1;
            }
            self.top = top;
        }
        value
    }
}

fn insert_and_remove<L: Layout<N>, const N: usize>() -> f64 {
    let mut table = L::empty();
    nanoseconds_per_operation(10_000_000, || {
        let name = table.insert(black_box(7)).unwrap();
        black_box(table.remove(black_box(name)));
    })
}

fn lookup_in_a_full_table<L: Layout<N>, const N: usize>() -> f64 {
    let mut table = L::empty();
    let names: Vec<u64> = (0..N as u64).map(|i| table.insert(i).unwrap()).collect();
    let mut i = 0;
    nanoseconds_per_operation(10_000_000, || {
        // A stride coprime with the power-of-two N, so successive lookups land far apart.
        i = (i + 2_741) % N;
        black_box(table.get(black_box(names[i])));
    })
}

fn insert_into_a_nearly_full_table<L: Layout<N>, const N: usize>() -> f64 {
    let mut table = L::empty();
    for i in 0..N as u64 - 1 {
        table.insert(i).unwrap();
    }
    nanoseconds_per_operation(200_000, || {
        let name = table.insert(black_box(7)).unwrap();
        black_box(table.remove(black_box(name)));
    })
}

fn report<const N: usize>(workload: &str, crate_layout: f64, interleaved: f64) {
    println!(
        "{workload:<40} N = {N:>5}   two arrays {crate_layout:>8.1} ns   one array {interleaved:>8.1} ns"
    );
}

fn main() {
    report::<16>(
        "insert and remove, empty table",
        insert_and_remove::<Table<u64, 16>, 16>(),
        insert_and_remove::<Interleaved<16>, 16>(),
    );
    report::<4096>(
        "insert and remove, empty table",
        insert_and_remove::<Table<u64, 4096>, 4096>(),
        insert_and_remove::<Interleaved<4096>, 4096>(),
    );
    report::<4096>(
        "get, full table",
        lookup_in_a_full_table::<Table<u64, 4096>, 4096>(),
        lookup_in_a_full_table::<Interleaved<4096>, 4096>(),
    );
    report::<16>(
        "insert and remove, last slot free",
        insert_into_a_nearly_full_table::<Table<u64, 16>, 16>(),
        insert_into_a_nearly_full_table::<Interleaved<16>, 16>(),
    );
    report::<4096>(
        "insert and remove, last slot free",
        insert_into_a_nearly_full_table::<Table<u64, 4096>, 4096>(),
        insert_into_a_nearly_full_table::<Interleaved<4096>, 4096>(),
    );
    println!(
        "size of Table<u64, 4096>: {} bytes; one-array layout: {} bytes",
        size_of::<Table<u64, 4096>>(),
        size_of::<Interleaved<4096>>()
    );
}
