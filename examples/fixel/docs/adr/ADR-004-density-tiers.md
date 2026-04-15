# ADR-004: Density Tiers - Five Tiers from Nano to Ultra

## Status

**Accepted**

## Date

2026-01-09

## Context

FIXEL targets diverse market segments with varying requirements:

| Segment | Resolution | Power Budget | Use Case |
|---------|------------|--------------|----------|
| Wearables | 640x480 | <1W | AR glasses, smartwatch |
| Mobile | 1920x1080 | 2-5W | Smartphone, tablet |
| Consumer | 3840x2160 | 10-20W | TV, monitor |
| Professional | 7680x4320 | 30-100W | Medical imaging, CAD |
| Datacenter | 15360x8640+ | 200-500W | AI inference rack |

Key considerations:
1. **Cost scaling**: 2nm fabrication costs ~$15K per wafer; per-display cost must scale with market
2. **Thermal limits**: Higher density requires better cooling solutions
3. **Yield optimization**: Larger tiles improve yield but reduce flexibility
4. **Feature parity**: Core programming model should work across tiers
5. **Upgrade path**: Users should be able to migrate between tiers

Manufacturing constraints:
- Minimum economical die size: ~2mm^2 (handling, test)
- Maximum practical tile: ~100mm^2 (yield, thermal)
- Wafer diameter: 300mm
- Edge exclusion: 3mm

## Decision

We will implement **five density tiers** with standardized tile sizes and scaling rules.

### Tier Definitions

```
TIER 1: NANO (Wearables, IoT)
- Target resolution: 640x480 to 1280x720
- Pixel pitch: 50-100 um
- Cognitum transistors: 500K per pixel
- Local SRAM: 128 bytes
- Tile size: 8x8 = 64 pixels
- Power: <0.3 uW per pixel
- Compute: 10 MOps/pixel/second

TIER 2: MINI (Mobile)
- Target resolution: 1920x1080 to 2560x1440
- Pixel pitch: 60-80 um
- Cognitum transistors: 1M per pixel
- Local SRAM: 256 bytes
- Tile size: 16x16 = 256 pixels
- Power: <0.6 uW per pixel
- Compute: 50 MOps/pixel/second

TIER 3: STANDARD (Consumer)
- Target resolution: 3840x2160 (4K)
- Pixel pitch: 80-100 um
- Cognitum transistors: 2.3M per pixel
- Local SRAM: 512 bytes
- Tile size: 16x16 = 256 pixels
- Power: <1.5 uW per pixel
- Compute: 100 MOps/pixel/second

TIER 4: PRO (Professional)
- Target resolution: 7680x4320 (8K)
- Pixel pitch: 90-120 um
- Cognitum transistors: 3M per pixel
- Local SRAM: 1 KB
- Tile size: 32x32 = 1024 pixels
- Power: <3 uW per pixel
- Compute: 200 MOps/pixel/second

TIER 5: ULTRA (Datacenter/Scientific)
- Target resolution: 15360x8640 (16K) or tiled
- Pixel pitch: 100-150 um
- Cognitum transistors: 5M per pixel
- Local SRAM: 2 KB
- Tile size: 64x64 = 4096 pixels
- Power: <6 uW per pixel
- Compute: 500 MOps/pixel/second
```

### Cost Model (Volume Production)

| Tier | Die Area | Dies/Wafer | Cost/Display | Target Price |
|------|----------|------------|--------------|--------------|
| Nano | 1mm^2/tile | 70,000 | $2 | $49 |
| Mini | 2mm^2/tile | 35,000 | $15 | $199 |
| Standard | 4mm^2/tile | 17,500 | $50 | $499 |
| Pro | 8mm^2/tile | 8,700 | $150 | $1,499 |
| Ultra | 16mm^2/tile | 4,300 | $500 | $4,999 |

### Feature Matrix

| Feature | Nano | Mini | Standard | Pro | Ultra |
|---------|------|------|----------|-----|-------|
| MAC operations | Yes | Yes | Yes | Yes | Yes |
| Activation functions | ReLU | All | All | All | All |
| Spiking neural | No | Yes | Yes | Yes | Yes |
| Tile memory | 4KB | 16KB | 64KB | 256KB | 1MB |
| FP16 accumulate | No | No | Optional | Yes | Yes |
| External DRAM | No | Optional | Optional | Yes | Yes |

