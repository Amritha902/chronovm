# Example gallery

A set of small, self-contained `.cvm` programs for the chronovm bytecode VM and
its time-travel debugger. Each file opens with a comment explaining what it
computes and what it prints.

## Running

```sh
cargo run -- run   examples/NAME.cvm    # execute and print output
cargo run -- debug examples/NAME.cvm    # step, scrub, and ask "why?" in the debugger
```

Replace `NAME` with any example below.

## Starter examples

| Example | What it does | Output |
| --- | --- | --- |
| [`countdown.cvm`](countdown.cvm) | Counts down from 5 to 1 — the gentlest intro. | `5 4 3 2 1` |
| [`sum_to_n.cvm`](sum_to_n.cvm) | Sums the first n integers (n = 10) in a loop. | `55` |
| [`power.cvm`](power.cvm) | Integer power `base ^ exp` by repeated multiplication (2 ^ 10). | `1024` |
| [`gcd.cvm`](gcd.cvm) | Greatest common divisor by Euclid's subtraction method (gcd 48, 18). | `6` |
| [`collatz.cvm`](collatz.cvm) | Collatz sequence from n = 7, printing every term until it hits 1. | `7 22 11 34 17 52 26 13 40 20 10 5 16 8 4 2 1` |

## Debugger showpieces

| Example | What it does | Output |
| --- | --- | --- |
| [`factorial.cvm`](factorial.cvm) | Iterative `n!` (n = 5); flagship demo for the causal "why?" jump. | `120` |
| [`fib.cvm`](fib.cvm) | Prints the first 10 Fibonacci terms with two rolling variables. | `0 1 1 2 3 5 8 13 21 34` |
| [`recursive.cvm`](recursive.cvm) | Recursive factorial (n = 5); showpiece for the call-stack panel. | `120` |
| [`buggy.cvm`](buggy.cvm) | Divides 100 by a divisor counting down to 0 — walks into a division-by-zero fault on purpose. Scrub back to catch the moment before it faults. | `33 50 100` then faults |

New to the debugger? Start with `countdown.cvm`, then open `factorial.cvm` in
`debug` mode: park at the end, select `acc`, and press `w` to walk the causal
chain that built the final value.
