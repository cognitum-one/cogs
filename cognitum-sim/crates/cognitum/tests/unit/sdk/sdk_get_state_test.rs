//! Unit tests for CognitumSDK::get_state

use cognitum::sdk::{CognitumSDK, InternalState, MockSimulator, TileId, TileState};

#[test]
fn should_delegate_to_simulator_state() {
    let mut mock = MockSimulator::new();
    mock.expect_get_state().times(1).returning(|| InternalState {
        tiles: vec![
            TileState {
                id: TileId(0),
                program_counter: 100,
                stack_pointer: 50,
            },
            TileState {
                id: TileId(1),
                program_counter: 200,
                stack_pointer: 60,
            },
        ],
        cycle: 500,
        memory: vec![],
    });

    let sdk = CognitumSDK::with_simulator(Box::new(mock));

    let state = sdk.get_state();

    assert_eq!(state.tiles.len(), 2);
    assert_eq!(state.current_cycle, 500);
}

#[test]
fn should_include_program_loaded_status() {
    let mut mock = MockSimulator::new();
    mock.expect_get_state()
        .returning(|| InternalState::default());

    let mut sdk = CognitumSDK::with_simulator(Box::new(mock));

    // Before loading
    let state1 = sdk.get_state();
    assert!(!state1.program_loaded);

    // After loading (simulated)
    sdk.mark_program_loaded();
    let state2 = sdk.get_state();
    assert!(state2.program_loaded);
}

#[test]
fn should_return_tile_states() {
    let mut mock = MockSimulator::new();
    mock.expect_get_state().returning(|| InternalState {
        tiles: vec![
            TileState {
                id: TileId(0),
                program_counter: 0,
                stack_pointer: 0,
            },
            TileState {
                id: TileId(1),
                program_counter: 42,
                stack_pointer: 10,
            },
        ],
        cycle: 100,
        memory: vec![],
    });

    let sdk = CognitumSDK::with_simulator(Box::new(mock));
    let state = sdk.get_state();

    assert_eq!(state.tiles.len(), 2);
    assert_eq!(state.tiles[1].program_counter, 42);
    assert_eq!(state.tiles[1].stack_pointer, 10);
}

#[test]
fn should_return_memory_snapshot() {
    let mut mock = MockSimulator::new();
    mock.expect_get_state().returning(|| InternalState {
        tiles: vec![],
        cycle: 0,
        memory: vec![0xAB; 256],
    });

    let sdk = CognitumSDK::with_simulator(Box::new(mock));
    let state = sdk.get_state();

    let memory = state.get_memory(0, 256);
    assert_eq!(memory.len(), 256);
    assert_eq!(memory[0], 0xAB);
}
