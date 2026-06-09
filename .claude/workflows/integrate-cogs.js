export const meta = {
  name: 'integrate-cogs',
  description: 'Make the cognitum-one/cogs workspace fully functional — build + test green on the integration branch (ADR-019)',
  phases: [
    { title: 'Foundation', detail: 'fix root cognitum lib + workspace blockers; enumerate crates still failing' },
    { title: 'Fix', detail: 'one agent per failing crate — fix compile + test errors' },
    { title: 'Verify', detail: 'adversarial per-crate re-check (cargo check/test -p)' },
    { title: 'Gate', detail: 'full workspace build + test + clippy' },
  ],
}

const REPO = '/home/ruvultra/cognitum-projects/cogs'
const BRANCH = 'integrate-cogs-make-functional'

const FOUNDATION = {
  type: 'object',
  required: ['foundation_builds', 'failing_crates'],
  properties: {
    foundation_builds: { type: 'boolean' },
    actions: { type: 'array', items: { type: 'string' } },
    failing_crates: {
      type: 'array',
      items: {
        type: 'object',
        required: ['name', 'errors'],
        properties: { name: { type: 'string' }, errors: { type: 'string' } },
      },
    },
  },
}
const FIX = {
  type: 'object',
  required: ['name', 'status'],
  properties: {
    name: { type: 'string' },
    status: { type: 'string', enum: ['fixed', 'residual'] },
    detail: { type: 'string' },
  },
}
const VERIFY = {
  type: 'object',
  required: ['name', 'verified'],
  properties: { name: { type: 'string' }, verified: { type: 'boolean' }, detail: { type: 'string' } },
}
const GATE = {
  type: 'object',
  required: ['builds'],
  properties: {
    builds: { type: 'boolean' },
    tests_pass: { type: 'integer' },
    tests_fail: { type: 'integer' },
    residual: { type: 'array', items: { type: 'string' } },
    summary: { type: 'string' },
  },
}

// ── Stage 1 — Foundation (sequential; gates everything) ──────────────────────
phase('Foundation')
const foundation = await agent(
  `Rust workspace at ${REPO}, on git branch ${BRANCH}. The workspace does NOT compile.
Known first blocker: src/storage/postgres.rs:64 uses sqlx::migrate!() pointing at a missing
./migrations directory ("error canonicalizing migration directory ... No such file or directory").

TASK — make the FOUNDATION compile (the root \`cognitum\` lib + the shared crates under crates/):
 1. Fix the migrations/sqlx blocker honestly — restore the migrations/ dir (check git history /
    the optimizer source), or create the correct schema migrations, or guard/repoint the macro.
    Do NOT delete real DB logic to force a build.
 2. Fix any workspace-level Cargo.toml / Cargo.lock / member / feature-flag breakage.
 3. Run \`cargo check --workspace --message-format=short 2>&1\` and parse the output.
 4. Commit foundation fixes (git add -A && git commit) on branch ${BRANCH}.
Return foundation_builds (does the root lib + shared crates now compile?), the actions you took,
and failing_crates: every crate that STILL fails, with a 1-3 line error summary each.
Do NOT edit individual cog crates under src/cogs/ in this stage — foundation only.`,
  { label: 'foundation', phase: 'Foundation', schema: FOUNDATION },
)

const failing = (foundation?.failing_crates || []).filter(Boolean)
log(`foundation_builds=${foundation?.foundation_builds} — ${failing.length} crates still failing`)

if (!failing.length) {
  // Foundation already green everything — go straight to the gate.
  phase('Gate')
  const gate0 = await agent(
    `In ${REPO} (branch ${BRANCH}) run \`cargo build --workspace\`, \`cargo test --workspace\`,
\`cargo clippy --workspace\`. Report builds/tests_pass/tests_fail/residual/summary.`,
    { label: 'workspace-gate', phase: 'Gate', schema: GATE },
  )
  return { foundation, fixed: [], gate: gate0 }
}

// ── Stage 2 + 3 — Fix each failing crate, then adversarially verify ──────────
phase('Fix')
const results = await pipeline(
  failing,
  (crate) =>
    agent(
      `Rust workspace ${REPO}, branch ${BRANCH}. Make crate \`${crate.name}\` fully functional —
it must compile and (if it has tests) its tests must pass.
Error summary from the workspace check:
${crate.errors}
Rules: edit ONLY files under crate \`${crate.name}\`'s own directory. Do NOT stub out or delete
real logic to force a green build. If the fix genuinely requires an unavailable external dep,
hardware, or model artifact, STOP and return status=residual with the reason.
When done: \`cargo check -p ${crate.name}\` (and \`cargo test -p ${crate.name}\` if it has tests).
Commit your change. Return {name:"${crate.name}", status, detail}.`,
      { label: `fix:${crate.name}`, phase: 'Fix', schema: FIX },
    ),
  (fix) =>
    fix && fix.status === 'fixed'
      ? agent(
          `Adversarially verify in ${REPO} (branch ${BRANCH}): run \`cargo check -p ${fix.name}\`
(and \`cargo test -p ${fix.name}\` if it has tests). Be skeptical — report verified=true ONLY if it
genuinely compiles (and tests pass). Return {name:"${fix.name}", verified, detail}.`,
          { label: `verify:${fix.name}`, phase: 'Verify', schema: VERIFY },
        ).then((v) => ({ ...fix, verified: v?.verified === true, verify_detail: v?.detail }))
      : fix,
)

// ── Stage 4 — Workspace gate (sequential) ────────────────────────────────────
phase('Gate')
const gate = await agent(
  `In ${REPO} (branch ${BRANCH}): run \`cargo build --workspace 2>&1\`, then
\`cargo test --workspace 2>&1\`, then \`cargo clippy --workspace 2>&1\`. Give the final verdict:
does the whole workspace build? tests_pass / tests_fail counts; and residual: any crate that still
does not build/test, each with a one-line reason. Return {builds, tests_pass, tests_fail, residual, summary}.`,
  { label: 'workspace-gate', phase: 'Gate', schema: GATE },
)

const fixed = results.filter(Boolean)
const verified = fixed.filter((f) => f.verified)
log(`fixed=${fixed.length} verified=${verified.length} | workspace builds=${gate?.builds}`)
return { foundation, fixed, verified_count: verified.length, gate }
