# Changelog

Dates are UTC. One line per release, what changed and why, so a reader of the diff has the
sentence beside it.

## Unreleased

- First version.
- `Table` implements `Debug` and `Clone`, and `EntryName` implements `Hash`.
- `Table` has `insert`, `contains_key`, `iter_mut`, `capacity` and `clear`, and `EntryName`
  implements `PartialOrd` and `Ord` by its packed value.

## BUGS

- The template's lag check guards the CI pin, not this file; a change that matters to a user of
  the crate belongs in a release line here.
