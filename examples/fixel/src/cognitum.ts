/**
 * FIXEL Cognitum Chip Simulator
 *
 * A cycle-accurate simulator for the Cognitum - a tiny 2nm processing element
 * embedded behind each pixel in FIXEL neural vision arrays.
 *
 * Architecture:
 * - 16 x 8-bit registers (R0-R15, 64 bytes total)
 * - 512 bytes SRAM for local storage
 * - 8-bit MAC unit with 16-bit accumulator
 * - 8-bit ALU with full operation set
 * - 4-neighbor mesh interconnect (N, E, S, W)
 * - Leaky Integrate-and-Fire (LIF) spike generator
 * - Ultra-low power states (active/idle/sleep)
 *
 * @module cognitum
 * @version 1.0.0
 */

// ============================================================================
// Type Definitions
// ============================================================================

/**
 * Power states for the Cognitum chip
 */
export enum PowerState {
  /** Full operation, all units active */
  ACTIVE = 'active',
  /** Clock gated, registers retained */
  IDLE = 'idle',
  /** Deep sleep, only SRAM retained */
  SLEEP = 'sleep',
}

/**
 * Direction for neighbor communication in the mesh
 */
export enum Direction {
  NORTH = 0,
  EAST = 1,
  SOUTH = 2,
  WEST = 3,
}

/**
 * ALU operation codes
 */
export enum AluOp {
  ADD = 0x00,
  SUB = 0x01,
  AND = 0x02,
  OR = 0x03,
  XOR = 0x04,
  SHL = 0x05,  // Shift left
  SHR = 0x06,  // Shift right (logical)
  SAR = 0x07,  // Shift right (arithmetic)
  NOT = 0x08,
  NEG = 0x09,  // Two's complement negate
  INC = 0x0A,
  DEC = 0x0B,
  CMP = 0x0C,  // Compare (sets flags only)
  TST = 0x0D,  // Test (AND, sets flags only)
  MOV = 0x0E,
  NOP = 0x0F,
}

/**
 * COGNITUM-8 Instruction opcodes
 */
export enum Opcode {
  // Load/Store (0x0X)
  LDR = 0x00,   // Load register from SRAM
  STR = 0x01,   // Store register to SRAM
  LDI = 0x02,   // Load immediate to register
  LDN = 0x03,   // Load from neighbor
  STN = 0x04,   // Store to neighbor

  // ALU Operations (0x1X)
  ALU = 0x10,   // ALU operation (subop in operand)

  // MAC Operations (0x2X)
  MAC = 0x20,   // Multiply-accumulate
  MUL = 0x21,   // Multiply only
  CLR_ACC = 0x22, // Clear accumulator
  MOV_ACC = 0x23, // Move accumulator to registers

  // Branch/Control (0x3X)
  JMP = 0x30,   // Unconditional jump
  JZ = 0x31,    // Jump if zero
  JNZ = 0x32,   // Jump if not zero
  JC = 0x33,    // Jump if carry
  JNC = 0x34,   // Jump if no carry
  CALL = 0x35,  // Call subroutine
  RET = 0x36,   // Return from subroutine

  // Neural Operations (0x4X)
  SPIKE = 0x40,   // Check and generate spike
  ACCUM = 0x41,   // Accumulate input to membrane
  DECAY = 0x42,   // Apply leak to membrane
  RESET = 0x43,   // Reset membrane potential
  RELU = 0x44,    // ReLU activation
  SIGMOID = 0x45, // Sigmoid activation (LUT)
  TANH = 0x46,    // Tanh activation (LUT)

  // Power Management (0x5X)
  IDLE = 0x50,  // Enter idle state
  SLEEP = 0x51, // Enter sleep state
  WAKE = 0x52,  // Wake (handled by external signal)

  // Sync/Communication (0x6X)
  SYNC = 0x60,  // Barrier synchronization
  BCAST = 0x61, // Broadcast to all neighbors
  RECV = 0x62,  // Receive from any neighbor

  // Special (0xFX)
  HALT = 0xFF,  // Halt execution
}

/**
 * CPU status flags
 */
export interface StatusFlags {
  /** Zero flag - set when result is zero */
  zero: boolean;
  /** Carry flag - set on unsigned overflow */
  carry: boolean;
  /** Negative flag - set when result bit 7 is 1 */
  negative: boolean;
  /** Overflow flag - set on signed overflow */
  overflow: boolean;
  /** Spike flag - set when neuron fires */
  spike: boolean;
}

/**
 * Neighbor interface for mesh communication
 */
export interface NeighborInterface {
  /** Data buffer for sending */
  txBuffer: number;
  /** Data buffer for receiving */
  rxBuffer: number;
  /** Send ready flag */
  txReady: boolean;
  /** Receive valid flag */
  rxValid: boolean;
  /** Reference to connected neighbor (null if edge) */
  connected: Cognitum | null;
}

/**
 * Complete state of a Cognitum chip
 */
export interface CognitumState {
  /** 16 x 8-bit general purpose registers */
  registers: Uint8Array;
  /** 512 bytes of local SRAM */
  sram: Uint8Array;
  /** Program counter (9-bit for 512-byte addressable space) */
  pc: number;
  /** Stack pointer */
  sp: number;
  /** 16-bit MAC accumulator */
  accumulator: number;
  /** CPU status flags */
  flags: StatusFlags;
  /** Membrane potential for LIF neuron (16-bit fixed point) */
  membranePotential: number;
  /** Spike threshold */
  threshold: number;
  /** Leak factor (0-255, represents 0.0-1.0) */
  leakFactor: number;
  /** Refractory period counter */
  refractoryCounter: number;
  /** Current power state */
  powerState: PowerState;
  /** Neighbor interfaces */
  neighbors: NeighborInterface[];
  /** Current cycle count */
  cycleCount: number;
  /** Energy consumed (in femtojoules, approximate) */
  energyConsumed: number;
  /** Halted flag */
  halted: boolean;
}

/**
 * Decoded instruction structure
 */
export interface DecodedInstruction {
  /** Opcode */
  opcode: Opcode;
  /** Destination register (0-15) */
  rd: number;
  /** Source register 1 (0-15) */
  rs1: number;
  /** Source register 2 or immediate (0-15 or 8-bit) */
  rs2OrImm: number;
  /** ALU sub-operation (for ALU instructions) */
  aluOp?: AluOp;
  /** Direction for neighbor operations */
  direction?: Direction;
  /** Raw instruction bytes */
  raw: Uint8Array;
}

