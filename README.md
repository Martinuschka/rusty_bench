# rusty_bench

A small interactive Rust CLI benchmark that estimates Pi using a parallel Monte Carlo method.

Features:
- Interactive prompts asking how many correct digits to run for and how many threads to use.
- 0 digits means "run until cancelled" (Ctrl+C).
- Spawns the requested number of worker threads and attempts to pin each worker to a CPU core (best-effort) using the `core_affinity` crate.
- Shows a simple ASCII progress display in the console while running.
- Allows interruption (Ctrl+C) and then returns to the prompts so you can start another run.

Notes and limitations:
- This implementation uses a Monte Carlo estimator and verifies digits using f64 precision (about 15 decimal digits). Asking for more than ~15 digits is not meaningful here.
- The program is intended as a benchmark / stress test rather than a production-grade Pi digit calculator.

How to build and run:

1. Install Rust (https://rustup.rs/) if you don't have it.
2. Build:

   cargo build --release

3. Run:

   cargo run --release

Dependencies used:
- rand: random number generation for Monte Carlo sampling
- ctrlc: graceful Ctrl+C handling
- core_affinity: best-effort CPU core pinning for worker threads

If you'd like a version that computes digits deterministically (e.g. using arbitrary-precision arithmetic and a convergent algorithm) I can add that as an alternative mode, but it will require bigger dependencies (e.g. rug) and a different approach to parallelization and verifying digits.
