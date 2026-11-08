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

Last run: 2026-11-08

## What the last run did

A fresh clone and the README, on a machine that already had Rust and Kani 0.68.0.

- `cargo build` and `cargo test` pass: 16 unit tests, 4 integration tests and 1 documentation
  test.
- `cargo kani` verifies all five harnesses in about 26 seconds.
- For each of the five patches under `falsifications/`, `git apply` and then
  `cargo kani --harness <name>` fail that harness at its own assertion, and `git checkout -- .`
  leaves the tree clean.

The run found five gaps in the README, all open:

1. It gives no build or test command.
2. It says a patch makes its harness fail but not how to apply it, how to run one harness, or how
   to undo the patch.
3. It does not say that a passing `cargo kani` run still prints compiler warnings, unsupported
   constructs and an "unreachable code" check, which a newcomer can read as a failure.
4. The Kani install command breaks across two lines inside one code span, so it copies badly.
5. It states the minimum Rust version, 1.88, and the run built with a newer toolchain only.

Result: pass, with those gaps.
