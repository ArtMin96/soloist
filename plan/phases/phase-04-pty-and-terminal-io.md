# Phase 4 — PTY & Terminal I/O (C3)

**Goal:** Real pseudo-terminals so output renders with full ANSI and the user can **type into** running
processes/agents. Maintain both a **rendered** and a **raw** buffer (ref §7 — MCP needs both), handle
**resize** and **OSC** sequences (titles/bells), and replace Phase 3's pipes with PTYs while keeping the
same status/log contracts.

**Delivers:** C1–C9. **Architecture:** context C3; `ProcessSpawner` PTY methods.

## Why a PTY
Agents/TUIs (Claude Code, vim, htop) check `isatty` and change behavior — colors, cursor control,
interactive prompts. Pipes break this; a PTY makes interactivity (C3) and ANSI (C2) actually work.

## Tasks
1. **PTY backend:** per process, `portable-pty` `openpty(PtySize)`; spawn the command on the slave (still
   in its own process group from Phase 3); keep the master for I/O.
2. **Read loop → dual buffers:** stream raw bytes → `PtyOutput{id,bytes}` event + append to a **raw**
   byte scrollback (bounded, default 256 KB) **and** feed a terminal parser (`vte`) to maintain a
   **rendered** screen/line buffer. Keep the line-oriented `LogLine` stream from Phase 3 for logs/search.
3. **Input (C3):** `write_stdin(id, bytes)` forwards keystrokes/control bytes to the master (bounded mpsc
   = backpressure, `04` §8). Supports raw control bytes (Ctrl-C, arrows) per `send_input`.
4. **Resize (C6):** `resize(id,cols,rows)` → `master.resize()` so the child's `SIGWINCH`/winsize is
   right; UI sends size on attach + container resize.
5. **OSC parsing (C7):** extract OSC title sets and the bell (BEL/OSC) from the stream → emit
   `TerminalTitleChanged` / `TerminalBell` events (Phase 6 notifications + Phase 7 idle heuristics use
   these; ref §6 Codex/Amp/Gemini watch OSC titles).
6. **Supervisor integration:** Phase 3's spawn path now goes through the PTY; status/stop/restart/pgroup
   signaling unchanged; cleanup closes the master in the cancel/`Drop` path (`04` §8).
7. **Detach/attach (C9):** processes keep running with no viewer; attaching replays raw scrollback then
   live-streams; multiple viewers via shared broadcast.
8. **Env hygiene:** `TERM=xterm-256color` (ref §12), sane `LANG`/`COLUMNS`/`LINES`; strip Soloist-
   internal vars except the injected `SOLOIST_PROCESS_ID` (Phase 8).
9. **Frontend terminal:** xterm.js + fit + webgl (canvas fallback) bound to `pty:<id>`; send input via
   `pty_write`; this is consumed by the Phase 5 dashboard.

## Interfaces
```rust
impl Supervisor {
  fn subscribe_pty(&self,id:ProcessId)->Receiver<PtyOutput>;
  fn pty_scrollback(&self,id:ProcessId)->Bytes;            // raw replay
  fn rendered(&self,id:ProcessId)->RenderedScreen;         // for get_process_output
  async fn write_stdin(&self,id:ProcessId,data:Bytes)->Result<()>;
  async fn resize(&self,id:ProcessId,cols:u16,rows:u16)->Result<()>;
}
enum DomainEvent { PtyOutput{id,bytes}, TerminalTitleChanged{id,title}, TerminalBell{id}, … }
```

## Acceptance criteria
- `ls --color=always` shows color bytes; `vim`/`htop`/a real agent render and accept input.
- `bash -c 'read x; echo got=$x'` receives sent bytes and echoes `got=...`; Ctrl-C interrupts a process.
- Resize changes the child's `tput cols`.
- Both buffers are correct: rendered = on-screen text; raw = includes escape sequences.
- An OSC title set emits `TerminalTitleChanged`; a bell emits `TerminalBell`.
- Detaching doesn't kill the process; reattaching replays recent screen then streams.
- All Phase 3 acceptance tests still pass (no status/stop/orphan regression).

## Test plan
- **Integration (headless):** drive PTYs from Rust — assert `read x` echo, color bytes for
  `--color=always`, `resize` via `tput cols`, OSC title/bell emission, raw-vs-rendered divergence.
- **Manual:** attach to `vim`/`htop`/`claude` and interact.

## Risks & mitigations
- **portable-pty on Ubuntu 20.04** → CI matrix includes 20.04; pin a known-good version.
- **Byte vs line vs rendered triality** → one read loop fans out to all three; `LogLine` is best-effort
  line-split; `vte` owns rendered.
- **Scrollback memory** → bounded per-process + global cap (`04` §8).

## Effort
~4–5 days.
