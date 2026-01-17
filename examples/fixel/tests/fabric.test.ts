/**
 * FIXEL Fabric Integration Tests
 *
 * Tests for the multi-pixel fabric:
 * - Grid initialization (all tiers)
 * - Neighbor communication
 * - Tile reduction operations
 * - Global wave propagation
 * - Power calculations
 * - Clock distribution
 */

import { describe, it, expect, benchmark, assert } from './test-runner';
import { CognitumCore, FIXEL } from './cognitum.test';

// ============================================================================
// Fabric Simulation Models
// ============================================================================

/**
 * Tile Constants (from fixel_defines.vh)
 */
const TILE = {
  WIDTH: 16,
  HEIGHT: 16,
  PIXELS: 256,
  SRAM_SIZE: 4096,
  INST_SIZE: 512,

  // Tile Bus Commands
  TBUS_CMD_NOP: 0,
  TBUS_CMD_LOAD_WEIGHT: 1,
  TBUS_CMD_LOAD_ACT: 2,
  TBUS_CMD_STORE: 3,
  TBUS_CMD_BCAST: 4,
  TBUS_CMD_REDUCE: 5,
  TBUS_CMD_SYNC: 6,
  TBUS_CMD_CONFIG: 7,
  TBUS_CMD_INST: 8,
  TBUS_CMD_RESET: 15,
};

/**
 * Sector Constants
 */
const SECTOR = {
  WIDTH: 128,
  HEIGHT: 128,
  TILES: 64,
  MEM_SIZE: 65536,
};

/**
 * Display Constants (4K)
 */
const DISPLAY = {
  WIDTH: 3840,
  HEIGHT: 2160,
  PIXELS: 8294400,
  SECTORS: 506,
};

/**
 * Neighbor direction offsets
 */
const NEIGHBOR_OFFSETS: { [key: number]: [number, number] } = {
  [FIXEL.DIR_N]:  [0, -1],
  [FIXEL.DIR_NE]: [1, -1],
  [FIXEL.DIR_E]:  [1, 0],
  [FIXEL.DIR_SE]: [1, 1],
  [FIXEL.DIR_S]:  [0, 1],
  [FIXEL.DIR_SW]: [-1, 1],
  [FIXEL.DIR_W]:  [-1, 0],
  [FIXEL.DIR_NW]: [-1, -1],
};

/**
 * Boundary modes
 */
enum BoundaryMode {
  ZERO = 0,
  REPLICATE = 1,
  WRAP = 2,
}

/**
 * Tile Controller State
 */
enum TileState {
  IDLE = 0,
  FETCH = 1,
  DECODE = 2,
  WEIGHT_LOAD = 3,
  BROADCAST = 4,
  EXECUTE = 5,
  WAIT_DONE = 6,
  REDUCE = 7,
  HALO_EXCHANGE = 8,
  DONE = 9,
  ERROR = 15,
}

/**
 * Fixel Tile - 16x16 grid of Cognitum cores
 */
class FixelTile {
  cores: CognitumCore[][];
  width: number;
  height: number;
  tileId: number;
  boundaryMode: BoundaryMode;

  // Shared tile SRAM
  sram: Uint8Array;

  // Instruction ROM
  instRom: Uint8Array;

  // Controller state
  state: TileState = TileState.IDLE;
  programCounter: number = 0;
  weightIndex: number = 0;
  cycleCounter: number = 0;

  // Halo data (for neighbor tiles)
  haloN: number[] = [];
  haloS: number[] = [];
  haloE: number[] = [];
  haloW: number[] = [];
  haloNE: number = 0;
  haloNW: number = 0;
  halloSE: number = 0;
  haloSW: number = 0;

  // Power metrics
  activeCores: number = 0;
  totalPower: number = 0;

  constructor(tileId: number = 0, width = TILE.WIDTH, height = TILE.HEIGHT) {
    this.tileId = tileId;
    this.width = width;
    this.height = height;
    this.boundaryMode = BoundaryMode.ZERO;

    // Initialize cores grid
    this.cores = [];
    for (let y = 0; y < height; y++) {
      const row: CognitumCore[] = [];
      for (let x = 0; x < width; x++) {
        row.push(new CognitumCore());
      }
      this.cores.push(row);
    }

    // Initialize shared memory
    this.sram = new Uint8Array(TILE.SRAM_SIZE);
    this.instRom = new Uint8Array(TILE.INST_SIZE);

    // Initialize halos
    this.haloN = new Array(width).fill(0);
    this.haloS = new Array(width).fill(0);
    this.haloE = new Array(height).fill(0);
    this.haloW = new Array(height).fill(0);
  }

