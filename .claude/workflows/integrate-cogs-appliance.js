export const meta = {
  name: 'integrate-cogs-appliance',
  description: 'ADR-019 Stage B — make build-green cogs fully functional WITH the V0 appliance: cross-compile, deploy via the cog supervisor, e2e-validate on the live cluster, optimize accelerator routing',
  phases: [
    { title: 'CrossCompile', detail: 'aarch64/armhf build via scripts/build-all-arm.sh → dist/' },
    { title: 'Validate', detail: 'per-cog install→run→assert→remove round-trip on the live cluster' },
    { title: 'Optimize', detail: 'accelerator routing (H8/H10/CPU) + WiFi-coexistence + throughput' },
    { title: 'Gate', detail: 'final functional tally + residual list' },
  ],
}

const REPO = '/home/ruvultra/cognitum-projects/cogs'
const BRANCH = 'integrate-cogs-make-functional'
const LEADER = 'cognitum-v0' // root@ over Tailscale SSH; gateway :9000, pairing_token in /var/lib/cognitum-fleet/appliance.json

const XBUILD = {
  type: 'object',
  required: ['built', 'cogs'],
  properties: {
    built: { type: 'boolean' },
    target: { type: 'string' },
    cogs: { type: 'array', items: { type: 'string' } },
    failed: { type: 'array', items: { type: 'object', properties: { name: { type: 'string' }, reason: { type: 'string' } } } },
  },
}
const VALID = {
  type: 'object',
  required: ['name', 'functional'],
  properties: {
    name: { type: 'string' },
    functional: { type: 'boolean' },
    lifecycle: { type: 'string' }, // install/run/assert/remove outcome
    detail: { type: 'string' },
  },
}
const GATE = {
  type: 'object',
  required: ['functional_count'],
  properties: {
    functional_count: { type: 'integer' },
    residual: { type: 'array', items: { type: 'string' } },
    optimization: { type: 'string' },
    summary: { type: 'string' },
  },
}

// args: optional explicit cog list (the build-green verified set). If omitted,
// the cross-compile agent builds all and reports what succeeded.
const candidateList = Array.isArray(args) ? args : null

// ── Stage 1 — cross-compile for the appliance target ─────────────────────────
phase('CrossCompile')
const xbuild = await agent(
  `Rust workspace ${REPO}, branch ${BRANCH}. The x86 workspace is build-green (ADR-019 Stage A).
Now cross-compile the cogs for the V0 appliance (Raspberry Pi 5 — aarch64-unknown-linux-gnu, and
armhf where dist/arm expects it) using scripts/build-all-arm.sh (Docker-based; read it first).
${candidateList ? `Restrict to these Stage-A-verified cogs: ${candidateList.join(', ')}.` : 'Build all cogs that compiled in Stage A.'}
Honest reporting: return built (did the cross-compile succeed overall), the target triple, the list
of cogs that produced a runnable binary in dist/, and failed[] (cog + reason) for any that didn't
cross-compile (e.g. a dep that's x86-only). Do NOT fake artifacts.`,
  { label: 'cross-compile', phase: 'CrossCompile', schema: XBUILD },
)

const deployable = (xbuild?.cogs || []).filter(Boolean)
log(`cross-compiled ${deployable.length} cogs for ${xbuild?.target || 'aarch64'}`)
if (!deployable.length) {
  return { xbuild, validated: [], gate: { functional_count: 0, summary: 'no cross-compiled cogs to deploy' } }
}

// ── Stage 2 — deploy + e2e validate each cog on the live cluster ─────────────
phase('Validate')
const validated = await pipeline(
  deployable,
  (cog) =>
    agent(
      `Make cog \`${cog}\` fully functional WITH the V0 appliance, end to end on the LIVE cluster.
Access: \`ssh root@${LEADER}\` over Tailscale (and cluster-1/2/3); the cog supervisor is the
cognitum-cog-gateway on ${LEADER}:9000 (pairing_token in /var/lib/cognitum-fleet/appliance.json;
the ADR-220 cog lifecycle endpoints under /api/v1/v0/...). The built binary is in ${REPO}/dist.
Run the round-trip: publish/install \`${cog}\` via the supervisor → configure if it needs args →
run it → assert it reaches \`running\` and emits its expected output/metrics (check logs + status) →
capture any SOTA metric → then remove it (leave the appliance clean). Be honest: functional=true
ONLY if the live install→run→assert round-trip genuinely worked. If it needs an absent
model/hardware/sensor, set functional=false and say why (residual). Return {name:"${cog}", functional,
lifecycle, detail}.`,
      { label: `validate:${cog}`, phase: 'Validate', schema: VALID },
    ),
)

// ── Stage 3 — optimization pass (single agent, reasons over the validated set) ─
phase('Optimize')
const functional = validated.filter((v) => v && v.functional).map((v) => v.name)
const optimize = await agent(
  `On the V0 cluster (ssh root@${LEADER} + cluster-1/2/3), optimize the cogs that validated functional:
${functional.join(', ') || '(none)'}.
For each compute-heavy cog, route it to the right accelerator — H8 (cluster-1/2, MiniLM embedding,
~70/s NPU-bound) vs H10 (v0/cluster-3, LLM) vs CPU — and confirm placement. Confirm the ADR-240
WiFi-coexistence cap (12 dBm) still holds under cog load (no node returns to 31 dBm; CSI capture
unaffected). Record per-cog accelerator assignment + any throughput/p99 number you can capture.
Return a one-paragraph optimization summary.`,
  { label: 'optimize', phase: 'Optimize', schema: { type: 'object', properties: { summary: { type: 'string' } }, required: ['summary'] } },
)

// ── Stage 4 — final functional gate ──────────────────────────────────────────
phase('Gate')
const residual = validated.filter((v) => v && !v.functional).map((v) => `${v.name}: ${v.detail || 'residual'}`)
const gate = {
  functional_count: functional.length,
  residual,
  optimization: optimize?.summary,
  summary: `${functional.length}/${deployable.length} cross-compiled cogs validated functional with the appliance; ${residual.length} residual.`,
}
log(gate.summary)
return { xbuild, validated, gate }
