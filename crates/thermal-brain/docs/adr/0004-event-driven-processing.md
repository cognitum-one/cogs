# ADR-0004: Event-Driven Processing

## Status
Accepted

## Context
Traditional time-stepped neural network execution processes all neurons every timestep, regardless of activity. For sparse spiking networks, this wastes significant power on inactive neurons.

## Decision
Implement **Event-Driven Processing** where computation occurs only when spikes are received.

### Architecture

```
┌─────────────────────────────────────────────┐
│            Event Queue (Min-Heap)           │
│  Ordered by timestamp, priority             │
└─────────────────┬───────────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│           Event Processor                    │
│  - Processes events up to current time      │
│  - Generates output spikes                  │
│  - Updates neuron states                    │
└─────────────────┬───────────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│           Power State Machine               │
│  Active → Idle → Sleep → DeepSleep          │
└─────────────────────────────────────────────┘
```

### Key Design Choices

1. **Event Queue**
   - Binary heap ordered by timestamp
   - Maximum 256 events
   - Priority field for important events

2. **Power States**
   - Active: Processing events
   - Idle: No events, fast wake-up
   - Sleep: 1ms timeout, medium wake-up
   - DeepSleep: 10ms timeout, slow wake-up

3. **Batch Processing**
   - Process up to 16 events per call
   - Prevents starvation of other tasks

4. **Event Coalescing** (optional)
   - Merge nearby events to same neuron
   - Reduces queue pressure

## Consequences

### Positive
- **10x Power Reduction**: For typical sparse activity
- **Natural Sparsity**: Only active neurons consume power
- **Variable Timestep**: No wasted cycles on quiet periods
- **Scalable**: Complexity proportional to activity

### Negative
- **Queue Management**: Overhead for event insertion
- **Worst Case**: High activity can exceed time-stepped
- **Timing Complexity**: Events must be properly ordered

## References
- Neuromorphic Event-Driven Systems (Intel Loihi)
- Asynchronous Neural Network Architectures