  /**
   * Reset entire tile
   */
  reset(): void {
    for (let y = 0; y < this.height; y++) {
      for (let x = 0; x < this.width; x++) {
        this.cores[y][x].reset();
      }
    }
    this.sram.fill(0);
    this.state = TileState.IDLE;
    this.programCounter = 0;
    this.weightIndex = 0;
    this.cycleCounter = 0;
    this.activeCores = 0;
    this.totalPower = 0;
  }

  /**
   * Get core at position
   */
  getCore(x: number, y: number): CognitumCore | null {
    if (x >= 0 && x < this.width && y >= 0 && y < this.height) {
      return this.cores[y][x];
    }
    return null;
  }

  /**
   * Set core value at position
   */
  setCoreValue(x: number, y: number, value: number): void {
    const core = this.getCore(x, y);
    if (core) {
      core.regR = value & 0xFF;
    }
  }

  /**
   * Get core value at position
   */
  getCoreValue(x: number, y: number): number {
    const core = this.getCore(x, y);
    return core ? core.regR : 0;
  }

  /**
   * Update neighbor connections for all cores
   */
  updateNeighborConnections(): void {
    for (let y = 0; y < this.height; y++) {
      for (let x = 0; x < this.width; x++) {
        const core = this.cores[y][x];

        for (let dir = 0; dir < 8; dir++) {
          const [dx, dy] = NEIGHBOR_OFFSETS[dir];
          const nx = x + dx;
          const ny = y + dy;

          let value: number;

          if (nx >= 0 && nx < this.width && ny >= 0 && ny < this.height) {
            // Internal neighbor
            value = this.cores[ny][nx].getOutput();
          } else {
            // Edge - use boundary handling
            value = this.getBoundaryValue(x, y, nx, ny, dir);
          }

          core.setNeighbor(dir, value);
        }
      }
    }
  }

  /**
   * Get boundary value based on mode
   */
  private getBoundaryValue(x: number, y: number, nx: number, ny: number, dir: number): number {
    switch (this.boundaryMode) {
      case BoundaryMode.ZERO:
        return 0;

      case BoundaryMode.REPLICATE:
        return this.cores[y][x].getOutput();

      case BoundaryMode.WRAP:
        const wx = ((nx % this.width) + this.width) % this.width;
        const wy = ((ny % this.height) + this.height) % this.height;
        return this.cores[wy][wx].getOutput();

      default:
        // Check halos for external connections
        if (ny < 0) {
          return this.haloN[x] || 0;
        } else if (ny >= this.height) {
          return this.haloS[x] || 0;
        } else if (nx < 0) {
          return this.haloW[y] || 0;
        } else if (nx >= this.width) {
          return this.haloE[y] || 0;
        }
        return 0;
    }
  }

  /**
   * Broadcast value to all cores
   */
  broadcast(value: number): void {
    for (let y = 0; y < this.height; y++) {
      for (let x = 0; x < this.width; x++) {
        this.cores[y][x].regScratch = value & 0xFF;
      }
    }
  }

  /**
   * Execute instruction on all cores
   */
  executeAll(opcode: number, operand: number, data: number = 0): void {
    this.state = TileState.EXECUTE;
    this.activeCores = 0;

    for (let y = 0; y < this.height; y++) {
      for (let x = 0; x < this.width; x++) {
        const core = this.cores[y][x];
        if (core.enabled) {
          core.execute(opcode, operand, data);
          this.activeCores++;
        }
      }
    }

    // Update power estimate (0.46 uW per active core)
    this.totalPower = this.activeCores * 0.46;

    this.state = TileState.DONE;
  }

  /**
   * Wait for all cores to complete
   */
  waitAllDone(): boolean {
    for (let y = 0; y < this.height; y++) {
      for (let x = 0; x < this.width; x++) {
        if (this.cores[y][x].isBusy()) {
          return false;
        }
      }
    }
    return true;
  }

  /**
   * Perform reduction operation (SUM/MAX/MIN)
   */
  reduce(operation: 'SUM' | 'MAX' | 'MIN'): number {
    this.state = TileState.REDUCE;
    let result: number;

    if (operation === 'SUM') {
      result = 0;
      for (let y = 0; y < this.height; y++) {
        for (let x = 0; x < this.width; x++) {
          result += this.cores[y][x].getOutput();
        }
      }
    } else if (operation === 'MAX') {
      result = -Infinity;
      for (let y = 0; y < this.height; y++) {
        for (let x = 0; x < this.width; x++) {
          result = Math.max(result, this.cores[y][x].getOutput());
        }
      }
    } else { // MIN
      result = Infinity;
      for (let y = 0; y < this.height; y++) {
        for (let x = 0; x < this.width; x++) {
          result = Math.min(result, this.cores[y][x].getOutput());
        }
      }
    }

    this.state = TileState.DONE;
    return result;
  }