/**
 * Execution result for a single instruction
 */
export interface ExecutionResult {
  /** Number of cycles consumed */
  cycles: number;
  /** Energy consumed in femtojoules */
  energy: number;
  /** Whether a spike was generated */
  spiked: boolean;
  /** Whether execution should halt */
  halt: boolean;
  /** Branch target if taken, undefined otherwise */
  branchTarget?: number;
}

// ============================================================================
// Legacy Compatibility Types
// ============================================================================

/**
 * Legacy configuration interface for backward compatibility
 */
export interface CognitumConfig {
  transistorCount: number;
  sramBytes: number;
  clockSpeedMHz: number;
  x: number;
  y: number;
}

/**
 * Legacy spike event interface
 */
export interface SpikeEvent {
  sourceX: number;
  sourceY: number;
  timestamp: number;
  weight: number;
  payload?: Uint8Array;
}

/**
 * Legacy metrics interface
 */
export interface CognitumMetrics {
  spikeRate: number;
  powerMicroWatts: number;
  utilization: number;
  memoryUsed: number;
}

// ============================================================================
// Lookup Tables
// ============================================================================

/**
 * Sigmoid LUT (256 entries, input scaled to -4 to +4)
 * Output: 0-255 representing 0.0-1.0
 */
const SIGMOID_LUT: Uint8Array = new Uint8Array(256);

/**
 * Tanh LUT (256 entries, input scaled to -4 to +4)
 * Output: 0-255 representing -1.0 to +1.0 (128 = 0)
 */
const TANH_LUT: Uint8Array = new Uint8Array(256);

// Initialize LUTs at module load
(function initializeLUTs(): void {
  for (let i = 0; i < 256; i++) {
    // Map 0-255 to -4.0 to +4.0
    const x = (i / 255) * 8 - 4;

    // Sigmoid: 1 / (1 + e^(-x))
    const sigmoid = 1 / (1 + Math.exp(-x));
    SIGMOID_LUT[i] = Math.round(sigmoid * 255);

    // Tanh: (e^x - e^(-x)) / (e^x + e^(-x))
    const tanh = Math.tanh(x);
    TANH_LUT[i] = Math.round((tanh + 1) * 127.5);
  }
})();

// ============================================================================
// Energy Model Constants (femtojoules per operation at 2nm)
// ============================================================================

const ENERGY = {
  REG_READ: 0.5,
  REG_WRITE: 0.5,
  SRAM_READ: 2.0,
  SRAM_WRITE: 2.5,
  ALU_OP: 1.0,
  MAC_OP: 3.0,
  NEIGHBOR_TX: 5.0,
  NEIGHBOR_RX: 4.0,
  LUT_ACCESS: 1.5,
  SPIKE_GEN: 2.0,
  IDLE_CYCLE: 0.1,
  SLEEP_CYCLE: 0.01,
} as const;

// ============================================================================
// Cognitum Class Implementation
// ============================================================================

/**
 * Cognitum - Per-pixel neural processing element
 *
 * This class implements a cycle-accurate simulator for the Cognitum chip,
 * a 2nm processing element designed for embedding in neural vision arrays.
 * Each Cognitum handles local computations for a single pixel and communicates
 * with its 4 neighbors (North, East, South, West) in a mesh topology.
 *
 * @example
 * ```typescript
 * const chip = new Cognitum();
 *
 * // Load program into SRAM
 * chip.loadProgram([0x02, 0x10, 0x42, ...]); // LDI R0, 0x42
 *
 * // Run for 1000 cycles
 * const stats = chip.run(1000);
 * console.log(`Executed ${stats.instructionsExecuted} instructions`);
 * ```
 */
export class Cognitum {
  private state: CognitumState;

  /** Chip coordinates in the mesh (for debugging) */
  public readonly x: number;
  public readonly y: number;

  // Legacy compatibility fields
  public readonly transistorCount: number;
  public readonly sramBytes: number;
  private clockSpeedMHz: number;
  private pendingSpikes: SpikeEvent[] = [];
  private outputSpikes: SpikeEvent[] = [];

  // Power model constants (in microWatts per MHz) for legacy API
  private static readonly STATIC_POWER_UW = 0.1;
  private static readonly DYNAMIC_POWER_PER_MHZ = 0.01;
  private static readonly SPIKE_POWER_UW = 0.5;

  /**
   * Creates a new Cognitum instance
   *
   * @param configOrX - Either a CognitumConfig object (legacy) or X coordinate
   * @param y - Y coordinate in mesh (default 0, used when first arg is number)
   */
  constructor(configOrX: CognitumConfig | number = 0, y: number = 0) {
    if (typeof configOrX === 'object') {
      // Legacy constructor with config
      this.x = configOrX.x;
      this.y = configOrX.y;
      this.transistorCount = configOrX.transistorCount;
      this.sramBytes = configOrX.sramBytes;
      this.clockSpeedMHz = configOrX.clockSpeedMHz;
    } else {
      // New constructor with coordinates
      this.x = configOrX;
      this.y = y;
      this.transistorCount = 50000; // ~50K transistors for 2nm
      this.sramBytes = 512;
      this.clockSpeedMHz = 100;
    }
    this.state = this.createInitialState();
  }

  /**
   * Creates the initial state for a Cognitum chip
   */
  private createInitialState(): CognitumState {
    return {
      registers: new Uint8Array(16),
      sram: new Uint8Array(512),
      pc: 0,
      sp: 511, // Stack grows downward from top of SRAM
      accumulator: 0,
      flags: {
        zero: false,
        carry: false,
        negative: false,
        overflow: false,
        spike: false,
      },
      membranePotential: 0,
      threshold: 0x8000, // Default threshold at midpoint
      leakFactor: 230,   // ~0.9 leak factor
      refractoryCounter: 0,
      powerState: PowerState.ACTIVE,
      neighbors: [
        { txBuffer: 0, rxBuffer: 0, txReady: false, rxValid: false, connected: null },
        { txBuffer: 0, rxBuffer: 0, txReady: false, rxValid: false, connected: null },
        { txBuffer: 0, rxBuffer: 0, txReady: false, rxValid: false, connected: null },
        { txBuffer: 0, rxBuffer: 0, txReady: false, rxValid: false, connected: null },
      ],
      cycleCount: 0,
      energyConsumed: 0,
      halted: false,
    };
  }

