//! RaceWay bridge for parallel vector operations

use crate::ruvector::types::*;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

#[cfg(test)]
use mockall::automock;

/// Trait for RaceWay communication and parallel operations
#[cfg_attr(test, automock)]
pub trait RaceWayBridge: Send + Sync {
    /// Send message between tiles
    fn send_message(&self, from: TileId, to: TileId, payload: &[u8; 12])
        -> Result<(), RaceWayError>;

    /// Broadcast to all tiles
    fn broadcast(&self, from: TileId, payload: &[u8; 12]) -> Result<(), RaceWayError>;

    /// Create tile group for parallel ops
    fn create_group(&mut self, tiles: &[TileId]) -> Result<GroupId, RaceWayError>;

    /// Execute parallel vector operation on group
    fn parallel_op(&self, group: GroupId, op: VectorOp, data: &[f32])
        -> Result<Vec<f32>, RaceWayError>;
}

/// Default implementation of RaceWay bridge
pub struct DefaultRaceWayBridge {
    num_tiles: usize,
    groups: Arc<RwLock<HashMap<GroupId, Vec<TileId>>>>,
    next_group_id: Arc<RwLock<u32>>,
}

impl DefaultRaceWayBridge {
    pub fn new(num_tiles: usize) -> Self {
        Self {
            num_tiles,
            groups: Arc::new(RwLock::new(HashMap::new())),
            next_group_id: Arc::new(RwLock::new(0)),
        }
    }

    fn execute_op_on_tile(&self, _tile: TileId, op: VectorOp, data: &[f32]) -> Vec<f32> {
        // Simulate tile operation
        match op {
            VectorOp::Sum => vec![data.iter().sum()],
            VectorOp::DotProduct => {
                // Assume data is two vectors concatenated
                let mid = data.len() / 2;
                let a = &data[..mid];
                let b = &data[mid..];
                vec![a.iter().zip(b).map(|(x, y)| x * y).sum()]
            }
            VectorOp::MatrixMultiply => {
                // Simplified: return sum for demo
                vec![data.iter().sum()]
            }
            VectorOp::Normalize => {
                let norm: f32 = data.iter().map(|x| x * x).sum::<f32>().sqrt();
                if norm == 0.0 {
                    data.to_vec()
                } else {
                    data.iter().map(|x| x / norm).collect()
                }
            }
        }
    }
}

impl RaceWayBridge for DefaultRaceWayBridge {
    fn send_message(&self, from: TileId, to: TileId, _payload: &[u8; 12])
        -> Result<(), RaceWayError> {
        if from.0 >= self.num_tiles as u32 || to.0 >= self.num_tiles as u32 {
            return Err(RaceWayError::Communication(
                format!("Invalid tile ID: from={}, to={}", from.0, to.0)
            ));
        }

        // Simulate message send
        Ok(())
    }

    fn broadcast(&self, from: TileId, _payload: &[u8; 12]) -> Result<(), RaceWayError> {
        if from.0 >= self.num_tiles as u32 {
            return Err(RaceWayError::Communication(
                format!("Invalid tile ID: {}", from.0)
            ));
        }

        // Simulate broadcast
        Ok(())
    }

    fn create_group(&mut self, tiles: &[TileId]) -> Result<GroupId, RaceWayError> {
        if tiles.is_empty() {
            return Err(RaceWayError::InvalidGroup("Empty tile group".to_string()));
        }

        // Validate all tile IDs
        for tile in tiles {
            if tile.0 >= self.num_tiles as u32 {
                return Err(RaceWayError::InvalidGroup(
                    format!("Invalid tile ID: {}", tile.0)
                ));
            }
        }

        let mut next_id = self.next_group_id.write();
        let group_id = GroupId(*next_id);
        *next_id += 1;

        let mut groups = self.groups.write();
        groups.insert(group_id, tiles.to_vec());

        Ok(group_id)
    }

    fn parallel_op(&self, group: GroupId, op: VectorOp, data: &[f32])
        -> Result<Vec<f32>, RaceWayError> {
        let groups = self.groups.read();
        let tiles = groups.get(&group)
            .ok_or_else(|| RaceWayError::InvalidGroup(format!("Group {:?} not found", group)))?;

        if tiles.is_empty() {
            return Err(RaceWayError::InvalidGroup("Empty tile group".to_string()));
        }

        // Split data across tiles
        let chunk_size = (data.len() + tiles.len() - 1) / tiles.len();

        let results: Vec<Vec<f32>> = tiles
            .iter()
            .enumerate()
            .map(|(i, tile)| {
                let start = i * chunk_size;
                let end = ((i + 1) * chunk_size).min(data.len());
                if start < data.len() {
                    self.execute_op_on_tile(*tile, op, &data[start..end])
                } else {
                    vec![]
                }
            })
            .collect();

        // Combine results
        let combined: Vec<f32> = results.into_iter().flatten().collect();
        Ok(combined)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_send_message() {
        let bridge = DefaultRaceWayBridge::new(16);
        let payload = [0u8; 12];

        let result = bridge.send_message(TileId(0), TileId(5), &payload);
        assert!(result.is_ok());
    }

    #[test]
    fn test_create_group() {
        let mut bridge = DefaultRaceWayBridge::new(16);

        let tiles = vec![TileId(0), TileId(1), TileId(2), TileId(3)];
        let group = bridge.create_group(&tiles).unwrap();

        assert_eq!(group.0, 0);
    }

    #[test]
    fn test_parallel_sum() {
        let mut bridge = DefaultRaceWayBridge::new(16);

        let tiles = vec![TileId(0), TileId(1), TileId(2), TileId(3)];
        let group = bridge.create_group(&tiles).unwrap();

        let data: Vec<f32> = (0..16).map(|i| i as f32).collect();
        let result = bridge.parallel_op(group, VectorOp::Sum, &data).unwrap();

        // Should have results from 4 tiles
        assert_eq!(result.len(), 4);
    }

    #[test]
    fn test_invalid_tile_id() {
        let bridge = DefaultRaceWayBridge::new(8);
        let payload = [0u8; 12];

        let result = bridge.send_message(TileId(0), TileId(100), &payload);
        assert!(matches!(result, Err(RaceWayError::Communication(_))));
    }
}