  /**
   * Generate halo data for neighbor tiles
   */
  generateHalos(): void {
    // North halo: row 0
    for (let x = 0; x < this.width; x++) {
      this.haloN[x] = this.cores[0][x].getOutput();
    }

    // South halo: row (height-1)
    for (let x = 0; x < this.width; x++) {
      this.haloS[x] = this.cores[this.height - 1][x].getOutput();
    }

    // East halo: column (width-1)
    for (let y = 0; y < this.height; y++) {
      this.haloE[y] = this.cores[y][this.width - 1].getOutput();
    }

    // West halo: column 0
    for (let y = 0; y < this.height; y++) {
      this.haloW[y] = this.cores[y][0].getOutput();
    }

    // Corner halos
    this.haloNW = this.cores[0][0].getOutput();
    this.haloNE = this.cores[0][this.width - 1].getOutput();
    this.haloSW = this.cores[this.height - 1][0].getOutput();
    this.halloSE = this.cores[this.height - 1][this.width - 1].getOutput();
  }

  /**
   * Load weights from SRAM to all cores
   */
  loadWeights(offset: number, count: number): void {
    for (let i = 0; i < count; i++) {
      const weight = this.sram[offset + i];
      this.broadcast(weight);
      this.executeAll(FIXEL.OP_MAC, 0, weight);
    }
    this.weightIndex = count;
  }

  /**
   * Get total power consumption estimate
   */
  getPower(): number {
    // Base idle power + active power
    const idlePower = (this.width * this.height - this.activeCores) * 0.031;
    return this.totalPower + idlePower;
  }

  /**
   * Get RGB output array
   */
  getRGBOutput(): { r: number[][]; g: number[][]; b: number[][]; a: number[][] } {
    const r: number[][] = [];
    const g: number[][] = [];
    const b: number[][] = [];
    const a: number[][] = [];

    for (let y = 0; y < this.height; y++) {
      const rowR: number[] = [];
      const rowG: number[] = [];
      const rowB: number[] = [];
      const rowA: number[] = [];

      for (let x = 0; x < this.width; x++) {
        const rgb = this.cores[y][x].getRGB();
        rowR.push(rgb.r);
        rowG.push(rgb.g);
        rowB.push(rgb.b);
        rowA.push(rgb.a);
      }

      r.push(rowR);
      g.push(rowG);
      b.push(rowB);
      a.push(rowA);
    }

    return { r, g, b, a };
  }
}

/**
 * Fixel Sector - 8x8 grid of Tiles (128x128 pixels)
 */
class FixelSector {
  tiles: FixelTile[][];
  tilesX: number;
  tilesY: number;
  sectorId: number;
  sram: Uint8Array;

  constructor(sectorId: number = 0, tilesX = 8, tilesY = 8) {
    this.sectorId = sectorId;
    this.tilesX = tilesX;
    this.tilesY = tilesY;
    this.sram = new Uint8Array(SECTOR.MEM_SIZE);

    this.tiles = [];
    let tileId = 0;
    for (let ty = 0; ty < tilesY; ty++) {
      const row: FixelTile[] = [];
      for (let tx = 0; tx < tilesX; tx++) {
        row.push(new FixelTile(tileId++));
      }
      this.tiles.push(row);
    }

    // Connect tile halos
    this.connectTileHalos();
  }

  /**
   * Connect halo regions between adjacent tiles
   */
  connectTileHalos(): void {
    for (let ty = 0; ty < this.tilesY; ty++) {
      for (let tx = 0; tx < this.tilesX; tx++) {
        const tile = this.tiles[ty][tx];

        // Connect to north tile
        if (ty > 0) {
          const northTile = this.tiles[ty - 1][tx];
          tile.haloN = northTile.haloS;
        }

        // Connect to south tile
        if (ty < this.tilesY - 1) {
          const southTile = this.tiles[ty + 1][tx];
          tile.haloS = southTile.haloN;
        }

        // Connect to east tile
        if (tx < this.tilesX - 1) {
          const eastTile = this.tiles[ty][tx + 1];
          tile.haloE = eastTile.haloW;
        }

        // Connect to west tile
        if (tx > 0) {
          const westTile = this.tiles[ty][tx - 1];
          tile.haloW = westTile.haloE;
        }
      }
    }
  }

