# Fixel Density Tier Cost Analysis

This document provides a detailed cost breakdown for each Fixel density tier, including manufacturing costs, yield considerations, and total cost of ownership projections.

---

## Executive Summary

| Tier | Unit Cost | Cost/Pixel | Cost/MTOPS | Target Volume |
|------|-----------|------------|------------|---------------|
| NANO | $0.50 | $30.52/Mpx | $0.0003 | 100M+ units |
| MICRO | $5.00 | $21.70/Mpx | $0.0002 | 10M+ units |
| STANDARD | $25.00 | $6.78/Mpx | $0.00003 | 1M+ units |
| PRO | $80.00 | $9.65/Mpx | $0.00001 | 100K+ units |
| ULTRA | $200.00 | $6.04/Mpx | $0.000006 | 10K+ units |

---

## NANO Tier Cost Breakdown

**Target Price: $0.50**

### Bill of Materials (BOM)

| Component | Cost | % of Total |
|-----------|------|------------|
| Silicon die (28nm, 2mm^2) | $0.15 | 30% |
| Package (QFN-16) | $0.08 | 16% |
| Testing & calibration | $0.05 | 10% |
| PCB integration support | $0.02 | 4% |
| **Subtotal (Manufacturing)** | **$0.30** | **60%** |

### Overhead Costs

| Category | Cost | % of Total |
|----------|------|------------|
| R&D amortization | $0.05 | 10% |
| Quality assurance | $0.03 | 6% |
| Logistics & handling | $0.02 | 4% |
| Margin | $0.10 | 20% |
| **Total** | **$0.50** | **100%** |

### Yield Assumptions

- Die yield: 95% (mature 28nm process)
- Package yield: 99%
- Test yield: 98%
- **Composite yield: 92%**

### Volume Economics

| Annual Volume | Unit Cost | Notes |
|---------------|-----------|-------|
| 1M units | $0.75 | Low volume premium |
| 10M units | $0.55 | Standard pricing |
| 100M units | $0.45 | Volume discount |
| 1B units | $0.35 | Strategic partnership |

---

## MICRO Tier Cost Breakdown

**Target Price: $5.00**

### Bill of Materials (BOM)

| Component | Cost | % of Total |
|-----------|------|------------|
| Silicon die (16nm, 15mm^2) | $1.50 | 30% |
| Package (BGA-144) | $0.40 | 8% |
| Testing & calibration | $0.30 | 6% |
| Power management IC | $0.25 | 5% |
| Decoupling capacitors | $0.05 | 1% |
| **Subtotal (Manufacturing)** | **$2.50** | **50%** |

### Overhead Costs

| Category | Cost | % of Total |
|----------|------|------------|
| R&D amortization | $0.75 | 15% |
| Quality assurance | $0.25 | 5% |
| Software/firmware | $0.20 | 4% |
| Certification (FCC, CE) | $0.10 | 2% |
| Logistics | $0.10 | 2% |
| Margin | $1.10 | 22% |
| **Total** | **$5.00** | **100%** |

### Yield Assumptions

- Die yield: 88% (16nm, medium die size)
- Package yield: 98%
- Test yield: 97%
- **Composite yield: 84%**

### Volume Economics

| Annual Volume | Unit Cost | Notes |
|---------------|-----------|-------|
| 100K units | $7.50 | Engineering samples |
| 1M units | $5.50 | Standard pricing |
| 10M units | $4.50 | Volume discount |
| 100M units | $3.80 | Major OEM pricing |

---

## STANDARD Tier Cost Breakdown

**Target Price: $25.00**

### Bill of Materials (BOM)

| Component | Cost | % of Total |
|-----------|------|------------|
| Silicon die (7nm, 80mm^2) | $8.00 | 32% |
| Advanced package (FCCSP) | $1.50 | 6% |
| Interposer/redistribution | $1.00 | 4% |
| Testing & calibration | $1.00 | 4% |
| Power management | $0.75 | 3% |
| Thermal interface | $0.25 | 1% |
| **Subtotal (Manufacturing)** | **$12.50** | **50%** |

