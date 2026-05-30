# MCP Resources Reference

MCP resources are planned for Phase 3.

## Planned Resources

| Resource | Purpose |
| --- | --- |
| `adtp://transfers/recent` | Recent transfer records. |
| `adtp://audit/status` | Audit chain verification state. |
| `adtp://files/{transfer_id}/metadata` | File metadata and context. |
| `adtp://sessions/current` | Active session details. |

## Resource Flow

```mermaid
flowchart LR
    Agent[AI Agent] --> MCP[MCP Server]
    MCP --> Audit[(Audit DB)]
    MCP --> Context[Context Store]
    MCP --> Files[Intake/Output Files]
```