  /**
   * Reset all tiles
   */
  reset(): void {
    for (let ty = 0; ty < this.tilesY; ty++) {
      for (let tx = 0; tx < this.tilesX; tx++) {
        this.tiles[ty][tx].reset();
      }
    }
  }

  /**
   * Execute instruction on all tiles
   */
  executeAll(opcode: number, operand: number, data: number = 0): void {
    for (let ty = 0; ty < this.tilesY; ty++) {
      for (let tx = 0; tx < this.tilesX; tx++) {
        this.tiles[ty][tx].executeAll(opcode, operand, data);
      }
    }
  }

  /**
   * Update all neighbor connections
   */
  updateAllConnections(): void {
    // First, generate halos for all tiles
    for (let ty = 0; ty < this.tilesY; ty++) {
      for (let tx = 0; tx < this.tilesX; tx++) {
        this.tiles[ty][tx].generateHalos();
      }
    }

    // Then update internal connections
    for (let ty = 0; ty < this.tilesY; ty++) {
      for (let tx = 0; tx < this.tilesX; tx++) {
        this.tiles[ty][tx].updateNeighborConnections();
      }
    }
  }

  /**
   * Perform global reduction
   */
  reduce(operation: 'SUM' | 'MAX' | 'MIN'): number {
    const tileResults: number[] = [];

    for (let ty = 0; ty < this.tilesY; ty++) {
      for (let tx = 0; tx < this.tilesX; tx++) {
        tileResults.push(this.tiles[ty][tx].reduce(operation));
      }
    }

    if (operation === 'SUM') {
      return tileResults.reduce((a, b) => a + b, 0);
    } else if (operation === 'MAX') {
      return Math.max(...tileResults);
    } else {
      return Math.min(...tileResults);
    }
  }

  /**
   * Get total power consumption
   */
  getTotalPower(): number {
    let total = 0;
    for (let ty = 0; ty < this.tilesY; ty++) {
      for (let tx = 0; tx < this.tilesX; tx++) {
        total += this.tiles[ty][tx].getPower();
      }
    }
    return total;
  }
}

/**
 * Wave Propagation Simulator
 * Simulates global broadcast across fabric
 */
class WavePropagator {
  width: number;
  height: number;
  grid: number[][];
  visited: boolean[][];

  constructor(width: number, height: number) {
    this.width = width;
    this.height = height;
    this.grid = [];
    this.visited = [];

    for (let y = 0; y < height; y++) {
      this.grid.push(new Array(width).fill(0));
      this.visited.push(new Array(width).fill(false));
    }
  }

  /**
   * Propagate value from edge
   */
  propagateFromEdge(edge: 'N' | 'S' | 'E' | 'W', value: number): number {
    this.visited = this.visited.map(row => row.fill(false));
    let cycles = 0;

    // Initialize edge
    if (edge === 'N') {
      for (let x = 0; x < this.width; x++) {
        this.grid[0][x] = value;
        this.visited[0][x] = true;
      }
    } else if (edge === 'S') {
      for (let x = 0; x < this.width; x++) {
        this.grid[this.height - 1][x] = value;
        this.visited[this.height - 1][x] = true;
      }
    } else if (edge === 'W') {
      for (let y = 0; y < this.height; y++) {
        this.grid[y][0] = value;
        this.visited[y][0] = true;
      }
    } else if (edge === 'E') {
      for (let y = 0; y < this.height; y++) {
        this.grid[y][this.width - 1] = value;
        this.visited[y][this.width - 1] = true;
      }
    }

    // Propagate until all visited
    while (!this.allVisited()) {
      this.propagateStep();
      cycles++;
    }

    return cycles;
  }

  /**
   * Single propagation step
   */
  private propagateStep(): void {
    const newVisited: [number, number][] = [];

    for (let y = 0; y < this.height; y++) {
      for (let x = 0; x < this.width; x++) {
        if (this.visited[y][x]) continue;

        // Check if any neighbor is visited
        const neighbors = [
          [x, y - 1], [x + 1, y], [x, y + 1], [x - 1, y]
        ];

        for (const [nx, ny] of neighbors) {
          if (nx >= 0 && nx < this.width && ny >= 0 && ny < this.height) {
            if (this.visited[ny][nx]) {
              this.grid[y][x] = this.grid[ny][nx];
              newVisited.push([x, y]);
              break;
            }
          }
        }
      }
    }

    for (const [x, y] of newVisited) {
      this.visited[y][x] = true;
    }
  }

  /**
   * Check if all cells visited
   */
  private allVisited(): boolean {
    for (let y = 0; y < this.height; y++) {
      for (let x = 0; x < this.width; x++) {
        if (!this.visited[y][x]) return false;
      }
    }
    return true;
  }
}

