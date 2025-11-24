import { describe, it } from 'node:test'
import assert from 'node:assert'
import { NewportNode } from '../index.js'

describe('Newport NAPI', () => {
  it('should create Newport instance', async () => {
    const newport = new NewportNode()
    assert.ok(newport)
  })

  it('should create Newport with custom config', async () => {
    const config = {
      num_tiles: 8,
      memory_size: 512 * 1024,
      clock_freq_mhz: 800,
      enable_debug: false,
    }
    const newport = new NewportNode(config)
    const readConfig = await newport.getConfig()
    assert.strictEqual(readConfig.num_tiles, 8)
    assert.strictEqual(readConfig.memory_size, 512 * 1024)
    assert.strictEqual(readConfig.clock_freq_mhz, 800)
  })

  it('should load program into tile', async () => {
    const newport = new NewportNode()
    const program = Buffer.from([0x01, 0x02, 0x03, 0x04])
    await newport.loadProgram(0, program)
    const state = await newport.getState(0)
    assert.strictEqual(state.status, 'ready')
  })

  it('should run cycles async', async () => {
    const newport = new NewportNode()
    const program = Buffer.from([0x01, 0x02, 0x03, 0x04])
    await newport.loadProgram(0, program)
    await newport.runCycles(100)
    const cycles = await newport.getTotalCycles()
    assert.strictEqual(cycles, 100n)
  })

  it('should run cycles sync', async () => {
    const newport = new NewportNode()
    const program = Buffer.from([0x01, 0x02, 0x03, 0x04])
    await newport.loadProgram(0, program)
    newport.runCyclesSync(50)
    const cycles = await newport.getTotalCycles()
    assert.strictEqual(cycles, 50n)
  })

  it('should get and set register values', async () => {
    const newport = new NewportNode()
    await newport.setRegister(0, 5, 42)
    const value = await newport.getRegister(0, 5)
    assert.strictEqual(value, 42)
  })

  it('should reset tile', async () => {
    const newport = new NewportNode()
    await newport.setRegister(0, 1, 100)
    await newport.resetTile(0)
    const value = await newport.getRegister(0, 1)
    assert.strictEqual(value, 0)
  })

  it('should reset all tiles', async () => {
    const newport = new NewportNode()
    await newport.setRegister(0, 1, 100)
    await newport.setRegister(1, 2, 200)
    await newport.resetAll()
    const val1 = await newport.getRegister(0, 1)
    const val2 = await newport.getRegister(1, 2)
    assert.strictEqual(val1, 0)
    assert.strictEqual(val2, 0)
  })

  it('should get all states', async () => {
    const config = {
      num_tiles: 4,
      memory_size: 1024 * 1024,
      clock_freq_mhz: 1000,
      enable_debug: false,
    }
    const newport = new NewportNode(config)
    const states = await newport.getAllStates()
    assert.strictEqual(states.length, 4)
    states.forEach((state, idx) => {
      assert.strictEqual(state.tile_id, idx)
    })
  })

  it('should get performance metrics', async () => {
    const newport = new NewportNode()
    const program = Buffer.from([0x01, 0x02, 0x03, 0x04])
    await newport.loadProgram(0, program)
    await newport.runCycles(1000)
    const metrics = await newport.getMetrics()
    assert.strictEqual(metrics.total_cycles, 1000n)
    assert.ok(metrics.active_tiles >= 0)
  })

  it('should handle invalid tile ID', async () => {
    const newport = new NewportNode()
    await assert.rejects(
      async () => await newport.getState(255),
      /Invalid tile ID/
    )
  })

  it('should handle invalid register number', async () => {
    const newport = new NewportNode()
    await assert.rejects(
      async () => await newport.getRegister(0, 999),
      /Invalid register number/
    )
  })

  it('should handle zero cycles', async () => {
    const newport = new NewportNode()
    await assert.rejects(
      async () => await newport.runCycles(0),
      /Cycles must be greater than 0/
    )
  })
})
