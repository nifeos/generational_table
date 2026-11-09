# generational_table

[![check](https://github.com/nifeos/generational_table/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/nifeos/generational_table/actions/workflows/ci.yml?query=branch%3Amain)
[![proofs](https://github.com/nifeos/generational_table/actions/workflows/proofs.yml/badge.svg?branch=main)](https://github.com/nifeos/generational_table/actions/workflows/proofs.yml?query=branch%3Amain)
[![license](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](https://github.com/nifeos/generational_table#license)
[![minimum Rust version](https://img.shields.io/badge/rust-1.88%2B-blue.svg)](https://github.com/nifeos/generational_table/blob/main/Cargo.toml)

A fixed-capacity table that names each entry with a typed `u64`, so a name outlives nothing it
named. Removing an entry raises its slot's generation, and every copy of the old name then
resolves to `None` for good, whether or not the slot holds a new entry. The table is an array
of `N` slots chosen at compile time: it is `no_std`, needs no allocator, has no dependency, and
forbids `unsafe` code. Game engines call the same shape a slotmap.

Use it where you keep many short-lived records and hold references to them as plain words (in a
packet, a register, an atomic, a file). It suits a program that needs a reference to a dead record
to fail safely rather than dangle or reach somebody else's record. It is not for a collection that must grow
past a capacity known up front; see the comparison below.

## EXAMPLES

```rust
use generational_table::Table;

// Room for four sessions. `insert_with` hands the new entry its own name.
let mut sessions: Table<(u32, &str), 4> = Table::new();
let alice = sessions.insert_with(|name| (name.to_bits() as u32, "alice")).unwrap();
let bob = sessions.insert_with(|_| (0, "bob")).unwrap();
assert_eq!(sessions.get(alice).map(|s| s.1), Some("alice"));
assert_eq!(sessions.len(), 2);

// End Alice's session and let someone else take her slot.
assert!(sessions.remove(alice).is_some());
let carol = sessions.insert_with(|_| (0, "carol")).unwrap();

// Alice's old name does not reach Carol. It reaches nothing.
assert_ne!(alice, carol);
assert!(sessions.get(alice).is_none());
assert_eq!(sessions.get(carol).map(|s| s.1), Some("carol"));
assert_eq!(sessions.get(bob).map(|s| s.1), Some("bob"));

// A name crosses a boundary that carries a plain word, and comes back.
let word: u64 = carol.to_bits();
let again = generational_table::EntryName::<(u32, &str)>::from_bits(word);
assert_eq!(sessions.get(again).map(|s| s.1), Some("carol"));

// A word the table never minted resolves to nothing, whatever it holds.
assert!(sessions.get(generational_table::EntryName::NEVER_MINTED).is_none());
```

`Table::<T, N>::new` is a `const fn`, so a table can sit in a `static`. A name is an
`EntryName<T>`, which packs the generation in the high 32 bits of a `u64` and the slot in the low
32. It carries the entry type `T` as a tag only, so a name from a table of one entry type does not
compile where a function wants another table's name. Two tables of the same entry type share a name
type; give each its own entry type (a one-field wrapper is enough) to keep them apart.

Besides `insert_with`, `get`, `get_mut` and `remove`, the table has `insert` for a value that does
not need its own name, `contains_key`, `slot_of`, `len`, `is_empty` and `capacity` (`N`, for
generic code). `clear` removes every entry and kills every name the table held. The walks are
`values`, `values_mut`, `live_slots`, `iter` and `iter_mut` (each name with its entry), and
`iter_from`, a sweep that resumes from a slot index after you give a lock back. The walks and
`clear` cost what the table holds, not what it could hold.

`Table<T, N>` implements `Clone` when `T: Clone`, and `Debug` when `T: Debug`. A clone resolves
exactly the names the original does. `Debug` prints the length, the capacity and the live entries
only, each under its name, so the output does not grow with `N`. `EntryName<T>` implements `Clone`,
`Copy`, `PartialEq`, `Eq`, `PartialOrd`, `Ord`, `Hash` and `Debug` for every `T`. Names order by
their packed value, generation first and then slot, so they key a `BTreeMap` and sort; the order
says nothing about which entry came first.

## What is proven

Nine Kani harnesses prove the safety claims. Kani is a model checker that proves properties for
all inputs up to a stated size. The proofs need Kani, which `rustup` does not install. Run
`cargo install --locked
kani-verifier --version 0.68.0`, then `cargo kani setup`. The setup step
downloads a model checker, so it needs network access. CI uses version 0.68.0, and another
version may verify differently. After that, `cargo kani` from the crate's directory runs every
harness.

- `a_removed_name_never_resolves_again`: from any starting generations, the last one before
  retirement included, a removed name resolves through none of `get`, `get_mut` and `remove`,
  even after its slot takes a new entry.
- `a_removed_name_stays_dead_whatever_comes_after`: the induction step. From any reachable
  two-slot table, any two operations keep a dead name's slot generation above the name's and keep
  every live slot below the retired generation, so the claim holds after any number of
  operations.
- `live_names_are_distinct_and_resolve_to_their_own_entry`: two live entries never share a name,
  and each name reaches its own value.
- `a_name_the_table_never_minted_resolves_to_nothing`: for any `u64`, resolution fails or lands
  on the entry whose minted name is that number. Nobody can forge a name.
- `a_clone_resolves_exactly_the_same_names`: from any reachable two-slot table, a clone and the
  original resolve any name alike, a forged one included.
- `the_walk_bound_and_the_live_count_survive_any_operation`: the induction step for the walks.
  From any reachable two-slot table, any insert, remove or clear keeps the walk bound exactly one
  past the highest live slot and the live count equal to the occupied slots. So every walk reports
  exactly `len` entries after any number of operations, and `iter_mut` yields the names `iter`
  does.
- `a_cleared_table_resolves_no_name_it_held`: from any reachable two-slot table, `clear` leaves
  nothing to walk, and no name the table held before it resolves, even after the next insert.
- `contains_key_and_get_agree_on_any_name`: for any `u64`, `contains_key` answers exactly what
  `get` answers.
- `a_bounded_walk_yields_every_live_entry`: after removing any subset of three entries, every
  walk, `values_mut` included, reports exactly `len` entries and the survivors still resolve.

Each harness has a falsification record at `falsifications/verification.<harness>.patch`.
Applying it makes that harness fail at its own assertion, which shows the proof checks
something. The proofs use tables of two or three slots and operation sequences of one or two
steps. The host tests cover the rest: a name whose slot is past the table, a retired slot, a fully
retired table, the hole below the walk bound, and an insert the table refuses without running its
closure.

## BUGS

- Capacity is fixed. `insert_with` returns `None`, without running its closure, when every slot
  is live or retired.
- Generations are 32-bit and saturate; they never wrap. A slot whose generation reaches
  `u32::MAX` retires and never takes an entry again, so the table's capacity shrinks by one. A slot
  retires only after 2^32 - 1 occupants. A loop that does nothing but insert and remove one entry
  can get there in about half a minute (about 7 nanoseconds a cycle on a 2.1 gigahertz server core).
  A table whose every slot has retired refuses every insert for good. A program that creates
  and destroys entries without pause should treat a refused insert as a real outcome.
- `insert_with` scans from slot 0 for the first free slot, which is O(N). `get` is O(1).
- Storage is `[Option<T>; N]`, which spends a tag on each slot when `T` has no niche.
- Expected demand is low. Fixed-capacity tables are a small corner of the ecosystem, and this
  crate does not try to be anything else.

## How it differs from slotmap, thunderdome, slots and coca

`slotmap` and `thunderdome` are the established, growable members of this family, and for most
programs one of them is the right choice. They keep their entries in a `Vec`, so they need an
allocator and grow without bound, and neither fixes its capacity at compile time. Their
generations wrap, `slotmap` after 2^31 reuses of a slot and `thunderdome` after 2^32 - 1, so
after that many reuses of one slot an ancient name can resolve to a new occupant. `slotmap` also
lets a plain `u64` convert into a key, and two such words can reach one entry. This crate trades
growth for the opposite properties: a fixed array, no allocation, a `const` constructor, a
generation that retires a slot rather than wrap, names that nobody can forge from a word, and
machine-checked proofs of those claims. It costs you the capacity you choose up front and a slot
now and then in a very long-lived table.

Two crates on crates.io also keep a fixed number of slots in an array, with no allocator. The notes
below come from reading the published source of `slots` 0.4.0 and `coca` 0.3.0. Both versions date
from March 2022.

`slots` is a fixed array of `N` slots with no generations. Its `Slots` type hands out a `Key` that
nobody can copy or clone, and `take` consumes it. The compiler therefore rejects a second use of a
removed key, which this crate checks only at run time. By default a key also records which `Slots`
value made it and panics in another, where this crate lets a name from a second table of the same
entry type resolve there. Its store and take run in constant time from a free list, where this
crate's `insert_with` scans. The price is that you cannot keep a key as a plain word. `Key::index`
gives a `usize`, and `try_read` with that index reaches whatever the slot holds now, a later
occupant included. Its `UnrestrictedSlots` type uses such indexes throughout. Its `new` is not
`const`, it has no `unsafe` code of its own, and it depends on `array-init`. The published source
has unit tests and no proofs or fuzzing.

`coca` is the closer match. Its `DirectInlinePool` and `PackedInlinePool` keep `N` entries inline,
with no allocator unless you turn on its `alloc` feature. Its handles carry a generation, 32 bits in
the default 64-bit handle, and as few as 16 in a 32-bit handle type from its `handle_type!` macro.
That macro also gives each pool its own handle type. The generation wraps. Its documentation says so
and shows a stale 16-bit handle reaching a new value after 32,768 reuses. Safe code cannot build its
default handle from an integer, since the constructor is an `unsafe` function. This crate offers a
safe `from_bits` instead and checks the generation on every lookup. `coca` inserts in constant time,
its `try_insert_with_handle` matches `insert_with`, and its packed pool keeps values in one dense
array. It also offers vectors, deques, maps and arenas. It uses `unsafe` code throughout its pools,
and its changelog cites Miri, the Rust interpreter that detects undefined behavior. Its `new` is not
`const`. The published source has tests and no proofs or fuzzing.

`heapless` 0.9.3 has no generational table. It has fixed-capacity vectors, maps and queues with
`const` constructors, and its object pool hands out owning `Object` pointers rather than integer
names. `generational-arena` 0.2.9 forbids `unsafe` code but keeps its entries in a `Vec`, and
`stable-vec` 0.4.3 keeps a `Vec` with no generations.

## Minimum Rust version

Rust 1.88.

## License

Licensed under either of the MIT license ([LICENSE-MIT](LICENSE-MIT)) or the Apache License,
Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE)), at your option.