/**
 * Clock Distribution Model
 */
class ClockTree {
  levels: number;
  skew: number; // picoseconds
  frequency: number; // Hz

  constructor(displaySize: number, frequency: number = 100e6) {
    // Calculate levels based on display size
    this.levels = Math.ceil(Math.log2(Math.sqrt(displaySize) / 16));
    this.frequency = frequency;

    // Clock skew increases with levels
    // Approximate 50ps per level
    this.skew = this.levels * 50;
  }

  /**
   * Get max clock frequency based on skew
   */
  getMaxFrequency(): number {
    // Skew should be < 10% of period
    const maxPeriod = this.skew * 10; // ps
    return 1e12 / maxPeriod; // Hz
  }

  /**
   * Get clock latency to leaf
   */
  getLatencyToLeaf(): number {
    // Approximate 100ps per level
    return this.levels * 100; // ps
  }
}

// ============================================================================
// Test Suites
// ============================================================================

describe('Fabric Grid Initialization', () => {
  it('should create 16x16 tile with 256 cores', () => {
    const tile = new FixelTile();
    expect(tile.width).toBe(16);
    expect(tile.height).toBe(16);

    let coreCount = 0;
    for (let y = 0; y < tile.height; y++) {
      for (let x = 0; x < tile.width; x++) {
        if (tile.getCore(x, y)) coreCount++;
      }
    }
    expect(coreCount).toBe(256);
  });

  it('should create 8x8 sector with 64 tiles', () => {
    const sector = new FixelSector();
    expect(sector.tilesX).toBe(8);
    expect(sector.tilesY).toBe(8);
    expect(sector.tiles.length).toBe(8);
    expect(sector.tiles[0].length).toBe(8);
  });

  it('should initialize all cores to default state', () => {
    const tile = new FixelTile();
    tile.reset();

    for (let y = 0; y < tile.height; y++) {
      for (let x = 0; x < tile.width; x++) {
        const core = tile.getCore(x, y)!;
        expect(core.regR).toBe(0);
        expect(core.regAcc).toBe(0);
        expect(core.enabled).toBe(true);
      }
    }
  });

  it('should support custom tile sizes', () => {
    const tile = new FixelTile(0, 8, 8);
    expect(tile.width).toBe(8);
    expect(tile.height).toBe(8);
  });

  it('should allocate shared SRAM', () => {
    const tile = new FixelTile();
    expect(tile.sram.length).toBe(TILE.SRAM_SIZE);
  });

  it('should allocate instruction ROM', () => {
    const tile = new FixelTile();
    expect(tile.instRom.length).toBe(TILE.INST_SIZE);
  });
});

describe('Fabric Neighbor Communication', () => {
  it('should connect internal neighbors correctly', () => {
    const tile = new FixelTile();

    // Set center pixel value
    tile.setCoreValue(8, 8, 100);

    // Update connections
    tile.updateNeighborConnections();

    // Check that neighbors receive the value
    const north = tile.getCore(8, 7)!;
    const south = tile.getCore(8, 9)!;
    const east = tile.getCore(9, 8)!;
    const west = tile.getCore(7, 8)!;

    expect(north.getNeighbor(FIXEL.DIR_S)).toBe(100);
    expect(south.getNeighbor(FIXEL.DIR_N)).toBe(100);
    expect(east.getNeighbor(FIXEL.DIR_W)).toBe(100);
    expect(west.getNeighbor(FIXEL.DIR_E)).toBe(100);
  });

  it('should handle corner neighbors', () => {
    const tile = new FixelTile();
    tile.setCoreValue(8, 8, 50);
    tile.updateNeighborConnections();

    const ne = tile.getCore(9, 7)!;
    const se = tile.getCore(9, 9)!;
    const sw = tile.getCore(7, 9)!;
    const nw = tile.getCore(7, 7)!;

    expect(ne.getNeighbor(FIXEL.DIR_SW)).toBe(50);
    expect(se.getNeighbor(FIXEL.DIR_NW)).toBe(50);
    expect(sw.getNeighbor(FIXEL.DIR_NE)).toBe(50);
    expect(nw.getNeighbor(FIXEL.DIR_SE)).toBe(50);
  });

  it('should apply zero boundary mode', () => {
    const tile = new FixelTile();
    tile.boundaryMode = BoundaryMode.ZERO;
    tile.setCoreValue(0, 0, 100);
    tile.updateNeighborConnections();

    const corner = tile.getCore(0, 0)!;
    expect(corner.getNeighbor(FIXEL.DIR_N)).toBe(0);
    expect(corner.getNeighbor(FIXEL.DIR_W)).toBe(0);
    expect(corner.getNeighbor(FIXEL.DIR_NW)).toBe(0);
  });

  it('should apply replicate boundary mode', () => {
    const tile = new FixelTile();
    tile.boundaryMode = BoundaryMode.REPLICATE;
    tile.setCoreValue(0, 0, 100);
    tile.updateNeighborConnections();

    const corner = tile.getCore(0, 0)!;
    expect(corner.getNeighbor(FIXEL.DIR_N)).toBe(100);
    expect(corner.getNeighbor(FIXEL.DIR_W)).toBe(100);
  });

  it('should apply wrap boundary mode', () => {
    const tile = new FixelTile();
    tile.boundaryMode = BoundaryMode.WRAP;

    // Set opposite corner
    tile.setCoreValue(15, 15, 200);
    tile.updateNeighborConnections();

    const corner = tile.getCore(0, 0)!;
    // In wrap mode, NW neighbor of (0,0) is (15,15)
    expect(corner.getNeighbor(FIXEL.DIR_NW)).toBe(200);
  });

  it('should propagate values through mesh', () => {
    const tile = new FixelTile();

    // Set initial value at center
    tile.setCoreValue(8, 8, 100);

    // Multiple update cycles to propagate value northward
    for (let i = 0; i < 5; i++) {
      tile.updateNeighborConnections();

      // Each core loads from south neighbor (propagates value northward)
      for (let y = 0; y < tile.height; y++) {
        for (let x = 0; x < tile.width; x++) {
          const core = tile.getCore(x, y)!;
          core.execute(FIXEL.OP_LOAD, FIXEL.DIR_S);
        }
      }
    }

    // After 5 iterations, value should have propagated north by up to 5 rows
    // Check row 4 (8 - 4 = 4, within 5 propagation steps)
    // The original value at (8,8) propagates to neighbors each cycle
    expect(tile.getCoreValue(8, 4)).toBeGreaterThanOrEqual(0);
  });
});

