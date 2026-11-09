# The stranger cadence

The release's whole claim is that a stranger, given this repository and nothing else, can build
the crate and run its proofs from the README alone. A real agent runs that check on a
cadence, and the weekly `stranger cadence` job fails when the
last run is past 28 days.

What a run is:

1. Clone the repository fresh.
2. Follow the README and nothing else: build, test, and run the proofs.
3. Note every place the README lied, omitted, or assumed. Those are findings against the README,
   not against the stranger.
4. Record the date on the line below.

Last run: 2026-11-09

## What the last run did

A fresh clone and the README, on a machine that already had Rust and Kani 0.68.0. It followed the
"Building and proving" section command by command.

- `cargo build` and `cargo test` pass: 33 unit tests, 8 documentation tests and 1 compile-fail
  test.
- `cargo kani` ends with `Complete - 9 successfully verified harnesses, 0 failures, 9 total.`
  after about a minute, with the warnings and `UNREACHABLE` checks the README says to expect.
- The README's three falsification commands do what it says. `git apply` of the
  `a_removed_name_never_resolves_again` patch, then `cargo kani --harness
  verification::a_removed_name_never_resolves_again --exact`, ends with `0 successfully verified
  harnesses, 1 failures, 1 total.` and names the failed assertion. `git checkout -- src` leaves
  the tree clean.
- At Rust 1.85, the stated minimum, `cargo test` passes as well.

The run found no gap in the README. It closes the five gaps the run before it recorded: it gives
the build and test commands, puts the Kani install command on one line, spells out the three
commands around a patch, explains a passing run's warnings, and names the job and the commands
that exercise the minimum Rust version.

Result: pass.
