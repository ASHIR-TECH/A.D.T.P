# ADTP Phase 8 — Task Lifecycle, Artifacts & Push/Wake

## Overview
Phase 8 adds the **Task Lifecycle** layer to ADTP, turning file transfers into structured work items. A Task describes work that one peer asks another to do with a file and expects a structured result back — possibly multiple result artifacts, possibly a long-running process, possibly a failure with a documented reason. This layer transforms ADTP from a file-drop mechanism into genuine agent-to-agent delegation.

Phase 8 also implements the **Subscribe / Notify push-wake mechanism** (reserved in Phase 7), allowing peers to receive real-time notifications without polling.

## Architecture — Task State Machine

```
                   ┌──────────┐
                   │ Received │
                   └────┬─────┘
                   ┌────┴─────┐
                   ▼          ▼
              ┌─────────┐ ┌──────────┐
              │ Accepted│ │ Rejected │ (terminal)
              └────┬────┘ └──────────┘
                   ▼
              ┌──────────┐
              │Processing│
              └────┬─────┘
                   ▼
             ┌──────────────┐
             │ArtifactReady │ ◄── (one or more artifacts)
             └──────┬───────┘
                ┌───┴───┐
                ▼       ▼
          ┌─────────┐ ┌──────┐
          │Completed│ │Failed│ (terminal)
          └─────────┘ └──────┘

Ordering: received → accepted → processing → artifact_ready → completed
Alternative: received → rejected (terminal)
Any active state → failed (terminal)  
Any state → cancelled (terminal, originator withdraws)
```

## Protocol — New Packet Types

| Type | Hex | Fields | Purpose |
|---|---|---|---|
| `TaskRequest` | 0x0E | `task_id`, `instructions`, `expected_artifacts` | Initiates a task |
| `TaskUpdate` | 0x0F | `task_id`, `new_state`, `reason` | Communicates state change |
| `ArtifactReady` | 0x10 | `task_id`, `artifact_name`, `mime_type`, `transfer_id` | Marks file as task output |

## Package Structure

### Rust — `adtp-peer-core` (3 new modules)

| Module | File | Description |
|---|---|---|
| `Task` | `task.rs` | Task struct with state machine, transition validation, `TaskStore` for in-memory management |
| `Artifact` | `artifact.rs` | Artifact struct linking a transferred file to its parent task |
| `SubscribeManager` | `subscribe.rs` | Manages one long-lived Subscribe stream per peer, sends `Notify` packets instantly |

### Python — MCP Tools (6 new)

| Tool | Input | Effect |
|---|---|---|
| `create_task` | `peer_id`, `file_path`, `instructions`, `expected_artifacts` | Creates a task (state: received) |
| `accept_task` | `task_id` | Transitions to accepted |
| `reject_task` | `task_id`, `reason` | Transitions to rejected (terminal) |
| `report_artifact` | `task_id`, `file_path`, `artifact_name` | Adds artifact, transitions to artifact_ready |
| `complete_task` | `task_id` | Transitions to completed (terminal) |
| `fail_task` | `task_id`, `reason` | Transitions to failed (terminal) |

### Python — REST API (`/api/v1/tasks`)

| Method | Path | Purpose |
|---|---|---|
| POST | `/api/v1/tasks` | Create a task |
| GET | `/api/v1/tasks` | List tasks (filterable by `peer_id`, `state`) |
| GET | `/api/v1/tasks/{task_id}` | Get single task with artifact list |
| POST | `/api/v1/tasks/{task_id}/accept` | Accept a pending task |
| POST | `/api/v1/tasks/{task_id}/reject` | Reject with reason |
| POST | `/api/v1/tasks/{task_id}/complete` | Mark completed |
| POST | `/api/v1/tasks/{task_id}/fail` | Mark failed with reason |
| POST | `/api/v1/tasks/{task_id}/artifact` | Report an artifact |

## Subscribe / Notify — Push Wake Mechanism

Each peer opens one long-lived Subscribe stream per connected peer immediately after Hello handshake. The `SubscribeManager` tracks these streams and sends `Notify` packets instantly when new Transfers or TaskUpdates are queued — no polling delay.

- **Sequence numbers**: Each Notify carries an incrementing `u64` sequence for ordering and gap detection
- **Reconnection**: On QUIC drop, the peer re-dials and re-opens the Subscribe stream
- **Catch-up**: Missed events recovered from the audit log using last known sequence number
- **Fallback polling**: Peers without persistent connections can still use `GET /transfers?status=pending`

## Core Types Added