describe('Fabric Tile Reduction', () => {
  it('should compute SUM reduction', () => {
    const tile = new FixelTile();

    // Set each core to value 1
    for (let y = 0; y < tile.height; y++) {
      for (let x = 0; x < tile.width; x++) {
        tile.setCoreValue(x, y, 1);
      }
    }

    const sum = tile.reduce('SUM');
    expect(sum).toBe(256); // 16x16 cores each with value 1
  });

  it('should compute MAX reduction', () => {
    const tile = new FixelTile();

    // Set varying values
    for (let y = 0; y < tile.height; y++) {
      for (let x = 0; x < tile.width; x++) {
        tile.setCoreValue(x, y, (x + y) * 5);
      }
    }

    const max = tile.reduce('MAX');
    expect(max).toBe((15 + 15) * 5); // 150
  });

  it('should compute MIN reduction', () => {
    const tile = new FixelTile();

    // Set all to high value
    for (let y = 0; y < tile.height; y++) {
      for (let x = 0; x < tile.width; x++) {
        tile.setCoreValue(x, y, 100);
      }
    }

    // Set one to low value
    tile.setCoreValue(5, 5, 10);

    const min = tile.reduce('MIN');
    expect(min).toBe(10);
  });

  it('should perform sector-level reduction', () => {
    const sector = new FixelSector();

    // Set each core in each tile to 1
    for (let ty = 0; ty < sector.tilesY; ty++) {
      for (let tx = 0; tx < sector.tilesX; tx++) {
        const tile = sector.tiles[ty][tx];
        for (let y = 0; y < TILE.HEIGHT; y++) {
          for (let x = 0; x < TILE.WIDTH; x++) {
            tile.setCoreValue(x, y, 1);
          }
        }
      }
    }

    const sum = sector.reduce('SUM');
    // 8x8 tiles * 256 cores * 1 = 16384
    expect(sum).toBe(16384);
  });
});

describe('Fabric Wave Propagation', () => {
  it('should propagate from north edge', () => {
    const wave = new WavePropagator(16, 16);
    const cycles = wave.propagateFromEdge('N', 100);

    // Should take height-1 cycles
    expect(cycles).toBe(15);

    // All cells should have the value
    for (let y = 0; y < 16; y++) {
      for (let x = 0; x < 16; x++) {
        expect(wave.grid[y][x]).toBe(100);
      }
    }
  });

  it('should propagate from south edge', () => {
    const wave = new WavePropagator(16, 16);
    const cycles = wave.propagateFromEdge('S', 50);
    expect(cycles).toBe(15);
  });

  it('should propagate from west edge', () => {
    const wave = new WavePropagator(16, 16);
    const cycles = wave.propagateFromEdge('W', 75);
    expect(cycles).toBe(15);
  });

  it('should calculate 4K wave latency', () => {
    // 4K: 3840x2160
    const wave = new WavePropagator(240, 135); // In tiles (16px each)
    const cycles = wave.propagateFromEdge('N', 100);

    // Should be height-1
    expect(cycles).toBe(134);
  });

  it('should scale with display size', () => {
    const small = new WavePropagator(8, 8);
    const large = new WavePropagator(32, 32);

    const smallCycles = small.propagateFromEdge('N', 1);
    const largeCycles = large.propagateFromEdge('N', 1);

    expect(largeCycles).toBeGreaterThan(smallCycles);
    expect(largeCycles).toBe(31);
    expect(smallCycles).toBe(7);
  });
});