  // ==========================================================================
  // State Access
  // ==========================================================================

  /**
   * Gets a readonly copy of the current state
   */
  public getState(): Readonly<CognitumState> {
    return { ...this.state };
  }

  /**
   * Gets the current power state
   */
  public getPowerState(): PowerState {
    return this.state.powerState;
  }

  /**
   * Gets the current cycle count
   */
  public getCycleCount(): number {
    return this.state.cycleCount;
  }

  /**
   * Gets the total energy consumed (in femtojoules)
   */
  public getEnergyConsumed(): number {
    return this.state.energyConsumed;
  }

  /**
   * Checks if the chip is halted
   */
  public isHalted(): boolean {
    return this.state.halted;
  }

  // ==========================================================================
  // Register Operations
  // ==========================================================================

  /**
   * Reads a value from a register
   *
   * @param reg - Register index (0-15)
   * @returns The 8-bit register value
   * @throws Error if register index is out of range
   */
  public readRegister(reg: number): number {
    if (reg < 0 || reg > 15) {
      throw new Error(`Invalid register index: ${reg}`);
    }
    this.state.energyConsumed += ENERGY.REG_READ;
    return this.state.registers[reg];
  }

  /**
   * Writes a value to a register
   *
   * @param reg - Register index (0-15)
   * @param value - 8-bit value to write
   * @throws Error if register index is out of range
   */
  public writeRegister(reg: number, value: number): void {
    if (reg < 0 || reg > 15) {
      throw new Error(`Invalid register index: ${reg}`);
    }
    this.state.registers[reg] = value & 0xFF;
    this.state.energyConsumed += ENERGY.REG_WRITE;
  }

  // ==========================================================================
  // SRAM Operations
  // ==========================================================================

  /**
   * Reads a byte from SRAM
   *
   * @param address - SRAM address (0-511)
   * @returns The 8-bit value at the address
   * @throws Error if address is out of range
   */
  public loadSRAM(address: number): number {
    if (address < 0 || address > 511) {
      throw new Error(`Invalid SRAM address: ${address}`);
    }
    this.state.energyConsumed += ENERGY.SRAM_READ;
    return this.state.sram[address];
  }

  /**
   * Writes a byte to SRAM
   *
   * @param address - SRAM address (0-511)
   * @param value - 8-bit value to write
   * @throws Error if address is out of range
   */
  public storeSRAM(address: number, value: number): void {
    if (address < 0 || address > 511) {
      throw new Error(`Invalid SRAM address: ${address}`);
    }
    this.state.sram[address] = value & 0xFF;
    this.state.energyConsumed += ENERGY.SRAM_WRITE;
  }

  /**
   * Loads a program into SRAM starting at address 0
   *
   * @param program - Array of bytes to load
   * @throws Error if program exceeds SRAM size
   */
  public loadProgram(program: number[] | Uint8Array): void {
    if (program.length > 512) {
      throw new Error(`Program too large: ${program.length} bytes (max 512)`);
    }
    this.state.sram.set(program);
    this.state.pc = 0;
    this.state.halted = false;
  }

  /**
   * Legacy method: Read from local SRAM
   *
   * @param offset - Start offset
   * @param length - Number of bytes to read
   * @returns Slice of SRAM contents
   */
  public readMemory(offset: number, length: number): Uint8Array {
    if (offset + length > this.sramBytes) {
      throw new Error(`Memory access out of bounds: ${offset}+${length} > ${this.sramBytes}`);
    }
    return this.state.sram.slice(offset, offset + length);
  }

  /**
   * Legacy method: Write to local SRAM
   *
   * @param offset - Start offset
   * @param data - Data to write
   */
  public writeMemory(offset: number, data: Uint8Array): void {
    if (offset + data.length > this.sramBytes) {
      throw new Error(`Memory access out of bounds: ${offset}+${data.length} > ${this.sramBytes}`);
    }
    this.state.sram.set(data, offset);
  }

  // ==========================================================================
  // MAC Unit Operations
  // ==========================================================================

  /**
   * Performs multiply-accumulate operation
   * Result = accumulator + (a * b)
   *
   * @param a - First 8-bit operand
   * @param b - Second 8-bit operand
   * @returns The 16-bit accumulator value after operation
   */
  public mac(a: number, b: number): number {
    const product = (a & 0xFF) * (b & 0xFF);
    this.state.accumulator = (this.state.accumulator + product) & 0xFFFF;
    this.state.energyConsumed += ENERGY.MAC_OP;
    return this.state.accumulator;
  }

  /**
   * Performs multiply-only operation (clears accumulator first)
   *
   * @param a - First 8-bit operand
   * @param b - Second 8-bit operand
   * @returns The 16-bit product
   */
  public multiply(a: number, b: number): number {
    this.state.accumulator = (a & 0xFF) * (b & 0xFF);
    this.state.energyConsumed += ENERGY.MAC_OP;
    return this.state.accumulator;
  }

  /**
   * Clears the MAC accumulator
   */
  public clearAccumulator(): void {
    this.state.accumulator = 0;
  }

  /**
   * Gets the current accumulator value
   */
  public getAccumulator(): number {
    return this.state.accumulator;
  }

  /**
   * Moves accumulator high/low bytes to registers
   *
   * @param regHigh - Register for high byte
   * @param regLow - Register for low byte
   */
  public moveAccumulatorToRegs(regHigh: number, regLow: number): void {
    this.writeRegister(regHigh, (this.state.accumulator >> 8) & 0xFF);
    this.writeRegister(regLow, this.state.accumulator & 0xFF);
  }

  // ==========================================================================
  // ALU Operations
  // ==========================================================================

