//! Unit tests for RaceWayBridge

#[cfg(test)]
mod raceway_bridge {
    use cognitum::ruvector::{RaceWayBridge, DefaultRaceWayBridge};
    use cognitum::ruvector::types::{TileId, GroupId, VectorOp};

    #[test]
    fn should_send_message() {
        let bridge = DefaultRaceWayBridge::new(16);

        let payload = [0u8; 12];
        let result = bridge.send_message(TileId(0), TileId(5), &payload);

        assert!(result.is_ok());
    }

    #[test]
    fn should_broadcast() {
        let bridge = DefaultRaceWayBridge::new(16);

        let payload = [0xAB; 12];
        let result = bridge.broadcast(TileId(0), &payload);

        assert!(result.is_ok());
    }

    #[test]
    fn should_create_tile_group() {
        let mut bridge = DefaultRaceWayBridge::new(16);

        let tiles = vec![TileId(0), TileId(1), TileId(2), TileId(3)];
        let result = bridge.create_group(&tiles);

        assert!(result.is_ok());
        let group = result.unwrap();
        assert_eq!(group.0, 0);
    }

    #[test]
    fn should_reject_invalid_tile_id() {
        let bridge = DefaultRaceWayBridge::new(8);

        let payload = [0u8; 12];
        let result = bridge.send_message(TileId(0), TileId(100), &payload);

        assert!(result.is_err());
    }

    #[test]
    fn should_reject_empty_group() {
        let mut bridge = DefaultRaceWayBridge::new(16);

        let tiles: Vec<TileId> = vec![];
        let result = bridge.create_group(&tiles);

        assert!(result.is_err());
    }

    #[test]
    fn should_execute_parallel_sum() {
        let mut bridge = DefaultRaceWayBridge::new(16);

        let tiles = vec![TileId(0), TileId(1), TileId(2), TileId(3)];
        let group = bridge.create_group(&tiles).unwrap();

        let data: Vec<f32> = (0..16).map(|i| i as f32).collect();
        let result = bridge.parallel_op(group, VectorOp::Sum, &data);

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(!output.is_empty());
    }

    #[test]
    fn should_execute_parallel_normalize() {
        let mut bridge = DefaultRaceWayBridge::new(4);

        let tiles = vec![TileId(0), TileId(1)];
        let group = bridge.create_group(&tiles).unwrap();

        let data: Vec<f32> = vec![3.0, 4.0, 0.0, 0.0]; // Will be split
        let result = bridge.parallel_op(group, VectorOp::Normalize, &data);

        assert!(result.is_ok());
    }

    #[test]
    fn should_fail_parallel_op_with_invalid_group() {
        let bridge = DefaultRaceWayBridge::new(16);

        let data: Vec<f32> = vec![1.0, 2.0, 3.0];
        let result = bridge.parallel_op(GroupId(999), VectorOp::Sum, &data);

        assert!(result.is_err());
    }
}