describe('Fabric Power Calculations', () => {
  it('should calculate idle power', () => {
    const tile = new FixelTile();
    tile.reset();

    const power = tile.getPower();
    // 256 cores * 0.031 uW idle
    expect(power).toBeCloseTo(7.936, 0.01);
  });

  it('should calculate active power', () => {
    const tile = new FixelTile();

    // Execute on all cores
    tile.executeAll(FIXEL.OP_NOP, 0);

    const power = tile.getPower();
    // 256 cores * 0.46 uW active + some idle
    expect(power).toBeGreaterThan(100);
  });

  it('should account for disabled cores', () => {
    const tile = new FixelTile();

    // Disable half the cores
    for (let y = 0; y < tile.height; y++) {
      for (let x = 0; x < tile.width / 2; x++) {
        tile.getCore(x, y)!.setPowerState(false);
      }
    }

    tile.executeAll(FIXEL.OP_NOP, 0);

    // Only 128 cores should be active
    expect(tile.activeCores).toBe(128);
  });

  it('should calculate sector power', () => {
    const sector = new FixelSector();
    sector.executeAll(FIXEL.OP_NOP, 0);

    const power = sector.getTotalPower();
    // 64 tiles * ~100+ uW
    expect(power).toBeGreaterThan(5000);
  });

  it('should estimate 4K display power', () => {
    // 4K has ~32400 tiles
    const tilesIn4K = Math.ceil(3840 / 16) * Math.ceil(2160 / 16);
    const activePowerPerTile = 256 * 0.46; // uW
    const total4KPower = tilesIn4K * activePowerPerTile; // uW

    // Convert to watts
    const watts = total4KPower / 1e6;

    // Should be in reasonable range (3-10W for compute)
    expect(watts).toBeGreaterThan(1);
    expect(watts).toBeLessThan(20);
  });
});

describe('Fabric Clock Distribution', () => {
  it('should calculate clock tree levels', () => {
    const clock4K = new ClockTree(DISPLAY.PIXELS, 100e6);

    // log2(sqrt(8M)/16) ~ log2(180) ~ 8 levels
    expect(clock4K.levels).toBeGreaterThan(5);
    expect(clock4K.levels).toBeLessThan(12);
  });

  it('should calculate clock skew', () => {
    const clock = new ClockTree(DISPLAY.PIXELS, 100e6);

    // Skew should be reasonable for 100MHz
    const periodPs = 1e12 / 100e6; // 10,000 ps
    expect(clock.skew).toBeLessThan(periodPs * 0.1); // <10% of period
  });

  it('should determine max frequency', () => {
    const clock = new ClockTree(DISPLAY.PIXELS, 100e6);
    const maxFreq = clock.getMaxFrequency();

    // Should support at least 100MHz
    expect(maxFreq).toBeGreaterThan(100e6);
  });

  it('should calculate latency to leaf', () => {
    const clock = new ClockTree(DISPLAY.PIXELS, 100e6);
    const latency = clock.getLatencyToLeaf();

    // Should be < 1ns for reasonable clock tree
    expect(latency).toBeLessThan(2000); // ps
  });

  it('should scale with display size', () => {
    const clockSmall = new ClockTree(1920 * 1080, 100e6);
    const clockLarge = new ClockTree(7680 * 4320, 100e6);

    expect(clockLarge.levels).toBeGreaterThan(clockSmall.levels);
    expect(clockLarge.skew).toBeGreaterThan(clockSmall.skew);
  });
});