  /**
   * Updates status flags based on an operation result
   *
   * @param result - The operation result
   * @param operandA - First operand (for overflow detection)
   * @param operandB - Second operand (for overflow detection)
   * @param isSubtraction - Whether this was a subtraction operation
   */
  private updateFlags(
    result: number,
    operandA: number,
    operandB: number,
    isSubtraction: boolean = false
  ): void {
    const result8 = result & 0xFF;

    this.state.flags.zero = result8 === 0;
    this.state.flags.negative = (result8 & 0x80) !== 0;
    this.state.flags.carry = result > 0xFF || result < 0;

    // Overflow: sign of result differs from expected sign
    if (isSubtraction) {
      const signA = (operandA & 0x80) !== 0;
      const signB = (operandB & 0x80) !== 0;
      const signR = (result8 & 0x80) !== 0;
      this.state.flags.overflow = signA !== signB && signR !== signA;
    } else {
      const signA = (operandA & 0x80) !== 0;
      const signB = (operandB & 0x80) !== 0;
      const signR = (result8 & 0x80) !== 0;
      this.state.flags.overflow = signA === signB && signR !== signA;
    }
  }

  /**
   * Executes an ALU operation
   *
   * @param op - ALU operation code
   * @param a - First operand
   * @param b - Second operand (optional for unary ops)
   * @returns The 8-bit result
   */
  public aluOperation(op: AluOp, a: number, b: number = 0): number {
    let result: number;
    a = a & 0xFF;
    b = b & 0xFF;

    switch (op) {
      case AluOp.ADD:
        result = a + b;
        this.updateFlags(result, a, b, false);
        break;

      case AluOp.SUB:
        result = a - b;
        this.updateFlags(result, a, b, true);
        break;

      case AluOp.AND:
        result = a & b;
        this.updateFlags(result, a, b);
        break;

      case AluOp.OR:
        result = a | b;
        this.updateFlags(result, a, b);
        break;

      case AluOp.XOR:
        result = a ^ b;
        this.updateFlags(result, a, b);
        break;

      case AluOp.SHL:
        result = (a << (b & 0x07)) & 0xFF;
        this.state.flags.carry = ((a << ((b & 0x07) - 1)) & 0x80) !== 0;
        this.updateFlags(result, a, b);
        break;

      case AluOp.SHR:
        result = a >>> (b & 0x07);
        this.state.flags.carry = ((a >>> ((b & 0x07) - 1)) & 0x01) !== 0;
        this.updateFlags(result, a, b);
        break;

      case AluOp.SAR:
        // Arithmetic right shift preserves sign bit
        result = (a >> (b & 0x07)) & 0xFF;
        if (a & 0x80) {
          result |= (0xFF << (8 - (b & 0x07))) & 0xFF;
        }
        this.state.flags.carry = ((a >>> ((b & 0x07) - 1)) & 0x01) !== 0;
        this.updateFlags(result, a, b);
        break;

      case AluOp.NOT:
        result = (~a) & 0xFF;
        this.updateFlags(result, a, 0);
        break;

      case AluOp.NEG:
        result = (-a) & 0xFF;
        this.updateFlags(result, a, 0, true);
        break;

      case AluOp.INC:
        result = (a + 1) & 0xFF;
        this.updateFlags(result, a, 1, false);
        break;

      case AluOp.DEC:
        result = (a - 1) & 0xFF;
        this.updateFlags(result, a, 1, true);
        break;

      case AluOp.CMP:
        result = a - b;
        this.updateFlags(result, a, b, true);
        result = a; // CMP doesn't modify destination
        break;

      case AluOp.TST:
        result = a & b;
        this.updateFlags(result, a, b);
        result = a; // TST doesn't modify destination
        break;

      case AluOp.MOV:
        result = b;
        this.updateFlags(result, a, b);
        break;

      case AluOp.NOP:
      default:
        result = a;
        break;
    }

    this.state.energyConsumed += ENERGY.ALU_OP;
    return result & 0xFF;
  }

  // ==========================================================================
  // Neighbor Communication
  // ==========================================================================

  /**
   * Connects this Cognitum to a neighbor
   *
   * @param direction - Direction of the neighbor
   * @param neighbor - The neighbor Cognitum instance
   */
  public connectNeighbor(direction: Direction, neighbor: Cognitum): void {
    this.state.neighbors[direction].connected = neighbor;

    // Connect the reciprocal direction
    const reciprocal = (direction + 2) % 4;
    neighbor.state.neighbors[reciprocal].connected = this;
  }

  /**
   * Sends data to a neighbor
   *
   * @param direction - Direction to send (N, E, S, W)
   * @param data - 8-bit data to send
   * @returns true if send succeeded, false if neighbor not connected or busy
   */
  public sendToNeighbor(direction: Direction, data: number): boolean {
    const neighbor = this.state.neighbors[direction];

    if (!neighbor.connected) {
      return false; // No neighbor in this direction
    }

    neighbor.txBuffer = data & 0xFF;
    neighbor.txReady = true;

    // Transfer to neighbor's receive buffer
    const reciprocal = (direction + 2) % 4;
    const remoteNeighbor = neighbor.connected.state.neighbors[reciprocal];
    remoteNeighbor.rxBuffer = data & 0xFF;
    remoteNeighbor.rxValid = true;

    this.state.energyConsumed += ENERGY.NEIGHBOR_TX;
    return true;
  }

  /**
   * Receives data from a neighbor
   *
   * @param direction - Direction to receive from (N, E, S, W)
   * @returns The received data, or null if no data available
   */
  public receiveFromNeighbor(direction: Direction): number | null {
    const neighbor = this.state.neighbors[direction];

    if (!neighbor.rxValid) {
      return null;
    }

    const data = neighbor.rxBuffer;
    neighbor.rxValid = false;

    this.state.energyConsumed += ENERGY.NEIGHBOR_RX;
    return data;
  }

  /**
   * Broadcasts data to all connected neighbors
   *
   * @param data - 8-bit data to broadcast
   * @returns Number of neighbors that received the data
   */
  public broadcast(data: number): number {
    let count = 0;
    for (let dir = 0; dir < 4; dir++) {
      if (this.sendToNeighbor(dir as Direction, data)) {
        count++;
      }
    }
    return count;
  }

  /**
   * Receives from any neighbor that has data
   *
   * @returns Object with direction and data, or null if no data available
   */
  public receiveFromAny(): { direction: Direction; data: number } | null {
    for (let dir = 0; dir < 4; dir++) {
      const data = this.receiveFromNeighbor(dir as Direction);
      if (data !== null) {
        return { direction: dir as Direction, data };
      }
    }
    return null;
  }

  // ==========================================================================
  // Activation Functions
  // ==========================================================================

