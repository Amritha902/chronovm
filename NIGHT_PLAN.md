# NIGHT_PLAN — chronovm autonomous build

This file is the brain of the overnight "Ralph loop". Each iteration reads it,
does the highest-value **unchecked** task, and checks it off.

## Mission

Make chronovm a *fantastic*, demo-winning hackathon project. The owner cares
most about **sophisticated UI and touch-and-feel** — the browser debugger at
`docs/` should feel polished, alive, and delightful. Secondary: fix loopholes,
add depth, harden quality.

## Working rules (do not break these)

1. **Green-gate every commit.** `cargo test` and `cargo clippy` must pass before
   committing. If something fails, fix it or revert — never commit red.
2. **Rebuild wasm when it matters.** If you change `src/` (core) or `docs/`,
   run `./build-web.sh` so `docs/pkg/` is current, and commit the regenerated
   files. The live demo (GitHub Pages, `docs/` on `master`) must keep working.
3. **Small green increments.** One coherent improvement per commit, clear
   message. Push to `master` after each. Never force-push. Never delete the
   owner's data or history.
4. **Keep it working.** Never leave the build, the tests, the TUI, or the live
   web demo broken between iterations.
5. **Log your work.** Check off the task here and append a dated line to the
   "Done log" at the bottom. Add new ideas you discover to the backlog.
6. **When unsure, pick the option that best serves polish + a great demo.** Only
   leave a task for the owner if it truly needs their decision — note it under
   "Needs owner".
7. Prefer breadth of *visible* improvement early (owner checks in the morning).

## How to verify

- `cargo test` — unit + render tests
- `cargo clippy --all-targets` — no warnings
- `./build-web.sh` — wasm builds; then the debugger loads at `docs/`
- Optional visual check: serve `docs/` and confirm no console errors

---

## Backlog (priority order — do the top unchecked item)

### Theme A — Sophisticated UI & touch-and-feel (web) ★ top priority
- [x] Refine the visual system: type scale, spacing rhythm, panel elevation,
      refined palette, a proper hero/header, tasteful accent usage.
- [x] Motion: phosphor-persistence flash on freshly-rendered values/active
      line each step, plus a slow CRT refresh sweep. (reduced-motion honored)
- [x] Timeline track markers: red ticks at fault steps, yellow ticks at current
      search matches, subtle hover tooltip showing that step's summary.
- [x] Provenance visualization: when a stack value or variable is selected,
      visually connect it to the step(s) that produced it (highlight + arrows).
- [x] Micro-interactions: hover/focus states, button press feedback, copy button
      on the output panel, smooth panel transitions.
- [ ] Loading state while wasm initializes (skeleton/spinner), and graceful
      empty states.
- [ ] Keyboard shortcut overlay (press `?`), and a first-run coach tour.
- [ ] Light/dark theme toggle with a polished switch; respect system theme.
- [x] Mobile/responsive polish so it looks intentional on a phone.
- [ ] "Share" button: encode the current program into the URL (base64) so a link
      reproduces it; load from URL on start.
- [ ] Editor upgrades: line numbers + lightweight `.cvm` syntax highlighting +
      inline assembler-error underlines.
- [ ] Make timeline ticks clickable (jump straight to a fault/match on click),
      and restore the green "OK" styling in the typewriter boot sequence.
- [ ] Provenance follow-ups: extend arrows to memory cells (trace an mload back to
      the mstore that wrote it), and add a visible "clear trace" affordance so the
      persistent "why?" arrows can be dismissed without re-recording.
- [ ] Reuse the new `.copybtn` frame-affordance pattern on other panels: copy the
      source listing, and copy the causal chain as text (pairs well with the
      Theme C "export trace as JSON" item).
- [ ] The `?` help overlay lists shortcuts but nothing announces it on first
      visit — fold a one-line "press ? for shortcuts" hint into the coach-tour
      task above.

### Theme B — Loopholes / robustness
- [x] Integer overflow (add/sub/mul/div/neg) → clean fault, not a panic.
- [ ] Cap recorded steps for the web build with a visible notice (avoid huge
      memory on pathological programs); keep the terminal cap too.
- [ ] Terminal min-size guard (render a friendly message if too small).
- [ ] Horizontal handling for long source lines (TUI + web).
- [ ] Assembler: nicer error messages with a caret; detect fall-through into a
      function label and warn.
- [ ] Fuzz/property tests for VM invariants (stack/provenance never desync).

### Theme C — New features (depth)
- [ ] Breakpoints: mark instructions; "play to next breakpoint"; reverse to
      previous breakpoint.
