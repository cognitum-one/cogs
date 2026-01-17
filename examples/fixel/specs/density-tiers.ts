/**
 * Fixel Density Tier Specifications
 *
 * Defines the hardware specifications for each density tier of the
 * Fixel in-pixel computing architecture. Each tier targets different
 * market segments with varying compute density and power requirements.
 */

export interface Resolution {
  width: number;
  height: number;
}

export interface ResolutionRange {
  min: Resolution;
  max: Resolution;
}

export interface DensityTierSpec {
  name: string;
  tier: 'NANO' | 'MICRO' | 'STANDARD' | 'PRO' | 'ULTRA';
  resolution: ResolutionRange;
  transistorsPerPixel: number;
  sramPerPixel: number;  // bytes
  maxClockMHz: number;
  typicalPowerPerPixelUW: number;  // microwatts
  targetMarket: string;
  estimatedCostUSD: number;
  description: string;
}

export interface TierCapabilities {
  tier: DensityTierSpec['tier'];
  computeCapabilities: string[];
  typicalApplications: string[];
  limitations: string[];
}

/**
 * NANO Tier - IoT and Sensor Applications
 * Minimal compute for simple threshold detection and basic filtering
 */
export const NANO_TIER: DensityTierSpec = {
  name: 'Nano',
  tier: 'NANO',
  resolution: {
    min: { width: 64, height: 64 },
    max: { width: 128, height: 128 }
  },
  transistorsPerPixel: 100,
  sramPerPixel: 16,
  maxClockMHz: 10,
  typicalPowerPerPixelUW: 0.01,
  targetMarket: 'IoT sensors, environmental monitoring, basic motion detection',
  estimatedCostUSD: 0.50,
  description: 'Ultra-low-power tier for simple threshold and filter operations'
};

/**
 * MICRO Tier - Wearable Devices
 * Edge detection and basic classification for smartwatches and AR glasses
 */
export const MICRO_TIER: DensityTierSpec = {
  name: 'Micro',
  tier: 'MICRO',
  resolution: {
    min: { width: 256, height: 256 },
    max: { width: 480, height: 480 }
  },
  transistorsPerPixel: 10_000,
  sramPerPixel: 64,
  maxClockMHz: 50,
  typicalPowerPerPixelUW: 0.1,
  targetMarket: 'Wearables, smartwatches, AR/VR glasses, health monitors',
  estimatedCostUSD: 5.00,
  description: 'Low-power tier for edge detection and lightweight ML inference'
};

/**
 * STANDARD Tier - Consumer Electronics
 * Real-time object detection for tablets and smartphones
 */
export const STANDARD_TIER: DensityTierSpec = {
  name: 'Standard',
  tier: 'STANDARD',
  resolution: {
    min: { width: 1920, height: 1080 },
    max: { width: 2560, height: 1440 }
  },
  transistorsPerPixel: 500_000,
  sramPerPixel: 256,
  maxClockMHz: 100,
  typicalPowerPerPixelUW: 0.3,
  targetMarket: 'Tablets, smartphones, laptops, consumer cameras',
  estimatedCostUSD: 25.00,
  description: 'Mainstream tier for real-time object detection and tracking'
};

/**
 * PRO Tier - Professional Displays
 * Full neural inference and video processing at 4K resolution
 */
export const PRO_TIER: DensityTierSpec = {
  name: 'Pro',
  tier: 'PRO',
  resolution: {
    min: { width: 3840, height: 2160 },
    max: { width: 3840, height: 2160 }
  },
  transistorsPerPixel: 2_300_000,
  sramPerPixel: 512,
  maxClockMHz: 500,
  typicalPowerPerPixelUW: 0.6,
  targetMarket: 'Professional monitors, medical imaging, broadcast equipment',
  estimatedCostUSD: 80.00,
  description: 'High-performance tier for neural inference and HD video processing'
};

/**
 * ULTRA Tier - Workstation and Scientific
 * Maximum compute density for generative models and simulation
 */
export const ULTRA_TIER: DensityTierSpec = {
  name: 'Ultra',
  tier: 'ULTRA',
  resolution: {
    min: { width: 7680, height: 4320 },
    max: { width: 7680, height: 4320 }
  },
  transistorsPerPixel: 2_300_000,
  sramPerPixel: 1024,
  maxClockMHz: 1000,
  typicalPowerPerPixelUW: 1.2,
  targetMarket: 'Workstations, scientific instruments, data centers, AI research',
  estimatedCostUSD: 200.00,
  description: 'Maximum density tier for generative AI and scientific simulation'
};

/**
 * All density tiers indexed by tier name
 */
export const DENSITY_TIERS: Record<DensityTierSpec['tier'], DensityTierSpec> = {
  NANO: NANO_TIER,
  MICRO: MICRO_TIER,
  STANDARD: STANDARD_TIER,
  PRO: PRO_TIER,
  ULTRA: ULTRA_TIER
};

/**
 * Ordered array of all tiers from lowest to highest density
 */
export const ALL_TIERS: DensityTierSpec[] = [
  NANO_TIER,
  MICRO_TIER,
  STANDARD_TIER,
  PRO_TIER,
  ULTRA_TIER
];

/**
 * Calculate total transistor count for a given tier at its maximum resolution
 */
export function calculateTotalTransistors(tier: DensityTierSpec): number {
  const { max } = tier.resolution;
  return max.width * max.height * tier.transistorsPerPixel;
}

/**
 * Calculate total SRAM capacity in bytes for a given tier at its maximum resolution
 */
export function calculateTotalSRAM(tier: DensityTierSpec): number {
  const { max } = tier.resolution;
  return max.width * max.height * tier.sramPerPixel;
}

/**
 * Calculate total power consumption in watts for a given tier at maximum resolution
 */
