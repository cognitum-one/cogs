# Cognitum REST API

REST API server for the Cognitum neuromorphic chip simulator.

## Features

- **REST API** for simulation management
- **WebSocket streaming** for real-time events
- **API key authentication** via Bearer tokens
- **Rate limiting** per user
- **CORS support** for cross-origin requests
- **Input validation** with comprehensive error handling
- **Metrics endpoint** for Prometheus integration
- **Health check** endpoint

## Quick Start

```bash
# Build
cargo build --release

# Run tests
cargo test

# Start server (requires service implementations)
cargo run --release
```

## API Endpoints

### Simulations

- `POST /api/v1/simulations` - Create simulation
- `GET /api/v1/simulations` - List simulations
- `GET /api/v1/simulations/{id}` - Get simulation details
- `POST /api/v1/simulations/{id}/run` - Start simulation
- `GET /api/v1/simulations/{id}/status` - Get status
- `GET /api/v1/simulations/{id}/results` - Get results
- `DELETE /api/v1/simulations/{id}` - Delete simulation
- `WS /api/v1/simulations/{id}/stream` - WebSocket stream

### Programs

- `POST /api/v1/programs` - Upload program binary
- `GET /api/v1/programs/{id}` - Get program metadata

### System

- `GET /health` - Health check (no auth)
- `GET /metrics` - Prometheus metrics (no auth)

## Authentication

All API endpoints (except `/health` and `/metrics`) require authentication via Bearer token:

```bash
curl -H "Authorization: Bearer sk_your_api_key" \
  http://localhost:8080/api/v1/simulations
```

## Configuration

Environment variables:

- `API_HOST` - Server host (default: 127.0.0.1)
- `API_PORT` - Server port (default: 8080)
- `JWT_SECRET` - JWT signing secret (required in production)
- `RATE_LIMIT_RPM` - Requests per minute (default: 100)
- `ENABLE_CORS` - Enable CORS (default: true)
- `CORS_ORIGINS` - Allowed origins (default: *)

## Security

- Bearer token authentication
- Per-user rate limiting
- Input validation with range checks
- SQL injection prevention (parameterized queries)
- CORS configuration
- Proper error handling without sensitive data leakage

## Architecture

### Service Traits

The API uses trait-based dependency injection for testability:

- `SimulatorService` - Simulation management
- `StorageService` - Program storage
- `AuthService` - Authentication
- `RateLimiter` - Rate limiting

### Middleware Stack

1. Logging - Request/response logging
2. Authentication - Bearer token validation
3. Rate Limiting - Per-user limits
4. CORS - Cross-origin support

## Testing

Following TDD London School (Mockist) approach:

```bash
# Run all tests
cargo test

# Acceptance tests
cargo test --test '*_test'

# Unit tests
cargo test --lib
```

Test coverage includes:
- Simulation lifecycle (create, run, delete)
- Authentication (valid/invalid keys, public endpoints)
- Rate limiting
- Input validation
- Error handling

## Dependencies

- `actix-web 4.9` - Web framework
- `tokio` - Async runtime
- `serde` - Serialization
- `validator` - Input validation
- `mockall` - Mocking (dev)

## Implementation Status

### Completed

- ✅ Core API structure
- ✅ All endpoint handlers
- ✅ Authentication middleware
- ✅ Rate limiting middleware
- ✅ Input validation
- ✅ Error handling
- ✅ Service trait definitions
- ✅ Acceptance tests
- ✅ Unit tests

### Pending

- ⏳ Service implementations (SimulatorServiceImpl, etc.)
- ⏳ Database integration
- ⏳ WebSocket event broadcasting
- ⏳ Prometheus metrics collection
- ⏳ OpenAPI/Swagger documentation

## Example Usage

### Create Simulation

```bash
curl -X POST http://localhost:8080/api/v1/simulations \
  -H "Authorization: Bearer sk_test_xxx" \
  -H "Content-Type: application/json" \
  -d '{
    "config": {
      "tiles": 16,
      "memory_per_tile": 156000,
      "enable_crypto": true
    },
    "program_id": "prog_abc123"
  }'
```

Response:
```json
{
  "id": "sim_xyz789",
  "status": "created",
  "config": {
    "tiles": 16,
    "memory_per_tile": 156000,
    "enable_crypto": true
  },
  "created_at": "2025-01-15T10:30:00Z"
}
```

### Run Simulation

```bash
curl -X POST http://localhost:8080/api/v1/simulations/sim_xyz789/run \
  -H "Authorization: Bearer sk_test_xxx" \
  -H "Content-Type: application/json" \
  -d '{"cycles": 100000}'
```

Response:
```json
{
  "job_id": "job_abc123",
  "status": "queued",
  "estimated_completion": "2025-01-15T10:30:05Z"
}
```

## License

MIT OR Apache-2.0

## Documentation

See `/home/user/cognitum/docs/api/API_IMPLEMENTATION_SUMMARY.md` for detailed implementation notes.