  /**
   * Applies ReLU activation function
   *
   * @param value - Input value (interpreted as signed 8-bit)
   * @returns max(0, value) as unsigned 8-bit
   */
  public relu(value: number): number {
    const signed = value > 127 ? value - 256 : value;
    return signed > 0 ? (signed & 0xFF) : 0;
  }

  /**
   * Applies sigmoid activation function using LUT
   *
   * @param value - Input value (0-255 mapped to -4 to +4)
   * @returns Output (0-255 representing 0.0 to 1.0)
   */
  public sigmoid(value: number): number {
    this.state.energyConsumed += ENERGY.LUT_ACCESS;
    return SIGMOID_LUT[value & 0xFF];
  }

  /**
   * Applies tanh activation function using LUT
   *
   * @param value - Input value (0-255 mapped to -4 to +4)
   * @returns Output (0-255, where 128 = 0.0)
   */
  public tanh(value: number): number {
    this.state.energyConsumed += ENERGY.LUT_ACCESS;
    return TANH_LUT[value & 0xFF];
  }

  // ==========================================================================
  // Spike Generation (LIF Neuron Model)
  // ==========================================================================

  /**
   * Accumulates input to the membrane potential
   *
   * @param input - Input value to accumulate (can be weighted synaptic input)
   */
  public accumulateMembrane(input: number): void {
    if (this.state.refractoryCounter > 0) {
      return; // In refractory period, ignore inputs
    }

    // Signed accumulation
    const signedInput = input > 127 ? input - 256 : input;
    this.state.membranePotential += signedInput * 256; // Scale to 16-bit

    // Clamp to valid range
    this.state.membranePotential = Math.max(
      -32768,
      Math.min(32767, this.state.membranePotential)
    );
  }

  /**
   * Applies leak to the membrane potential
   * Implements the "leaky" part of LIF
   */
  public applyLeak(): void {
    // Leak factor is 0-255 representing 0.0-1.0
    // Multiply by leak factor and divide by 256
    this.state.membranePotential = Math.floor(
      (this.state.membranePotential * this.state.leakFactor) / 256
    );
  }

  /**
   * Checks if membrane potential exceeds threshold and generates spike
   *
   * @returns true if a spike was generated
   */
  public checkAndSpike(): boolean {
    if (this.state.refractoryCounter > 0) {
      this.state.refractoryCounter--;
      this.state.flags.spike = false;
      return false;
    }

    if (this.state.membranePotential >= this.state.threshold) {
      // Fire spike!
      this.state.flags.spike = true;
      this.state.membranePotential = 0; // Reset
      this.state.refractoryCounter = 3; // 3-cycle refractory period
      this.state.energyConsumed += ENERGY.SPIKE_GEN;
      return true;
    }

    this.state.flags.spike = false;
    return false;
  }

  /**
   * Resets the membrane potential
   */
  public resetMembrane(): void {
    this.state.membranePotential = 0;
    this.state.refractoryCounter = 0;
    this.state.flags.spike = false;
  }

  /**
   * Gets the current membrane potential
   */
  public getMembranePotential(): number {
    return this.state.membranePotential;
  }

  /**
   * Sets the spike threshold
   *
   * @param threshold - 16-bit threshold value
   */
  public setThreshold(threshold: number): void {
    this.state.threshold = threshold & 0xFFFF;
  }

  /**
   * Sets the leak factor
   *
   * @param factor - Leak factor (0-255, representing 0.0-1.0)
   */
  public setLeakFactor(factor: number): void {
    this.state.leakFactor = factor & 0xFF;
  }

  /**
   * Legacy method: Set membrane potential directly
   *
   * @param value - Membrane potential value
   */
  public setMembrane(value: number): void {
    this.state.membranePotential = Math.round(value * 0x8000);
  }

  // ==========================================================================
  // Legacy Spike API (for backward compatibility)
  // ==========================================================================

  /**
   * Legacy method: Receive a spike from a neighboring Cognitum
   *
   * @param spike - Spike event to receive
   */
  public receiveSpike(spike: SpikeEvent): void {
    this.pendingSpikes.push(spike);
  }

  /**
   * Legacy method: Execute one clock cycle with spike processing
   *
   * @param currentTime - Current simulation time
   * @returns Array of output spike events
   */
  public tick(currentTime: number): SpikeEvent[] {
    this.state.cycleCount++;
    this.outputSpikes = [];

    // Process refractory period
    if (this.state.refractoryCounter > 0) {
      this.state.refractoryCounter--;
      this.pendingSpikes = [];
      return [];
    }

    // Integrate incoming spikes
    for (const spike of this.pendingSpikes) {
      this.accumulateMembrane(Math.round(spike.weight * 127));
    }
    this.pendingSpikes = [];

    // Leak (exponential decay)
    this.applyLeak();

    // Check for spike generation
    if (this.checkAndSpike()) {
      // Generate output spike
      const outputSpike: SpikeEvent = {
        sourceX: this.x,
        sourceY: this.y,
        timestamp: currentTime,
        weight: 1.0
      };
      this.outputSpikes.push(outputSpike);
    }

    return this.outputSpikes;
  }

  /**
   * Legacy method: Get performance metrics
   *
   * @param windowCycles - Number of cycles to calculate metrics over
   * @returns Performance metrics
   */
  public getMetrics(windowCycles: number): CognitumMetrics {
    const spikeCount = this.state.flags.spike ? 1 : 0;
    const spikeRate = windowCycles > 0
      ? (spikeCount / windowCycles) * this.clockSpeedMHz * 1e6
      : 0;

    const staticPower = Cognitum.STATIC_POWER_UW;
    const dynamicPower = Cognitum.DYNAMIC_POWER_PER_MHZ * this.clockSpeedMHz;
    const spikePower = spikeRate * Cognitum.SPIKE_POWER_UW / 1e6;

    return {
      spikeRate,
      powerMicroWatts: staticPower + dynamicPower + spikePower,
      utilization: Math.min(1.0, this.state.cycleCount > 0 ? 0.5 : 0),
      memoryUsed: this.state.sram.reduce((sum, byte) => sum + (byte !== 0 ? 1 : 0), 0)
    };
  }

  // ==========================================================================
  // Power Management
  // ==========================================================================

  /**
   * Transitions to idle power state
   */
  public enterIdle(): void {
    this.state.powerState = PowerState.IDLE;
  }