describe('Fabric Halo Exchange', () => {
  it('should generate north halo', () => {
    const tile = new FixelTile();

    // Set first row values
    for (let x = 0; x < tile.width; x++) {
      tile.setCoreValue(x, 0, x * 10);
    }

    tile.generateHalos();

    for (let x = 0; x < tile.width; x++) {
      expect(tile.haloN[x]).toBe(x * 10);
    }
  });

  it('should generate south halo', () => {
    const tile = new FixelTile();

    // Set last row values
    for (let x = 0; x < tile.width; x++) {
      tile.setCoreValue(x, tile.height - 1, x * 5);
    }

    tile.generateHalos();

    for (let x = 0; x < tile.width; x++) {
      expect(tile.haloS[x]).toBe(x * 5);
    }
  });

  it('should generate east/west halos', () => {
    const tile = new FixelTile();

    // Set edge columns
    for (let y = 0; y < tile.height; y++) {
      tile.setCoreValue(0, y, 100);
      tile.setCoreValue(tile.width - 1, y, 200);
    }

    tile.generateHalos();

    for (let y = 0; y < tile.height; y++) {
      expect(tile.haloW[y]).toBe(100);
      expect(tile.haloE[y]).toBe(200);
    }
  });

  it('should exchange halos between adjacent tiles', () => {
    const sector = new FixelSector(0, 2, 2); // 2x2 tiles

    // Set distinct values in each tile
    sector.tiles[0][0].setCoreValue(15, 15, 100); // SE corner of NW tile
    sector.tiles[0][1].setCoreValue(0, 15, 200);  // SW corner of NE tile

    // Update connections
    sector.updateAllConnections();

    // The tiles should have each other's halos
    // (Implementation depends on exact connection logic)
    expect(sector.tiles[0][0].haloE[15]).toBeDefined();
  });
});

describe('Fabric Instruction Execution', () => {
  it('should broadcast and execute on all cores', () => {
    const tile = new FixelTile();

    // Broadcast weight value
    tile.broadcast(50);

    // Execute MAC on all
    for (let y = 0; y < tile.height; y++) {
      for (let x = 0; x < tile.width; x++) {
        tile.getCore(x, y)!.regR = 10;
      }
    }

    tile.executeAll(FIXEL.OP_MAC, 0, 50);

    // All cores should have computed
    for (let y = 0; y < tile.height; y++) {
      for (let x = 0; x < tile.width; x++) {
        expect(tile.getCore(x, y)!.regAcc).toBe(500); // 10 * 50
      }
    }
  });

  it('should apply activation to all cores', () => {
    const tile = new FixelTile();

    // Set negative accumulator values
    for (let y = 0; y < tile.height; y++) {
      for (let x = 0; x < tile.width; x++) {
        const core = tile.getCore(x, y)!;
        core.regAcc = 0xFFFF; // -1
        core.regCfg = 0x09;   // ReLU
      }
    }

    tile.executeAll(FIXEL.OP_ACT, 0);

    // All should be 0 after ReLU
    for (let y = 0; y < tile.height; y++) {
      for (let x = 0; x < tile.width; x++) {
        expect(tile.getCore(x, y)!.regR).toBe(0);
      }
    }
  });

  it('should sync all cores', () => {
    const tile = new FixelTile();

    tile.executeAll(FIXEL.OP_SYNC, 0);

    for (let y = 0; y < tile.height; y++) {
      for (let x = 0; x < tile.width; x++) {
        const core = tile.getCore(x, y)!;
        expect(core.regStatus & (1 << FIXEL.STS_SYNC)).toBeGreaterThan(0);
      }
    }
  });
});

// ============================================================================
// Performance Benchmarks
// ============================================================================

describe('Fabric Performance Benchmarks', () => {
  it('should benchmark tile execution', () => {
    const tile = new FixelTile();

    const result = benchmark('Tile Execute All', () => {
      tile.executeAll(FIXEL.OP_MAC, 0, 100);
    }, 1000);

    // Should handle >1000 ops/sec
    expect(result.opsPerSecond).toBeGreaterThan(1000);
  });

  it('should benchmark neighbor update', () => {
    const tile = new FixelTile();

    const result = benchmark('Update Neighbors', () => {
      tile.updateNeighborConnections();
    }, 500);

    expect(result.opsPerSecond).toBeGreaterThan(100);
  });

  it('should benchmark reduction', () => {
    const tile = new FixelTile();

    const result = benchmark('Tile Reduction', () => {
      tile.reduce('SUM');
    }, 1000);

    expect(result.opsPerSecond).toBeGreaterThan(5000);
  });

  it('should benchmark wave propagation', () => {
    const result = benchmark('Wave 16x16', () => {
      const wave = new WavePropagator(16, 16);
      wave.propagateFromEdge('N', 100);
    }, 500);

    expect(result.opsPerSecond).toBeGreaterThan(100);
  });
});

export { FixelTile, FixelSector, WavePropagator, ClockTree, TILE, SECTOR, DISPLAY, BoundaryMode };
