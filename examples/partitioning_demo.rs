//! Demo of ruvector-mincut partitioning for Cognitum chip tiles

use cognitum::ruvector::partitioning::{
    TileGraph, KernighanLinPartitioner, MinCutPartitioner, WorkloadPartitioner,
    helpers,
};
use cognitum::ruvector::types::{TileId, TaskEmbedding};
use cognitum::ruvector::router::{TinyDancerRouter, TaskRouter};

fn main() {
    println!("=== Cognitum Chip Tile Partitioning Demo ===\n");

    // 1. Create 16x16 tile graph with RaceWay topology
    println!("1. Creating 16x16 tile graph (256 tiles)...");
    let graph = TileGraph::new();
    println!("   ✓ Graph created with {} nodes", graph.node_count());

    // 2. K-L partitioning into 4 quadrants
    println!("\n2. Running Kernighan-Lin partitioning (k=4)...");
    let mut partitioner = KernighanLinPartitioner::new(graph);

    match partitioner.partition(4) {
        Ok(partitions) => {
            println!("   ✓ Partitioning successful!");
            for partition in &partitions {
                println!("   Partition {}: {} tiles, {} internal edges, {} external edges",
                    partition.id.0,
                    partition.size(),
                    partition.internal_edges,
                    partition.external_edges
                );
            }
            println!("   Min cut value: {}", partitioner.min_cut_value());
        }
        Err(e) => println!("   ✗ Error: {}", e),
    }

    // 3. Hierarchical partitioning
    println!("\n3. Hierarchical partitioning (16 partitions)...");
    let mut graph2 = TileGraph::new();
    if let Err(e) = helpers::hierarchical_partition(&mut graph2, 16) {
        println!("   ✗ Error: {}", e);
    } else {
        let partitions = graph2.get_partitions(16);
        println!("   ✓ Created {} partitions", partitions.len());
        println!("   Average partition size: {:.1} tiles",
            partitions.iter().map(|p| p.size()).sum::<usize>() as f64 / 16.0);
    }

    // 4. Workload-aware partitioning
    println!("\n4. Workload-aware partitioning (balance_factor=0.5)...");
    let graph3 = TileGraph::new();
    let mut workload_partitioner = WorkloadPartitioner::new(graph3, 0.5);

    // Set varying workloads
    for i in 0..256 {
        let load = if i % 4 == 0 { 3.0 } else { 1.0 };
        workload_partitioner.set_tile_load(TileId(i), load);
    }

    match workload_partitioner.partition(4) {
        Ok(partitions) => {
            println!("   ✓ Workload-balanced partitioning successful!");
            for partition in &partitions {
                println!("   Partition {}: {} tiles, load={:.1}",
                    partition.id.0,
                    partition.size(),
                    partition.total_load
                );
            }
        }
        Err(e) => println!("   ✗ Error: {}", e),
    }

    // 5. TinyDancer routing with partition awareness
    println!("\n5. TinyDancer routing with partition awareness...");
    let router = TinyDancerRouter::new(256, 256);
    let task = TaskEmbedding::from_description("matmul", &[1024, 1024]);

    let tile_basic = router.predict_tile(&task);
    let tile_partition = router.route_with_partition(&task, &partitioner);

    println!("   Basic routing: Tile {}", tile_basic.0);
    println!("   Partition-aware routing: Tile {} (partition {})",
        tile_partition.0,
        partitioner.get_partition(tile_partition).0);

    // 6. Dynamic edge updates
    println!("\n6. Dynamic edge weight update...");
    if let Ok(()) = partitioner.update_edge(TileId(0), TileId(1), 5.0) {
        println!("   ✓ Updated edge weight between Tile 0 and Tile 1 to 5.0");
    }

    println!("\n=== Demo Complete ===");
}