  /**
   * Transitions to sleep power state
   */
  public enterSleep(): void {
    this.state.powerState = PowerState.SLEEP;
  }

  /**
   * Wakes up from idle or sleep state
   */
  public wake(): void {
    this.state.powerState = PowerState.ACTIVE;
  }

  // ==========================================================================
  // Instruction Decoder
  // ==========================================================================

  /**
   * Fetches and decodes the next instruction
   *
   * @returns Decoded instruction structure
   */
  public fetchAndDecode(): DecodedInstruction {
    const opcode = this.state.sram[this.state.pc];
    let rd = 0, rs1 = 0, rs2OrImm = 0;
    let aluOp: AluOp | undefined;
    let direction: Direction | undefined;
    let instructionLength = 1;

    // Decode based on opcode category
    if (opcode <= 0x04) {
      // Load/Store: 2-byte format [opcode][rd:4|rs1/dir:4][address/imm]
      const byte1 = this.state.sram[(this.state.pc + 1) & 0x1FF];
      const byte2 = this.state.sram[(this.state.pc + 2) & 0x1FF];
      rd = (byte1 >> 4) & 0x0F;
      rs1 = byte1 & 0x0F;
      rs2OrImm = byte2;
      direction = rs1 as Direction;
      instructionLength = 3;
    } else if (opcode === Opcode.ALU) {
      // ALU: 2-byte format [opcode][aluop:4|rd:4][rs1:4|rs2:4]
      const byte1 = this.state.sram[(this.state.pc + 1) & 0x1FF];
      const byte2 = this.state.sram[(this.state.pc + 2) & 0x1FF];
      aluOp = (byte1 >> 4) & 0x0F;
      rd = byte1 & 0x0F;
      rs1 = (byte2 >> 4) & 0x0F;
      rs2OrImm = byte2 & 0x0F;
      instructionLength = 3;
    } else if (opcode >= 0x20 && opcode <= 0x23) {
      // MAC operations: 2-byte format [opcode][rs1:4|rs2:4]
      const byte1 = this.state.sram[(this.state.pc + 1) & 0x1FF];
      rs1 = (byte1 >> 4) & 0x0F;
      rs2OrImm = byte1 & 0x0F;
      rd = byte1 & 0x0F; // For MOV_ACC, rd is low register
      instructionLength = 2;
    } else if (opcode >= 0x30 && opcode <= 0x36) {
      // Branch/Control: 2-byte format [opcode][target_low][target_high]
      const byte1 = this.state.sram[(this.state.pc + 1) & 0x1FF];
      const byte2 = this.state.sram[(this.state.pc + 2) & 0x1FF];
      rs2OrImm = byte1 | (byte2 << 8); // 16-bit address
      instructionLength = 3;
    } else if (opcode >= 0x40 && opcode <= 0x46) {
      // Neural operations: 1 or 2 byte
      if (opcode === Opcode.ACCUM || opcode === Opcode.RELU ||
          opcode === Opcode.SIGMOID || opcode === Opcode.TANH) {
        const byte1 = this.state.sram[(this.state.pc + 1) & 0x1FF];
        rd = (byte1 >> 4) & 0x0F;
        rs1 = byte1 & 0x0F;
        instructionLength = 2;
      } else {
        instructionLength = 1;
      }
    } else if (opcode >= 0x60 && opcode <= 0x62) {
      // Sync/Communication: 2-byte format
      const byte1 = this.state.sram[(this.state.pc + 1) & 0x1FF];
      rd = (byte1 >> 4) & 0x0F;
      direction = (byte1 & 0x03) as Direction;
      instructionLength = 2;
    } else {
      // Single-byte instructions
      instructionLength = 1;
    }

    // Extract raw bytes
    const raw = new Uint8Array(instructionLength);
    for (let i = 0; i < instructionLength; i++) {
      raw[i] = this.state.sram[(this.state.pc + i) & 0x1FF];
    }

    return {
      opcode: opcode as Opcode,
      rd,
      rs1,
      rs2OrImm,
      aluOp,
      direction,
      raw,
    };
  }

  // ==========================================================================
  // Instruction Execution
  // ==========================================================================

