# The `.cvm` Assembly Language

`.cvm` is the human-readable assembly language for **ChronoVM**, a small
stack-based bytecode virtual machine with a time-travel debugger. This document
is the complete reference for the language: its syntax, execution model, every
instruction, and the fault conditions the VM can raise.

---

## 1. Execution model

ChronoVM is a **stack machine**. Almost every instruction operates on a single
shared **value stack** of integers. Instructions pop their operands off the top
of the stack and push their results back.

Alongside the value stack, execution is organized into **call frames**. Every
`call` creates a new frame with its own private set of **named locals**; a `ret`
(or falling off a function via the caller's flow) discards that frame's locals.
The value stack, by contrast, is shared across all frames — it is how arguments
and return values move between a caller and a callee (see §5).

Values are signed integers. Arithmetic that leaves the representable range
raises an **integer overflow** fault rather than wrapping silently (see §7).

A program starts executing at the **first instruction** in the file and runs
until it executes `halt`, until it **falls off the end** of the program (which
halts cleanly), or until it raises a fault.

---

## 2. Syntax

### Comments

A comment begins with either `;` or `#` and runs to the end of the line.
Comments may sit on their own line or trail an instruction.

```
push 5      ; this is a comment
push 6      # so is this
```

### Instructions

One instruction per line. An instruction is a mnemonic optionally followed by a
single operand (an integer literal, a variable name, or a label, depending on
the instruction). Whitespace around tokens is insignificant, and blank lines are
ignored.

### Labels

A **label** names a position in the program so that jumps and calls can target
it. A label is written as an identifier followed by a colon:

```
loop:
```

A label may appear on its own line or directly before an instruction on the same
line:

```
done: load acc
```

Labels are resolved for the whole program before execution, so you may reference
a label that is defined later in the file (forward jumps and forward calls are
fine).

### Integer literals

Integers are written in decimal and may be negative:

```
push 42
push -1
```

### Identifiers

Variable names and labels are identifiers (letters, digits, and underscores,
not starting with a digit). Variable names live in a separate space from labels.

---

## 3. Instruction set

Each instruction is listed with its **stack effect** using the notation
`( before -- after )`, where the **top of the stack is on the right**. For
binary operators, the operands are popped as `b` (top) then `a` (below), and the
result is `a op b` — so for `sub`, `( a b -- a-b )` means the value pushed later
is subtracted from the value pushed earlier.

### 3.1 Stack manipulation

| Instruction | Stack effect | Description |
|-------------|--------------|-------------|
| `push N`    | `( -- N )`   | Push the integer literal `N`. |
| `pop`       | `( a -- )`   | Discard the top value. |
| `dup`       | `( a -- a a )` | Duplicate the top value. |
| `swap`      | `( a b -- b a )` | Exchange the top two values. |

### 3.2 Arithmetic

Binary operators pop `b` then `a` and push `a op b`.

| Instruction | Stack effect | Description |
|-------------|--------------|-------------|
| `add`       | `( a b -- a+b )` | Addition. |
| `sub`       | `( a b -- a-b )` | Subtraction. |
| `mul`       | `( a b -- a*b )` | Multiplication. |
| `div`       | `( a b -- a/b )` | Integer division (truncates toward zero). Faults if `b == 0`. |
| `mod`       | `( a b -- a%b )` | Remainder. Faults if `b == 0`. |
| `neg`       | `( a -- -a )`    | Arithmetic negation. |

### 3.3 Comparison and logic

Comparison and logic instructions push `1` for true and `0` for false.

| Instruction | Stack effect | Description |
|-------------|--------------|-------------|
| `eq`        | `( a b -- a==b )` | Equal. |
| `lt`        | `( a b -- a<b )`  | Less than. |
| `gt`        | `( a b -- a>b )`  | Greater than. |
| `le`        | `( a b -- a<=b )` | Less than or equal. |
| `ge`        | `( a b -- a>=b )` | Greater than or equal. |
| `not`       | `( a -- !a )`     | Pushes `1` if `a == 0`, otherwise `0`. |

### 3.4 Variables (named locals)

| Instruction | Stack effect | Description |
|-------------|--------------|-------------|
| `store NAME`| `( a -- )`   | Pop the top value and store it in the current frame's local `NAME`. |
| `load NAME` | `( -- a )`   | Push the value of the current frame's local `NAME`. Faults if `NAME` is undefined in this frame. |

See §4 for how locals scope to a call frame.

### 3.5 Control flow

| Instruction  | Stack effect | Description |
|--------------|--------------|-------------|
| `jmp LABEL`  | `( -- )`     | Unconditional jump to `LABEL`. |
| `jz LABEL`   | `( a -- )`   | Pop `a`; jump to `LABEL` if `a == 0`. |
| `jnz LABEL`  | `( a -- )`   | Pop `a`; jump to `LABEL` if `a != 0`. |
| `call LABEL` | `( -- )`     | Push a new call frame (with fresh locals) and transfer control to `LABEL`. Arguments are passed on the shared value stack. |
| `ret`        | `( -- )`     | Discard the current frame and return control to the instruction after the matching `call`. Return values are left on the shared value stack. |

### 3.6 I/O and termination

| Instruction | Stack effect | Description |
|-------------|--------------|-------------|
| `print`     | `( a -- )`   | Pop the top value and print it. |
| `halt`      | `( -- )`     | Stop execution immediately. |

### 3.7 Memory

Alongside the value stack and named locals, ChronoVM has a single flat array of
**linear memory** that any frame can read and write.

| Instruction | Stack effect        | Description |
|-------------|---------------------|-------------|
| `mstore`    | `( value addr -- )` | Pop `addr` (top) then `value`; store `value` into memory cell `addr`. |
| `mload`     | `( addr -- value )` | Pop `addr` and push the current contents of memory cell `addr`. |

#### Linear memory

Linear memory is a flat block of integer cells at addresses **`0` through
`65535`**. Every cell is **auto-zeroed** at program start, so `mload` on a cell
that has never been written yields `0` rather than faulting. Unlike named
locals, memory is **not frame-scoped**: it is shared across all call frames and
persists for the whole run, which makes it the natural place to keep arrays and
other data that outlives a single function call.

An address outside `0..=65535` — whether **negative** or **out of range** — is a
fault on both `mstore` and `mload` (see **memory address out of bounds** in §7).

---

## 4. Variables and call frames

Named variables are **locals**: they belong to the call frame that is currently
executing. `store x` writes to `x` in the current frame, and `load x` reads it
back from the current frame.

Because each `call` creates a **new frame with its own independent set of
locals**, a variable named `x` in a callee is completely distinct from a
variable named `x` in the caller. When the callee returns, its locals are
discarded, and the caller's `x` is untouched. There are no global variables:
every named local is frame-scoped.

Reading a local that has never been stored in the current frame is a fault (see
**undefined variable** in §7).

---

## 5. Passing arguments and return values

The **value stack is shared** across all frames, so it is the channel through
which functions receive arguments and hand back results:

1. The caller pushes argument values onto the stack.
2. The caller executes `call f`. A new frame is created; `f` sees the arguments
   sitting on top of the shared stack and can `store` them into its own locals.
3. `f` computes, leaving its result(s) on the shared stack, then executes `ret`.
4. Control resumes in the caller, which finds the return value(s) on top of the
   stack.

By convention you decide how many values a function consumes and produces; the
VM only guarantees that the stack is shared and that locals are not.

---

## 6. A note on `print` and program end

`print` consumes and emits one value at a time, so print each result explicitly.
When execution reaches `halt`, or simply **falls off the end of the program**,
the VM stops cleanly with no fault.

---

## 7. Fault conditions

The VM raises a fault and stops when a program does something invalid. The faults
are:

- **Stack underflow** — an instruction tried to pop a value (or two) when the
  stack did not hold enough. For example, `add` with fewer than two values, or
  `pop` on an empty stack.
- **Division by zero** — `div` with a divisor of `0`.
- **Modulo by zero** — `mod` with a divisor of `0`.
- **Integer overflow** — an arithmetic result (`add`, `sub`, `mul`, `neg`, …)
  fell outside the range of the VM's signed integer type. Results are not
  wrapped silently; the overflow is reported as a fault.
- **Undefined variable** — `load NAME` where `NAME` has not been `store`d in the
  current call frame.
- **Memory address out of bounds** — `mstore` or `mload` with an address outside
  the valid range `0..=65535` (negative or too large).
- **Step limit exceeded** — the VM enforces a maximum number of executed
  instructions to stop runaway loops (for example, a `jmp` loop with no exit).
  When the limit is hit, execution stops with a step-limit fault instead of
  hanging.

Undefined labels (a `jmp`, `jnz`, `jz`, or `call` targeting a label that does
not exist) are rejected before the program runs.

---

## 8. Annotated examples

### 8.1 A loop — iterative factorial

Computes `5!` and prints `120`. `i` runs from `1` up to `n`, multiplying `acc`
each step.

```
    push 5
    store n         ; n = 5
    push 1
    store acc        ; acc = 1
    push 1
    store i          ; i = 1
loop:
    load i
    load n
    le               ; i <= n ?   ( -- 1/0 )
    jz done          ; if not, exit the loop
    load acc
    load i
    mul
    store acc        ; acc = acc * i
    load i
    push 1
    add
    store i          ; i = i + 1
    jmp loop
done:
    load acc
    print            ; prints 120
    halt
```

### 8.2 A conditional — absolute value

Reads a value into `x` and prints its absolute value using `lt` and `jz`.

```
    push -7
    store x
    load x
    push 0
    lt               ; x < 0 ?
    jz nonneg        ; if x >= 0, skip the negation
    load x
    neg              ; x = -x
    jmp show
nonneg:
    load x
show:
    print            ; prints 7
    halt
```

### 8.3 A function call — square via the shared stack

The caller pushes an argument, `call`s `square`, and finds the result on the
stack when `square` returns. Note that `square`'s local `t` lives in its own
frame and does not clash with anything in the caller.

```
    push 6
    call square      ; pass 6 on the shared stack
    print            ; prints 36 (result left on the stack by square)
    halt

square:              ; ( n -- n*n )
    store t          ; pop the argument into this frame's local t
    load t
    load t
    mul              ; n * n
    ret              ; leave the product on the shared stack
```

### 8.4 Linear memory — write a cell, then read it back

Stores `42` into memory cell `100`, reads it back, and prints `42`. Recall that
`mstore` pops the address off the top and the value beneath it.

```
    push 42
    push 100
    mstore           ; mem[100] = 42     ( value=42 addr=100 -- )
    push 100
    mload            ; push mem[100]      ( addr=100 -- 42 )
    print            ; prints 42
    halt
```

An untouched cell reads back as `0` — memory is zero-initialized:

```
    push 7
    mload            ; mem[7] was never written
    print            ; prints 0
    halt
```
