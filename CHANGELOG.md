# Changelog

Dates are UTC. One line per release, what changed and why, so a reader of the diff has the
sentence beside it.

## Unreleased

- First version.
- `Table` implements `Debug` and `Clone`, and `EntryName` implements `Hash`.
- `Table` has `insert`, `contains_key`, `iter_mut`, `capacity` and `clear`, and `EntryName`
  implements `PartialOrd` and `Ord` by its packed value.
- Insert scans from a cursor below which no slot is free, rather than from slot 0, so refilling
  a freed slot costs no scan; `cargo bench` reproduces the figures the documentation states.

## BUGS

- The template's lag check guards the CI pin, not this file; a change that matters to a user of
  the crate belongs in a release line here.
