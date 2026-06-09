export const meta = {
  name: 'integrate-cogs-appliance',
  description: 'ADR-019 Stage B (ALL cogs) — every cog functional WITH the V0 appliance: cross-compile all to ARM, e2e-validate EVERY built cog live on the cluster via the cog supervisor, optimize accelerator routing',
  phases: [
    { title: 'CrossCompile', detail: 'build-all-arm.sh → dist/arm/ ; enumerate every cog that built' },
    { title: 'Validate', detail: 'EVERY built cog: install→configure→run→assert→remove on the live cluster' },
    { title: 'Optimize', detail: 'accelerator routing (H8/H10/CPU) + WiFi-coexistence + throughput' },
    { title: 'Gate', detail: 'full functional tally across all cogs + residual list' },
  ],
}

const REPO = '/home/ruvultra/cognitum-projects/cogs'
const BRANCH = 'main'
const LEADER = 'cognitum-v0' // root@ over Tailscale; cog supervisor = cognitum-cog-gateway :9000

const XBUILD = {
  type: 'object', required: ['built', 'total_cogs'],
  properties: {
    total_cogs: { type: 'integer' }, built_count: { type: 'integer' }, target: { type: 'string' },
    built: { type: 'array', items: { type: 'string' } }, // EVERY cog that produced a runnable ARM binary
    failed: { type: 'array', items: { type: 'object', properties: { name: { type: 'string' }, reason: { type: 'string' } } } },
    summary: { type: 'string' },
  },
}
const VALID = {
  type: 'object', required: ['name', 'functional'],
  properties: { name: { type: 'string' }, functional: { type: 'boolean' }, lifecycle: { type: 'string' }, detail: { type: 'string' } },
}

// ── Stage 1 — cross-compile ALL cogs to ARM ──
phase('CrossCompile')
const xbuild = await agent(
  `Rust workspace ${REPO} on branch ${BRANCH}. The cogs under src/cogs/ are individual crates (own Cargo.toml +
cog.toml manifest with a \`binary\` name and config→CLI mapping), cross-compiled to ARM for the appliance/seed via
scripts/build-all-arm.sh (Docker; read it first for the target triple — armv7/armhf and/or aarch64 — and output dir).
Cross-compile EVERY cog. Honest reporting — return: total_cogs (count under src/cogs/), built_count, target,
\`built\`: the FULL list of every cog that produced a runnable ARM binary (not a sample — all of them), and failed[]
(cog + reason) for any that did not cross-compile. Do NOT fake artifacts. If build-all-arm.sh builds them in bulk,
parse its output/dist dir to enumerate exactly which cogs have a binary.`,
  { label: 'cross-compile-all', phase: 'CrossCompile', schema: XBUILD },
)

const built = (xbuild?.built || []).filter(Boolean)
log(`cross-compiled ${xbuild?.built_count}/${xbuild?.total_cogs} cogs for ${xbuild?.target}; live-validating ALL ${built.length}`)
if (!built.length) {
  return { xbuild, validated: [], gate: { summary: `cross-compile: ${xbuild?.built_count || 0}/${xbuild?.total_cogs || '?'} built; nothing to validate live`, residual: xbuild?.failed || [] } }
}

// ── Stage 2 — deploy + e2e validate EVERY built cog on the LIVE cluster ──
phase('Validate')
const validated = await pipeline(
  built,
  (cog) =>
    agent(
      `Validate cog \`${cog}\` end-to-end WITH the V0 appliance on the LIVE cluster.
Access: \`ssh root@${LEADER}\` over Tailscale (+ cluster-1/2/3). Cog supervisor = cognitum-cog-gateway on ${LEADER}:9000
(bearer token in /var/lib/cognitum-fleet/appliance.json; ADR-220 lifecycle endpoints under /api/v1/v0/...). The ARM
binary is in ${REPO}/dist (named per the cog's cog.toml \`binary\`). Read ${REPO}/src/cogs/${cog}/cog.toml for its
config schema.
Round-trip: publish/install \`${cog}\` via the supervisor → configure required args from cog.toml → run → assert it
reaches \`running\` and emits expected output/metrics (status + logs) → capture a SOTA metric → REMOVE it (leave the
appliance clean). functional=true ONLY if the live install→run→assert genuinely worked. If it needs an absent
sensor/model/hardware input it can't get on the appliance, functional=false with the reason (residual). Keep it
efficient — one clean lifecycle. Return {name:"${cog}", functional, lifecycle, detail}.`,
      { label: `validate:${cog}`, phase: 'Validate', schema: VALID },
    ),
)

// ── Stage 3 — optimization pass over all functional cogs ──
phase('Optimize')
const functional = validated.filter((v) => v && v.functional).map((v) => v.name)
const optimize = await agent(
  `On the V0 cluster (ssh root@${LEADER} + cluster-1/2/3), across the ${functional.length} cogs that validated functional:
group them by compute profile and route each to the right accelerator — H8 (cluster-1/2 MiniLM embedding, ~70/s
NPU-bound) / H10 (v0/cluster-3 LLM) / CPU — and state the placement policy. Confirm the ADR-240 WiFi-coexistence cap
(12 dBm) still holds under cog load (no node back to 31 dBm; CSI capture unaffected). Record representative
throughput/p99 numbers. Return a concise optimization summary (placement policy + WiFi-safe confirmation + numbers).`,
  { label: 'optimize', phase: 'Optimize', schema: { type: 'object', required: ['summary'], properties: { summary: { type: 'string' } } } },
)

// ── Stage 4 — full gate ──
phase('Gate')
const residual = validated.filter((v) => v && !v.functional).map((v) => `${v.name}: ${v.detail || 'residual'}`)
const xfail = (xbuild?.failed || []).map((f) => `${f.name}: ${f.reason}`)
const gate = {
  total_cogs: xbuild?.total_cogs,
  cross_compiled: xbuild?.built_count,
  live_validated: built.length,
  functional_count: functional.length,
  residual,
  cross_compile_failed: xfail,
  optimization: optimize?.summary,
  summary: `${xbuild?.built_count}/${xbuild?.total_cogs} cogs cross-compiled to ${xbuild?.target}; ${functional.length}/${built.length} live-validated functional with the appliance; ${residual.length} live-residual; ${xfail.length} failed cross-compile.`,
}
log(gate.summary)
return { xbuild, validated, gate }
