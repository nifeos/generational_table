# Changelog

Dates are UTC. One line per release, what changed and why, so a reader of the diff has the
sentence beside it.

## Unreleased

- The walks return named types, `Values`, `ValuesMut`, `Iter`, `IterMut` and `IterFromSlotIndex`, in
  place of `impl Iterator`, so a caller can keep one in a field. Each implements
  `DoubleEndedIterator` and `FusedIterator`, and each but `IterFromSlotIndex` implements
  `ExactSizeIterator`, counting down from the table's live count. The shared walks implement `Clone`
  for every entry type.
- `Table` implements `IntoIterator` by shared reference, by mutable reference and by value, so
  `for (name, entry) in &table` compiles. The walk by value is the new `IntoIter`.
- `Table::retain` removes every entry a test refuses, as `remove` removes it, and hands the test
  each entry's name. A `no_std` caller had no buffer to hold the names between a walk and the
  removals. A new Kani harness proves it, and the induction harnesses take it as one more
  operation.
- The manifest names the documentation on docs.rs and adds the `no-std::no-alloc` category, which
  fits a table that never allocates.

## 0.1.0-pre.4

- Breaking: the crate removes `Table::live_slot_indices` and makes `Table::slot_index` private. They served a
  parallel pool of entries indexed by table slot, which nothing used. `iter_from_slot_index(0)`
  still yields each live slot index beside its name and entry.

## 0.1.0-pre.3

- Five `Table` methods change name, with no alias for the old spelling: `slot_of` is `slot_index`,
  `live_slots` is `live_slot_indices`, `iter_from` is `iter_from_slot_index` (its parameter is
  `first_slot_index`), `insert_with` is `insert_with_name` and `capacity` is `slot_count`.
  `slot_count` counts live, free and retired slots alike. The `Debug` output names that field
  `slot_count` too, in place of `capacity`.

## 0.1.0-pre.2

- No change to the library. The repository's workflow pins and the mutation-diff time budget moved,
  and the version is the one a release rehearsal names.

## 0.1.0-pre.1

- First version.
- `Table` implements `Debug` and `Clone`, and `EntryName` implements `Hash`.
- `Table` has `insert`, `contains_key`, `iter_mut`, `capacity` and `clear`, and `EntryName`
  implements `PartialOrd` and `Ord` by its packed value.
- Insert scans from a cursor below which no slot is free, rather than from slot 0, so refilling
  a freed slot costs no scan; `cargo bench` reproduces the figures the documentation states.
- `EntryName::from_bits`, `to_bits` and `eq` are `#[inline(always)]`, so an unoptimized build of a
  caller does not pay a call for each of them.
- `Table::get`, `get_mut`, `slot_of` and the name unpacking are `#[inline(always)]`, and `slot_of`
  widens a slot without a checked conversion on a target whose `usize` is 32 bits or more, so an
  unoptimized build of a caller makes fewer calls per lookup.

## BUGS

- The template's lag check guards the CI pin, not this file; a change that matters to a user of
  the crate belongs in a release line here.