### `task.rs` — Key Types
```rust
pub enum TaskState { Received, Accepted, Rejected, Processing, ArtifactReady, Completed, Failed, Cancelled }

impl TaskState {
    pub fn valid_next_states(&self) -> &[TaskState];
    pub fn is_terminal(&self) -> bool;
    pub fn validate_transition(&self, to: &TaskState) -> AdtpResult<()>;
}

pub struct Task {
    pub task_id: Uuid, pub transfer_id: Uuid, pub session_id: Uuid,
    pub sender_peer_id: [u8; 16], pub receiver_peer_id: [u8; 16],
    pub instructions: String, pub expected_artifacts: Vec<String>,
    pub state: TaskState, pub reason: Option<String>,
    pub artifacts: Vec<ArtifactRef>,
    pub created_at: DateTime<Utc>, pub updated_at: DateTime<Utc>,
}

impl Task {
    pub fn transition(&mut self, to: TaskState, reason: Option<String>) -> AdtpResult<()>;
    pub fn add_artifact(&mut self, name: String, transfer_id: Uuid, mime: String, size: u64) -> AdtpResult<()>;
}

pub struct TaskStore { /* in-memory HashMap<String, Task> */ }
impl TaskStore {
    pub fn add(&mut self, task: Task);
    pub fn get(&self, task_id: &str) -> Option<&Task>;
    pub fn list(&self) -> Vec<&Task>;
    pub fn list_filtered(&self, peer_id: Option<&str>, state: Option<&str>) -> Vec<&Task>;
}
```

### `subscribe.rs` — Key Types
```rust
pub struct SubscribeManager { /* Mutex<Vec<SubscribePeer>> */ }
impl SubscribeManager {
    pub fn register(&self, peer_id: [u8; 16], connection: Arc<AdtpConnection>);
    pub fn unregister(&self, peer_id: &[u8; 16]);
    pub fn send_notify(&self, target: &[u8; 16], transfer_id: Uuid, notify_type: NotifyType) -> AdtpResult<()>;
}

pub enum NotifyType { NewTransfer, TaskUpdate, ArtifactReady, Disconnect }
pub struct NotifyPayload { pub sequence: u64, pub transfer_id: Uuid, pub notify_type: NotifyType }
```

## Testing

### Unit Tests (10 inline in task.rs + artifact.rs)

| Test | What it verifies |
|---|---|
| `test_task_initial_state` | New task starts in `Received` |
| `test_valid_transition_received_to_accepted` | Legal transition succeeds |
| `test_invalid_transition_received_to_completed` | Illegal transition returns error |
| `test_terminal_state_rejected` | Terminal state rejects all further transitions |
| `test_full_task_lifecycle` | received → accepted → processing → artifact_ready → completed |
| `test_duplicate_artifact_rejected` | Duplicate artifact names are rejected |
| `test_task_store_list_filtered` | Filtering by peer_id works |
| `test_state_display` | Display impl produces correct strings |
| `test_artifact_creation` | Artifact fields are set correctly |
| `test_artifact_response_conversion` | Artifact → ArtifactResponse conversion |

### Integration Tests (3 new in tests/integration.rs)

| Test | What it proves |
|---|---|
| `test_full_task_lifecycle_integration` | Full state sequence across TaskStore |
| `test_task_rejection_flow` | reject_task closes as terminal |
| `test_artifact_links_to_transfer` | Artifact correctly links task_id and transfer_id |

### Test Results
```
cargo test --workspace: 65 passed, 0 failed
  - adtp-core:          14/14
  - adtp-client:         2/2
  - adtp-server:         2/2
  - adtp-cli:           16/16
  - adtp-peer-core:     19/19  (10 unit + 9 integration)
  - adtp-desktop:       11/11
  - adtp-proto:          0/0
  - adtp-peer:           0/0
  - doc-tests:           1/1
```

## Error Variants Added

All new errors are in `adtp-core/src/errors.rs`:

| Variant | Meaning |
|---|---|
| `TaskNotFound { task_id }` | Referenced task doesn't exist |
| `IllegalTaskTransition { from, to, reason }` | State machine violation |
| `TaskTerminal { state }` | Attempted transition on terminal state |
| `ArtifactNotFound { task_id, artifact_name }` | Duplicate or missing artifact |

## How to Test

```bash
# Run all workspace tests
cargo test --workspace

# Run only peer-core tests (task + artifact + audit + trust + identity)
cargo test -p adtp-peer-core

# Run with output for a specific test
cargo test -p adtp-peer-core test_full_task_lifecycle -- --nocapture

# Check compilation
cargo check --workspace

# Python API tests (if pytest configured)
cd packages/adtp-api && python -m pytest tests/
```

## Phase 9 Hook Points

| Hook | What Phase 8 Provides | Phase 9 Uses It For |
|---|---|---|
| `task_id` and artifact chain | Full task and artifact audit trail per peer pair | Multi-hop provenance — same `task_id` preserved across relay hops |
| Notify / Subscribe | Real-time wake mechanism | Basis for cross-peer distributed trace propagation events |
| TaskRecord schema | REST-exposed task state | Mapped directly onto A2A's Task and Artifact JSON shape |
