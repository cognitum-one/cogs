//! Test suite for RaceWay packet format
//!
//! Tests the 97-bit packet structure matching the Verilog implementation.

use cognitum_raceway::*;

#[test]
fn test_packet_creation() {
    let packet = RaceWayPacket::new()
        .source(TileId(0x11))
        .dest(TileId(0x42))
        .command(Command::Write)
        .tag(0x05)
        .write_data(0xDEADBEEF)
        .address(0x00001000)
        .build()
        .unwrap();

    assert_eq!(packet.source(), TileId(0x11));
    assert_eq!(packet.dest(), TileId(0x42));
    assert_eq!(packet.command(), Command::Write);
    assert_eq!(packet.tag(), 0x05);
}

#[test]
fn test_packet_width() {
    let packet = RaceWayPacket::new()
        .source(TileId(0))
        .dest(TileId(1))
        .command(Command::Write)
        .data(&[0x12, 0x34, 0x56, 0x78])
        .build()
        .unwrap();

    // Packet is 96 bits + 1 bit PUSH + 1 bit RESET_N = 98 bits
    // But the data portion is 96 bits (97 with control)
    let bits = packet.to_bits();
    assert_eq!(bits.len(), 97);
}

#[test]
fn test_packet_bit_format() {
    // Test exact bit layout matching Verilog:
    // Bit Position: 96:95:88:87:80:79:72:71:64:63:32:31:0
    //              PUSH:COMMAND:TAG:DEST:SOURCE:WRITE_DATA:ADDRESS

    let packet = RaceWayPacket::new()
        .source(TileId(0x11))
        .dest(TileId(0x42))
        .command(Command::Write) // 0x91
        .tag(0x05)
        .write_data(0xDEADBEEF)
        .address(0x00001000)
        .push(true)
        .build()
        .unwrap();

    let bits = packet.to_bits();

    // Bit 96: PUSH
    assert_eq!(bits[96], true);

    // Bits 95:88: COMMAND (0x91 = 1001_0001)
    assert_eq!(bits[95], true); // MSB
    assert_eq!(bits[88], true); // LSB

    // Bits 87:80: TAG (0x05)
    assert_eq!(bits[80], true);

    // Bits 79:72: DEST (0x42)
    assert_eq!(bits[78], true);
    assert_eq!(bits[73], true);

    // Bits 71:64: SOURCE (0x11)
    assert_eq!(bits[68], true);
    assert_eq!(bits[64], true);
}

#[test]
fn test_command_encoding() {
    assert_eq!(Command::Write.to_u8(), 0x91);
    assert_eq!(Command::Read.to_u8(), 0x89);
    assert_eq!(Command::AtomicAdd.to_u8(), 0x92);
    assert_eq!(Command::AtomicSwap.to_u8(), 0x93);
    assert_eq!(Command::Broadcast.to_u8(), 0xB1);
    assert_eq!(Command::BarrierSync.to_u8(), 0xA0);
    assert_eq!(Command::Multicast.to_u8(), 0xB8);
}

#[test]
fn test_broadcast_detection() {
    // Broadcast commands have bit 93 set (0x08 in bits 95:88)
    let broadcast = RaceWayPacket::new()
        .command(Command::Broadcast)
        .build()
        .unwrap();

    assert!(broadcast.is_broadcast());

    let normal = RaceWayPacket::new()
        .command(Command::Write)
        .build()
        .unwrap();

    assert!(!normal.is_broadcast());
}

#[test]
fn test_response_packet_swap() {
    // Responses swap source and destination
    let request = RaceWayPacket::new()
        .source(TileId(0x11))
        .dest(TileId(0x42))
        .command(Command::Write)
        .build()
        .unwrap();

    let response = request.to_response(0x11); // SUCCESS ack

    // Source and dest should be swapped
    assert_eq!(response.source(), TileId(0x42)); // Original dest
    assert_eq!(response.dest(), TileId(0x11)); // Original source
    assert_eq!(response.command().to_u8(), 0x11); // SUCCESS
}

#[test]
fn test_packet_serialization() {
    let packet = RaceWayPacket::new()
        .source(TileId(0x23))
        .dest(TileId(0x45))
        .command(Command::Read)
        .tag(0x07)
        .address(0x12345678)
        .build()
        .unwrap();

    let bits = packet.to_bits();
    let reconstructed = RaceWayPacket::from_bits(&bits).unwrap();

    assert_eq!(packet.source(), reconstructed.source());
    assert_eq!(packet.dest(), reconstructed.dest());
    assert_eq!(packet.command(), reconstructed.command());
    assert_eq!(packet.tag(), reconstructed.tag());
}

#[test]
fn test_builder_validation() {
    // Missing required fields should error
    let result = RaceWayPacket::new().source(TileId(0)).build();

    assert!(result.is_err());
}