### Overhead Costs

| Category | Cost | % of Total |
|----------|------|------------|
| R&D amortization | $4.00 | 16% |
| Quality assurance | $1.00 | 4% |
| Software stack | $1.50 | 6% |
| Certification | $0.50 | 2% |
| Technical support | $0.50 | 2% |
| Margin | $5.00 | 20% |
| **Total** | **$25.00** | **100%** |

### Yield Assumptions

- Die yield: 75% (7nm, large die)
- Package yield: 96%
- Test yield: 95%
- **Composite yield: 68%**

### Volume Economics

| Annual Volume | Unit Cost | Notes |
|---------------|-----------|-------|
| 10K units | $40.00 | Early adopter pricing |
| 100K units | $28.00 | Standard pricing |
| 1M units | $22.00 | Tier-1 OEM pricing |
| 10M units | $18.00 | Strategic volume |

---

## PRO Tier Cost Breakdown

**Target Price: $80.00**

### Bill of Materials (BOM)

| Component | Cost | % of Total |
|-----------|------|------------|
| Silicon die (5nm, 200mm^2) | $28.00 | 35% |
| Advanced package (CoWoS) | $6.00 | 7.5% |
| HBM/advanced memory | $4.00 | 5% |
| Testing & burn-in | $3.00 | 3.75% |
| Power delivery | $2.00 | 2.5% |
| Thermal solution | $1.00 | 1.25% |
| **Subtotal (Manufacturing)** | **$44.00** | **55%** |

### Overhead Costs

| Category | Cost | % of Total |
|----------|------|------------|
| R&D amortization | $12.00 | 15% |
| Quality assurance | $3.00 | 3.75% |
| Software/SDK | $4.00 | 5% |
| Certification (medical, auto) | $2.00 | 2.5% |
| Premium support | $1.00 | 1.25% |
| Margin | $14.00 | 17.5% |
| **Total** | **$80.00** | **100%** |

### Yield Assumptions

- Die yield: 55% (5nm, very large die)
- Package yield: 92% (CoWoS complexity)
- Test yield: 94%
- **Composite yield: 48%**

### Volume Economics

| Annual Volume | Unit Cost | Notes |
|---------------|-----------|-------|
| 1K units | $150.00 | Engineering/eval |
| 10K units | $95.00 | Early production |
| 100K units | $75.00 | Volume production |
| 1M units | $60.00 | High-volume OEM |

---

## ULTRA Tier Cost Breakdown

**Target Price: $200.00**

### Bill of Materials (BOM)

| Component | Cost | % of Total |
|-----------|------|------------|
| Silicon die (3nm, 400mm^2) | $70.00 | 35% |
| Chiplet package (UCIe) | $15.00 | 7.5% |
| HBM3 memory (32GB) | $20.00 | 10% |
| Advanced testing | $8.00 | 4% |
| Power delivery network | $5.00 | 2.5% |
| Active cooling interface | $2.00 | 1% |
| **Subtotal (Manufacturing)** | **$120.00** | **60%** |

### Overhead Costs

| Category | Cost | % of Total |
|----------|------|------------|
| R&D amortization | $30.00 | 15% |
| Quality assurance | $8.00 | 4% |
| Enterprise software | $10.00 | 5% |
| Premium certification | $4.00 | 2% |
| White-glove support | $3.00 | 1.5% |
| Margin | $25.00 | 12.5% |
| **Total** | **$200.00** | **100%** |

### Yield Assumptions

- Die yield: 35% (3nm, maximum size)
- Package yield: 88% (chiplet complexity)
- Test yield: 92%
- **Composite yield: 28%**

### Volume Economics

| Annual Volume | Unit Cost | Notes |
|---------------|-----------|-------|
| 100 units | $500.00 | Prototype/sample |
| 1K units | $280.00 | Early adopter |
| 10K units | $200.00 | Production volume |
| 100K units | $150.00 | Datacenter scale |

---