export function calculateTotalPowerWatts(tier: DensityTierSpec): number {
  const { max } = tier.resolution;
  const totalPixels = max.width * max.height;
  const totalMicrowatts = totalPixels * tier.typicalPowerPerPixelUW;
  return totalMicrowatts / 1_000_000;  // Convert to watts
}

/**
 * Calculate TOPS (Tera Operations Per Second) for a tier
 * Assumes 1 operation per transistor per clock cycle (simplified)
 */
export function calculateTOPS(tier: DensityTierSpec): number {
  const totalTransistors = calculateTotalTransistors(tier);
  const opsPerSecond = totalTransistors * tier.maxClockMHz * 1_000_000;
  return opsPerSecond / 1_000_000_000_000;  // Convert to TOPS
}

/**
 * Calculate efficiency in TOPS per Watt
 */
export function calculateEfficiency(tier: DensityTierSpec): number {
  const tops = calculateTOPS(tier);
  const watts = calculateTotalPowerWatts(tier);
  return tops / watts;
}

/**
 * Get tier specifications summary
 */
export function getTierSummary(tier: DensityTierSpec): string {
  const totalTransistors = calculateTotalTransistors(tier);
  const totalSRAM = calculateTotalSRAM(tier);
  const totalPower = calculateTotalPowerWatts(tier);
  const tops = calculateTOPS(tier);
  const efficiency = calculateEfficiency(tier);

  return `
${tier.name} Tier (${tier.tier})
${'='.repeat(40)}
Resolution: ${tier.resolution.max.width}x${tier.resolution.max.height}
Transistors/pixel: ${tier.transistorsPerPixel.toLocaleString()}
Total transistors: ${(totalTransistors / 1e9).toFixed(2)} billion
SRAM/pixel: ${tier.sramPerPixel} bytes
Total SRAM: ${(totalSRAM / 1e9).toFixed(2)} GB
Clock: ${tier.maxClockMHz} MHz
Power/pixel: ${tier.typicalPowerPerPixelUW} µW
Total power: ${totalPower.toFixed(2)} W
Performance: ${tops.toFixed(2)} TOPS
Efficiency: ${efficiency.toFixed(2)} TOPS/W
Cost: $${tier.estimatedCostUSD.toFixed(2)}
Target: ${tier.targetMarket}
  `.trim();
}

// Export tier capabilities
export const TIER_CAPABILITIES: TierCapabilities[] = [
  {
    tier: 'NANO',
    computeCapabilities: [
      'Simple threshold detection',
      'Basic Sobel/Prewitt filters',
      'Binary morphological operations',
      'Pixel-level comparisons',
      'Simple noise reduction'
    ],
    typicalApplications: [
      'Motion detection sensors',
      'Light level monitoring',
      'Presence detection',
      'Simple edge triggering',
      'Environmental sensors'
    ],
    limitations: [
      'No floating-point operations',
      'Limited to 3x3 convolutions',
      'No multi-layer networks',
      'Single-bit outputs per pixel',
      'Very limited temporal memory'
    ]
  },
  {
    tier: 'MICRO',
    computeCapabilities: [
      'Edge detection (Canny, Sobel)',
      'Basic image classification',
      'Simple gesture recognition',
      'Face detection (Haar cascades)',
      'Histogram processing',
      'Basic optical flow'
    ],
    typicalApplications: [
      'Smartwatch face unlock',
      'AR glasses eye tracking',
      'Fitness tracker motion analysis',
      'Health monitor vitals detection',
      'Simple gesture interfaces'
    ],
    limitations: [
      'Limited to small neural networks (<1M params)',
      'No real-time video processing',
      'Basic quantization only (INT8)',
      'Limited convolution kernel sizes',
      'Restricted batch processing'
    ]
  },
  {
    tier: 'STANDARD',
    computeCapabilities: [
      'Real-time object detection (YOLO-style)',
      'Semantic segmentation',
      'Pose estimation',
      'Face recognition',
      'Scene understanding',
      'Video stabilization',
      'HDR processing'
    ],
    typicalApplications: [
      'Smartphone computational photography',
      'Tablet augmented reality',
      'Laptop video conferencing enhancement',
      'Consumer drone vision',
      'Smart home cameras'
    ],
    limitations: [
      'Limited to medium neural networks (<50M params)',
      'No generative model inference',
      'Restricted to specific model architectures',
      'Power constraints for mobile use'
    ]
  },
  {
    tier: 'PRO',
    computeCapabilities: [
      'Full neural network inference',
      'Real-time 4K video processing',
      'Multi-object tracking',
      'Advanced medical imaging analysis',
      'Real-time style transfer',
      'Depth estimation and 3D reconstruction',
      'Advanced denoising and super-resolution'
    ],
    typicalApplications: [
      'Professional video editing',
      'Medical diagnostic imaging',
      'Broadcast quality enhancement',
      'Industrial inspection systems',
      'Autonomous vehicle perception',
      'Surveillance and security'
    ],
    limitations: [
      'Large generative models require tiling',
      'Training not supported (inference only)',
      'Some transformer models exceed capacity'
    ]
  },
  {
    tier: 'ULTRA',
    computeCapabilities: [
      'Generative model inference (Stable Diffusion)',
      'Large language model acceleration',
      'Scientific simulation',
      'Ray tracing and path tracing',
      'Full transformer architectures',
      'Real-time 8K processing',
      'Multi-modal AI fusion'
    ],
    typicalApplications: [
      'AI research workstations',
      'Scientific visualization',
      'Film and VFX production',
      'High-frequency trading displays',
      'Astronomy and medical research',
      'Data center inference acceleration'
    ],
    limitations: [
      'High power consumption',
      'Premium pricing',
      'Requires active cooling',
      'Limited to inference (no training)'
    ]
  }
];
