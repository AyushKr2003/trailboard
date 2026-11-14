You are continuing development of Trailboard, a Linux canvas browser (Rust + winit + wgpu + CEF) in this repository.

1. Orient yourself before changing anything:
   - Read CLAUDE.md, then docs/PLAN.md (especially section 6 "Handoff" and the status marks), then the [Unreleased] part of CHANGELOG.md.
   - Run `git status` and `git log --oneline -15`. If the repo has no commits yet, make an initial commit of the current tree first (Conventional Commit subject, no Claude co-author or "Generated with" lines).
   - Run `cargo build` and `cargo test`. If something is broken, fixing it is the task for this session.

2. Pick the work: take the "Next up" item from the Handoff section. If none is listed, take the first ⬜ or 🟡 item in milestone order in docs/PLAN.md. Do one coherent item (or a few small related ones) per session. If it has open design questions, decide sensibly, record the decision in docs/PLAN.md, and only ask me if it is genuinely my call.

3. Build it following CLAUDE.md: crate boundaries, pure logic in scene/storage with unit tests, clean-room rule (never copy Marble code, assets or prompts).

4. Verify before calling it done:
   - `cargo test` passes and `cargo clippy` has no warnings.
   - For behaviour changes, run a scripted end-to-end check with TRAILBOARD_SCRIPT and an isolated XDG_DATA_HOME, and read the log.
   - For visual changes, take a screenshot of the window and look at it.
   - Report honestly what you could not verify.

5. Record progress:
   - Update the status marks in docs/PLAN.md, and rewrite section 6 "Handoff" (date, what was done, next up, notes).
   - Add the change to CHANGELOG.md under [Unreleased].
   - Commit with a Conventional Commit message (no Claude co-author or attribution lines). Do not push.

6. Finish with a short summary: what changed, how it was verified, what's next.