## Total Cost of Ownership (TCO) Analysis

### 5-Year TCO per Unit

| Tier | Unit Cost | Power (5yr) | Maintenance | Software | Total TCO |
|------|-----------|-------------|-------------|----------|-----------|
| NANO | $0.50 | $0.10 | $0.00 | $0.00 | $0.60 |
| MICRO | $5.00 | $1.00 | $0.50 | $1.00 | $7.50 |
| STANDARD | $25.00 | $15.00 | $5.00 | $10.00 | $55.00 |
| PRO | $80.00 | $75.00 | $20.00 | $50.00 | $225.00 |
| ULTRA | $200.00 | $250.00 | $50.00 | $100.00 | $600.00 |

### Power Cost Assumptions

- Electricity: $0.12/kWh
- Operating hours: 8,760/year (24/7) for STANDARD+
- Operating hours: 4,380/year (12/7) for MICRO
- Operating hours: 2,190/year (6/7) for NANO
- Power efficiency improvement: 5%/year

### Comparison to Alternatives

| Solution | STANDARD Tier | Discrete GPU | Cloud API |
|----------|---------------|--------------|-----------|
| Unit cost | $25 | $400 | N/A |
| 5-year power | $15 | $500 | N/A |
| Latency | <1ms | 5-20ms | 50-200ms |
| Privacy | On-device | On-device | Cloud |
| 5-year API cost | N/A | N/A | $5,000+ |
| **5-year TCO** | **$55** | **$950** | **$5,000+** |

---

## Cost Reduction Roadmap

### Near-term (1-2 years)

| Initiative | Savings | Applicable Tiers |
|------------|---------|------------------|
| Yield improvement (5%) | 3-8% | All |
| Package optimization | 2-5% | MICRO, STANDARD |
| Test time reduction | 1-3% | All |
| Volume scaling | 5-15% | All |

### Medium-term (2-4 years)

| Initiative | Savings | Applicable Tiers |
|------------|---------|------------------|
| Process shrink (1 node) | 15-25% | STANDARD, PRO |
| Chiplet architecture | 10-20% | PRO, ULTRA |
| Advanced packaging | 5-10% | All |
| Memory integration | 8-12% | STANDARD+ |

### Long-term (4+ years)

| Initiative | Savings | Applicable Tiers |
|------------|---------|------------------|
| 3D stacking | 20-30% | PRO, ULTRA |
| New materials (GAA) | 15-25% | All |
| Photonic interconnect | 10-15% | ULTRA |
| Quantum-ready architecture | TBD | Future tier |

---

## Investment Analysis

### Break-even Analysis (at target volumes)

| Tier | R&D Investment | Break-even Volume | Time to Break-even |
|------|----------------|-------------------|-------------------|
| NANO | $5M | 10M units | 12 months |
| MICRO | $25M | 5M units | 18 months |
| STANDARD | $100M | 4M units | 24 months |
| PRO | $250M | 3M units | 36 months |
| ULTRA | $500M | 2.5M units | 48 months |

### ROI Projections (5-year)

| Tier | Investment | Revenue (5yr) | ROI |
|------|------------|---------------|-----|
| NANO | $5M | $50M | 900% |
| MICRO | $25M | $250M | 900% |
| STANDARD | $100M | $625M | 525% |
| PRO | $250M | $800M | 220% |
| ULTRA | $500M | $1B | 100% |

---

## Conclusion

The Fixel density tier architecture provides a clear cost-performance gradient across market segments:

1. **NANO** offers exceptional value for high-volume IoT applications
2. **MICRO** balances capability and cost for wearables
3. **STANDARD** provides the best price-performance ratio for consumer electronics
4. **PRO** delivers professional-grade performance at accessible pricing
5. **ULTRA** enables workstation-class compute at display pricing

The cost structure benefits from:
- Shared architecture across tiers (R&D efficiency)
- Scalable manufacturing (common process nodes per tier)
- Software reuse (unified SDK)
- Volume economics (larger tiers subsidized by smaller tier volumes)
