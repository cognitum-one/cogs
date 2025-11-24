use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use cognitum_raceway::*;

fn bench_packet_creation(c: &mut Criterion) {
    c.bench_function("packet_creation", |b| {
        b.iter(|| {
            RaceWayPacket::new()
                .source(black_box(TileId(0x11)))
                .dest(black_box(TileId(0x42)))
                .command(Command::Write)
                .tag(0x05)
                .write_data(0xDEADBEEF)
                .address(0x1000)
                .push(true)
                .build()
                .unwrap()
        })
    });
}

fn bench_packet_serialization(c: &mut Criterion) {
    let packet = RaceWayPacket::new()
        .source(TileId(0x11))
        .dest(TileId(0x42))
        .command(Command::Write)
        .tag(0x05)
        .push(true)
        .build()
        .unwrap();

    c.bench_function("packet_to_bits", |b| {
        b.iter(|| black_box(&packet).to_bits())
    });
}

fn bench_packet_deserialization(c: &mut Criterion) {
    let packet = RaceWayPacket::new()
        .source(TileId(0x11))
        .dest(TileId(0x42))
        .command(Command::Write)
        .tag(0x05)
        .push(true)
        .build()
        .unwrap();

    let bits = packet.to_bits();

    c.bench_function("packet_from_bits", |b| {
        b.iter(|| RaceWayPacket::from_bits(black_box(&bits)).unwrap())
    });
}

criterion_group!(
    benches,
    bench_packet_creation,
    bench_packet_serialization,
    bench_packet_deserialization
);
criterion_main!(benches);