  /**
   * Executes a single decoded instruction
   *
   * @param inst - The decoded instruction to execute
   * @returns Execution result with cycle count and effects
   */
  public execute(inst: DecodedInstruction): ExecutionResult {
    const result: ExecutionResult = {
      cycles: 1,
      energy: 0,
      spiked: false,
      halt: false,
    };

    switch (inst.opcode) {
      // ======== Load/Store ========
      case Opcode.LDR: {
        const address = (this.readRegister(inst.rs1) + inst.rs2OrImm) & 0x1FF;
        const value = this.loadSRAM(address);
        this.writeRegister(inst.rd, value);
        result.cycles = 2;
        break;
      }

      case Opcode.STR: {
        const address = (this.readRegister(inst.rs1) + inst.rs2OrImm) & 0x1FF;
        const value = this.readRegister(inst.rd);
        this.storeSRAM(address, value);
        result.cycles = 2;
        break;
      }

      case Opcode.LDI: {
        this.writeRegister(inst.rd, inst.rs2OrImm);
        result.cycles = 1;
        break;
      }

      case Opcode.LDN: {
        const data = this.receiveFromNeighbor(inst.direction!);
        if (data !== null) {
          this.writeRegister(inst.rd, data);
        } else {
          this.writeRegister(inst.rd, 0);
          this.state.flags.zero = true;
        }
        result.cycles = 2;
        break;
      }

      case Opcode.STN: {
        const value = this.readRegister(inst.rd);
        this.sendToNeighbor(inst.direction!, value);
        result.cycles = 2;
        break;
      }

      // ======== ALU Operations ========
      case Opcode.ALU: {
        const a = this.readRegister(inst.rs1);
        const b = this.readRegister(inst.rs2OrImm);
        const aluResult = this.aluOperation(inst.aluOp!, a, b);
        if (inst.aluOp !== AluOp.CMP && inst.aluOp !== AluOp.TST) {
          this.writeRegister(inst.rd, aluResult);
        }
        result.cycles = 1;
        break;
      }

      // ======== MAC Operations ========
      case Opcode.MAC: {
        const a = this.readRegister(inst.rs1);
        const b = this.readRegister(inst.rs2OrImm);
        this.mac(a, b);
        result.cycles = 2;
        break;
      }

      case Opcode.MUL: {
        const a = this.readRegister(inst.rs1);
        const b = this.readRegister(inst.rs2OrImm);
        this.multiply(a, b);
        result.cycles = 2;
        break;
      }

      case Opcode.CLR_ACC: {
        this.clearAccumulator();
        result.cycles = 1;
        break;
      }

      case Opcode.MOV_ACC: {
        // rs1 is high register, rs2OrImm is low register
        this.moveAccumulatorToRegs(inst.rs1, inst.rs2OrImm);
        result.cycles = 1;
        break;
      }

      // ======== Branch/Control ========
      case Opcode.JMP: {
        result.branchTarget = inst.rs2OrImm & 0x1FF;
        result.cycles = 2;
        break;
      }

      case Opcode.JZ: {
        if (this.state.flags.zero) {
          result.branchTarget = inst.rs2OrImm & 0x1FF;
          result.cycles = 3;
        } else {
          result.cycles = 2;
        }
        break;
      }

      case Opcode.JNZ: {
        if (!this.state.flags.zero) {
          result.branchTarget = inst.rs2OrImm & 0x1FF;
          result.cycles = 3;
        } else {
          result.cycles = 2;
        }
        break;
      }

      case Opcode.JC: {
        if (this.state.flags.carry) {
          result.branchTarget = inst.rs2OrImm & 0x1FF;
          result.cycles = 3;
        } else {
          result.cycles = 2;
        }
        break;
      }

      case Opcode.JNC: {
        if (!this.state.flags.carry) {
          result.branchTarget = inst.rs2OrImm & 0x1FF;
          result.cycles = 3;
        } else {
          result.cycles = 2;
        }
        break;
      }

      case Opcode.CALL: {
        // Push return address (PC + instruction length)
        const returnAddr = (this.state.pc + inst.raw.length) & 0x1FF;
        this.storeSRAM(this.state.sp, returnAddr & 0xFF);
        this.state.sp = (this.state.sp - 1) & 0x1FF;
        this.storeSRAM(this.state.sp, (returnAddr >> 8) & 0xFF);
        this.state.sp = (this.state.sp - 1) & 0x1FF;
        result.branchTarget = inst.rs2OrImm & 0x1FF;
        result.cycles = 4;
        break;
      }

      case Opcode.RET: {
        // Pop return address
        this.state.sp = (this.state.sp + 1) & 0x1FF;
        const highByte = this.loadSRAM(this.state.sp);
        this.state.sp = (this.state.sp + 1) & 0x1FF;
        const lowByte = this.loadSRAM(this.state.sp);
        result.branchTarget = (highByte << 8) | lowByte;
        result.cycles = 4;
        break;
      }

      // ======== Neural Operations ========
      case Opcode.SPIKE: {
        result.spiked = this.checkAndSpike();
        result.cycles = 1;
        break;
      }

      case Opcode.ACCUM: {
        const input = this.readRegister(inst.rs1);
        this.accumulateMembrane(input);
        result.cycles = 1;
        break;
      }

      case Opcode.DECAY: {
        this.applyLeak();
        result.cycles = 1;
        break;
      }

      case Opcode.RESET: {
        this.resetMembrane();
        result.cycles = 1;
        break;
      }

      case Opcode.RELU: {
        const input = this.readRegister(inst.rs1);
        const output = this.relu(input);
        this.writeRegister(inst.rd, output);
        result.cycles = 1;
        break;
      }

      case Opcode.SIGMOID: {
        const input = this.readRegister(inst.rs1);
        const output = this.sigmoid(input);
        this.writeRegister(inst.rd, output);
        result.cycles = 2;
        break;
      }

      case Opcode.TANH: {
        const input = this.readRegister(inst.rs1);
        const output = this.tanh(input);
        this.writeRegister(inst.rd, output);
        result.cycles = 2;
        break;
      }

      // ======== Power Management ========
      case Opcode.IDLE: {
        this.enterIdle();
        result.cycles = 1;
        break;
      }

      case Opcode.SLEEP: {
        this.enterSleep();
        result.cycles = 1;
        break;
      }

      case Opcode.WAKE: {
        this.wake();
        result.cycles = 3; // Wake-up latency
        break;
      }

      // ======== Sync/Communication ========
      case Opcode.SYNC: {
        // Barrier sync - in simulation, this is a no-op
        // Real implementation would wait for all neighbors
        result.cycles = 4;
        break;
      }

      case Opcode.BCAST: {
        const value = this.readRegister(inst.rd);
        this.broadcast(value);
        result.cycles = 4;
        break;
      }

      case Opcode.RECV: {
        const received = this.receiveFromAny();
        if (received) {
          this.writeRegister(inst.rd, received.data);
        } else {
          this.writeRegister(inst.rd, 0);
          this.state.flags.zero = true;
        }
        result.cycles = 2;
        break;
      }

      // ======== Special ========
      case Opcode.HALT: {
        result.halt = true;
        result.cycles = 1;
        break;
      }

      default: {
        // Unknown opcode - treat as NOP
        result.cycles = 1;
        break;
      }
    }

    result.energy = this.state.energyConsumed;
    return result;
  }

  /**
   * Executes a single instruction cycle
   *
   * @returns Execution result
   */
  public step(): ExecutionResult {
    if (this.state.halted) {
      return {
        cycles: 0,
        energy: 0,
        spiked: false,
        halt: true,
      };
    }

    if (this.state.powerState !== PowerState.ACTIVE) {
      // In idle or sleep, just count cycles and energy
      this.state.cycleCount++;
      const energy = this.state.powerState === PowerState.IDLE
        ? ENERGY.IDLE_CYCLE
        : ENERGY.SLEEP_CYCLE;
      this.state.energyConsumed += energy;
      return {
        cycles: 1,
        energy,
        spiked: false,
        halt: false,
      };
    }

    const energyBefore = this.state.energyConsumed;
    const inst = this.fetchAndDecode();
    const result = this.execute(inst);

    // Update PC
    if (result.branchTarget !== undefined) {
      this.state.pc = result.branchTarget;
    } else {
      this.state.pc = (this.state.pc + inst.raw.length) & 0x1FF;
    }

    // Update halt state
    if (result.halt) {
      this.state.halted = true;
    }

    // Update cycle count
    this.state.cycleCount += result.cycles;
    result.energy = this.state.energyConsumed - energyBefore;

    return result;
  }