## Alternatives Considered

### Alternative 1: Single Universal Tier

**Pros:**
- One design to develop and verify
- Maximum economies of scale
- Consistent programmer experience

**Cons:**
- Overbuilt for low-end markets (cost prohibitive)
- Underspecced for high-end (performance limited)
- Thermal challenges span extreme range
- No competitive differentiation

**Rejected because:** A $500 smartwatch display or a 1W datacenter inference chip are not viable products. Market segments have fundamentally different requirements.

### Alternative 2: Continuous Scaling (No Fixed Tiers)

**Pros:**
- Maximum flexibility for each application
- Fine-grained optimization possible
- No artificial constraints

**Cons:**
- Exponential verification complexity
- No standardized tooling or programming model
- Each product is a new design
- Supply chain fragmentation
- Testing infrastructure must handle infinite variants

**Rejected because:** The verification and manufacturing costs would be prohibitive. Standardized tiers enable reusable tooling, testing infrastructure, and programming abstractions.

### Alternative 3: Three Tiers Only (Low/Medium/High)

**Pros:**
- Simpler product line
- Faster time to market
- Easier inventory management

**Cons:**
- Gaps in market coverage (wearables vs mobile, professional vs datacenter)
- Forces customers to over/under-buy
- Less competitive differentiation

**Rejected because:** The wearable and datacenter segments have sufficiently different requirements that a three-tier model leaves significant market segments unaddressed.

### Alternative 4: Seven or More Tiers

**Pros:**
- Finer market segmentation
- Precise cost optimization per segment
- Maximum competitive positioning

**Cons:**
- Increased design complexity
- Higher verification burden
- SKU proliferation
- Customer confusion
- Minimal incremental value beyond five

**Rejected because:** Diminishing returns beyond five tiers. The five chosen tiers cover distinct use cases with meaningful differentiation; additional tiers would overlap.

## Consequences

### Positive Consequences

1. **Market coverage**: Five tiers address wearable, mobile, consumer, professional, and datacenter markets with appropriately scaled solutions.

2. **Cost optimization**: Each tier's silicon area matches market price expectations; no subsidizing low-end with high-end margins required.

3. **Reusable tooling**: Common ISA across tiers enables unified compiler and programming model; tier-specific optimizations are optional.

4. **Clear upgrade path**: Users can migrate applications from lower to higher tiers with predictable performance scaling.

5. **Manufacturing efficiency**: Standardized tile sizes enable optimized test flows and yield management per tier.

### Negative Consequences

1. **Verification burden**: Five distinct configurations require independent verification, increasing NRE costs.

2. **Testing complexity**: Each tier requires calibrated test equipment; five test programs must be maintained.

3. **Inventory management**: OEMs must stock components for each tier they support.

4. **Feature fragmentation**: Applications must handle graceful degradation when features (FP16, spiking) are unavailable on lower tiers.

5. **Documentation overhead**: Programming guides, datasheets, and specifications must cover all tier variations.

### Scaling Relationships

| Metric | Nano to Ultra Scaling |
|--------|----------------------|
| Transistors/pixel | 10x (500K to 5M) |
| SRAM/pixel | 16x (128B to 2KB) |
| Power/pixel | 20x (0.3uW to 6uW) |
| Compute/pixel | 50x (10 to 500 MOps/s) |
| Cost/display | 250x ($2 to $500) |

### Migration Guidelines

- **Nano -> Mini**: Add spiking support, increase SRAM
- **Mini -> Standard**: Double SRAM, increase clock
- **Standard -> Pro**: Add FP16 accumulate, 4x tile memory
- **Pro -> Ultra**: Maximum compute, full feature set

## Related Decisions

- ADR-001 (Cognitum Architecture): Base 8-bit design scales across tiers
- ADR-003 (Memory Hierarchy): SRAM scaling per tier
- ADR-005 (Power Management): Power budget defines tier boundaries

## References

- Apple product segmentation strategy (iPhone, iPad, Mac tiers)
- NVIDIA GPU tier structure (GeForce, Quadro, Tesla)
- ARM Cortex family (M0 through A78)