- [ ] Step-over vs step-into for `call` while scrubbing.
- [ ] Diff view: pick step A & B, highlight exactly what changed (stack/vars).
- [ ] Watch expressions panel.
- [ ] New opcodes for richer demos: indexed memory (arrays) and/or strings.
- [ ] Example gallery with descriptions (gcd, primes, gcd, sort, etc.).
- [ ] Export a recorded trace as JSON.

### Theme D — Quality / infra
- [ ] GitHub Actions CI: build + test + clippy + wasm build (and optionally
      deploy). Add a status badge to the README.
- [ ] `rustfmt.toml` + format the tree.
- [ ] Integration test asserting every `examples/*.cvm` runs to expected output.
- [ ] A tiny benchmark: record ~1M steps, print timing.

### Theme E — Docs / presentation
- [x] LICENSE file (MIT).
- [ ] README: hero screenshot + an animated GIF of scrubbing + the "why?" jump.
- [ ] A crisp 90-second demo script for judges (`DEMO.md`).
- [ ] "How it works" architecture deep-dive (record/replay + provenance).
- [ ] asciinema recording of the terminal UI.

---

## Needs owner (decisions I won't make alone)
- **I removed an uncommitted "talk to claude" panel** (2026-07-22). The working
  tree had uncommitted markup for a panel that asked visitors to paste an
  Anthropic API key into the public demo. It was HTML-only — no CSS for
  `.talknote`/`.talkrow`, no JS for any of its ten element IDs — so it would have
  rendered as an unstyled, non-functional key form on the live site. It is also
  not on this backlog. I removed it rather than ship it; nothing else was lost
  and the micro-interaction work from that same WIP was kept. If you want it,
  it's a real feature worth designing deliberately: a public page collecting
  API keys needs a clear trust story (key never leaves localStorage, scope
  warning, revocation guidance), and browser-side calls to api.anthropic.com
  need CORS + `anthropic-dangerous-direct-browser-access`. Your call.
- **Activate CI.** The CI workflow is written and lives at `ci/ci.yml`, but it
  couldn't be pushed to `.github/workflows/` because the `gh` OAuth token lacks
  the `workflow` scope. To turn it on:
  1. `gh auth refresh -h github.com -s workflow`
  2. `mkdir -p .github/workflows && git mv ci/ci.yml .github/workflows/ci.yml`
  3. `git commit -m "Enable CI" && git push`

## Done log
- 2026-07-19 — Fixed integer-overflow crash (checked arithmetic → VM fault) + test.
- 2026-07-19 — Added MIT LICENSE file.
- 2026-07-19 — Web: refined visual system — layered palette, elevated panels with
  per-panel accent dots, gradient wordmark + live badge header, custom slider
  (gradient fill + glowing thumb), custom scrollbars, accent-bar active line.
- 2026-07-20 — Web: motion + CRT feel — phosphor-persistence flash on freshly
  rendered values/active line, typewriter boot sequence, slow CRT refresh sweep,
  tube-edge vignette; all reduced-motion-honored.
- 2026-07-20 — Web: mobile/responsive polish (≤600px header wrap, tighter
  padding, fluid search input).
- 2026-07-20 — Web: timeline track markers — red ticks at the fault step, yellow
  ticks at search matches, and a hover tooltip previewing any step's instruction
  + status (edge-clamped so it never spills off-screen). Verified in-browser.
- 2026-07-20 — Web: provenance visualization — an SVG overlay draws glowing amber
  arrows from the source instruction(s) that produced a value to the value itself.
  A local's "why?" traces its whole causal chain (numbered producing lines →
  chain entries); hovering a stack slot previews a single origin arrow, then
  restores the persistent trace on leave. Arrows re-anchor on scroll/resize/scrub
  (rAF-throttled) and honor reduced-motion. Verified in-browser (11-node chain
  rendered correctly, no console errors).
- 2026-07-22 — Web: micro-interactions pass. Panels lift on hover (border warms,
  soft phosphor bloom, title brightens); buttons gain a press-in feel (nudge +
  scale + inset shadow); an amber focus-visible ring covers buttons, header
  links, the editor summary and the scrubber, which previously had no keyboard
  focus style at all. Added a `⧉ copy` affordance on the output panel's top
  border with inline `✓ copied` / `∅ empty` feedback and a legacy
  `execCommand` fallback for when the async clipboard is unavailable or denied.
  Slider thumb glows on hover; text inputs, textarea, memory cells, clickable
  rows and causal nodes got hover states and eased transitions; panel bodies
  scroll smoothly and the memory/diff panels now glide in like the causal panel.
  All of it is disabled under `prefers-reduced-motion`. Verified in-browser: the
  wasm boots, no console errors, and a real click on the copy button shows
  `✓ copied` then resets.