  /**
   * Runs the simulator for a specified number of cycles
   *
   * @param maxCycles - Maximum cycles to run
   * @returns Statistics about the run
   */
  public run(maxCycles: number): {
    cyclesExecuted: number;
    instructionsExecuted: number;
    spikesGenerated: number;
    energyConsumed: number;
    halted: boolean;
  } {
    const startCycles = this.state.cycleCount;
    const startEnergy = this.state.energyConsumed;
    let instructionsExecuted = 0;
    let spikesGenerated = 0;

    while (
      this.state.cycleCount - startCycles < maxCycles &&
      !this.state.halted
    ) {
      const result = this.step();
      instructionsExecuted++;
      if (result.spiked) {
        spikesGenerated++;
      }
    }

    return {
      cyclesExecuted: this.state.cycleCount - startCycles,
      instructionsExecuted,
      spikesGenerated,
      energyConsumed: this.state.energyConsumed - startEnergy,
      halted: this.state.halted,
    };
  }

  /**
   * Resets the Cognitum to initial state
   */
  public reset(): void {
    this.state = this.createInitialState();
    this.pendingSpikes = [];
    this.outputSpikes = [];
  }

  // ==========================================================================
  // Debug/Inspection Methods
  // ==========================================================================

  /**
   * Gets a string representation of the current state
   */
  public toString(): string {
    const regs = Array.from(this.state.registers)
      .map((v, i) => `R${i}:${v.toString(16).padStart(2, '0')}`)
      .join(' ');

    const flags = [
      this.state.flags.zero ? 'Z' : '-',
      this.state.flags.carry ? 'C' : '-',
      this.state.flags.negative ? 'N' : '-',
      this.state.flags.overflow ? 'O' : '-',
      this.state.flags.spike ? 'S' : '-',
    ].join('');

    return [
      `Cognitum[${this.x},${this.y}] PC:${this.state.pc.toString(16).padStart(3, '0')}`,
      `Flags:${flags} ACC:${this.state.accumulator.toString(16).padStart(4, '0')}`,
      `Membrane:${this.state.membranePotential} Power:${this.state.powerState}`,
      `Cycles:${this.state.cycleCount} Energy:${this.state.energyConsumed.toFixed(2)}fJ`,
      regs,
    ].join('\n');
  }

  /**
   * Disassembles an instruction at the given address
   *
   * @param address - SRAM address to disassemble
   * @returns Disassembled instruction string
   */
  public disassemble(address: number): string {
    const savedPc = this.state.pc;
    this.state.pc = address;
    const inst = this.fetchAndDecode();
    this.state.pc = savedPc;

    const hex = Array.from(inst.raw)
      .map(b => b.toString(16).padStart(2, '0'))
      .join(' ');

    let mnemonic = Opcode[inst.opcode] || `???`;
    let operands = '';

    switch (inst.opcode) {
      case Opcode.LDR:
        operands = `R${inst.rd}, [R${inst.rs1}+${inst.rs2OrImm}]`;
        break;
      case Opcode.STR:
        operands = `[R${inst.rs1}+${inst.rs2OrImm}], R${inst.rd}`;
        break;
      case Opcode.LDI:
        operands = `R${inst.rd}, #${inst.rs2OrImm}`;
        break;
      case Opcode.LDN:
      case Opcode.STN:
        operands = `R${inst.rd}, ${Direction[inst.direction!]}`;
        break;
      case Opcode.ALU:
        mnemonic = AluOp[inst.aluOp!] || 'ALU';
        operands = `R${inst.rd}, R${inst.rs1}, R${inst.rs2OrImm}`;
        break;
      case Opcode.MAC:
      case Opcode.MUL:
        operands = `R${inst.rs1}, R${inst.rs2OrImm}`;
        break;
      case Opcode.MOV_ACC:
        operands = `R${inst.rs1}:R${inst.rs2OrImm}`;
        break;
      case Opcode.JMP:
      case Opcode.JZ:
      case Opcode.JNZ:
      case Opcode.JC:
      case Opcode.JNC:
      case Opcode.CALL:
        operands = `0x${(inst.rs2OrImm & 0x1FF).toString(16).padStart(3, '0')}`;
        break;
      case Opcode.RELU:
      case Opcode.SIGMOID:
      case Opcode.TANH:
        operands = `R${inst.rd}, R${inst.rs1}`;
        break;
      case Opcode.ACCUM:
        operands = `R${inst.rs1}`;
        break;
      case Opcode.BCAST:
      case Opcode.RECV:
        operands = `R${inst.rd}`;
        break;
    }

    return `${address.toString(16).padStart(3, '0')}: ${hex.padEnd(12)} ${mnemonic.padEnd(8)} ${operands}`;
  }
}

// ============================================================================
// Mesh Array Helper
// ============================================================================

/**
 * Creates a 2D mesh of connected Cognitum chips
 *
 * @param width - Width of the mesh
 * @param height - Height of the mesh
 * @returns 2D array of connected Cognitum instances
 */
export function createMesh(width: number, height: number): Cognitum[][] {
  // Create the array
  const mesh: Cognitum[][] = [];
  for (let y = 0; y < height; y++) {
    mesh[y] = [];
    for (let x = 0; x < width; x++) {
      mesh[y][x] = new Cognitum(x, y);
    }
  }

  // Connect neighbors
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const chip = mesh[y][x];

      if (y > 0) {
        chip.connectNeighbor(Direction.NORTH, mesh[y - 1][x]);
      }
      if (x < width - 1) {
        chip.connectNeighbor(Direction.EAST, mesh[y][x + 1]);
      }
      if (y < height - 1) {
        chip.connectNeighbor(Direction.SOUTH, mesh[y + 1][x]);
      }
      if (x > 0) {
        chip.connectNeighbor(Direction.WEST, mesh[y][x - 1]);
      }
    }
  }

  return mesh;
}

/**
 * Runs a synchronous step across all chips in a mesh
 *
 * @param mesh - The mesh to step
 * @returns Total spikes generated
 */
export function meshStep(mesh: Cognitum[][]): number {
  let totalSpikes = 0;
  for (const row of mesh) {
    for (const chip of row) {
      const result = chip.step();
      if (result.spiked) {
        totalSpikes++;
      }
    }
  }
  return totalSpikes;
}
