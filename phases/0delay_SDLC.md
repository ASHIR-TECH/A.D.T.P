**0DELAY**  
**Software Development Life Cycle (SDLC)**  
**AI-First File I/O Infrastructure — MCP Native**  
**Product:** 0delay  
   
 **Company:** Ashir Tech (Ashir Group)  
   
 **Version:** 1.0 SDLC  
   
 **Date:** 2026  
   
 **Author:** Ashir Tech Engineering  
![](data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAnEAAAACCAYAAAA3pIp+AAAABmJLR0QA/wD/AP+gvaeTAAAACXBIWXMAAA7EAAAOxAGVKw4bAAAANUlEQVR4nO3OMQ2AUBBAsUeCE4yeIiT9CRVMWGAjJK2CbjNzVGcAAPzF2qu7Wl9PAAB47XoA/vcF8exqpY4AAAAASUVORK5CYII=)  
**TABLE OF CONTENTS**  
1. Product Vision & Philosophy  
2. System Architecture  
3. Technology Stack  
4. Protocol Design  
5. Repository Structure  
6. MCP Server Specification  
7. API & Endpoints  
8. Data Flow & Security  
9. Development Phases  
10. Tools & DevOps  
11. Testing Strategy  
12. Deployment Architecture  
13. What Makes 0delay Indistinguishable  
![](data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAnEAAAACCAYAAAA3pIp+AAAABmJLR0QA/wD/AP+gvaeTAAAACXBIWXMAAA7EAAAOxAGVKw4bAAAANUlEQVR4nO3OQQmAABRAsSd4EKxgBjP+Asa0hxW8ibAl2DIzR3UFAMBf3Gu1VefXEwAAXtsfSqwDVbgKngwAAAAASUVORK5CYII=)  
**1. PRODUCT VISION & PHILOSOPHY**  
**One Line**  
*The encrypted, cloud-free, intelligent file channel between humans and AI agents.*  
**Problem**  
Every AI agent interaction involving files is broken:  
- Files must be uploaded through a browser  
- Outputs must be downloaded through a browser  
- All files pass through third-party cloud servers  
- No intelligent context is passed with the file  
- Agents have no awareness of file history or user filesystem  
**Solution**  
0delay is an **MCP-native file I/O daemon** that creates a direct, encrypted, intelligent tunnel between a user's local machine and any AI agent running on a Linux server — with no cloud intermediary, no browser, no download button.  
**Design Philosophy**  
1. Cloud-free by default    — files never touch a third-party server  
 2. Intelligence-first       — files carry context, not just bytes  
 3. Agent-agnostic           — works with any MCP-compatible agent  
 4. Offline-capable          — works on local networks and air-gapped environments  
 5. Audit-first              — every file transfer is logged, signed and verifiable  
 6. Protocol-first           — 0delay is a standard, not just a product  
   
![](data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAnEAAAACCAYAAAA3pIp+AAAABmJLR0QA/wD/AP+gvaeTAAAACXBIWXMAAA7EAAAOxAGVKw4bAAAANElEQVR4nO3OQQmAABRAsSdYxKY/jMFMIZ7ECt5E2BJsmZmt2gMA4C+Otbqr8+sJAACvXQ85QgYXd/O+eQAAAABJRU5ErkJggg==)  
**2. SYSTEM ARCHITECTURE**  
**High-Level Architecture**  
┌─────────────────────────────────────────────────────────────────┐  
 │                        USER MACHINE                             │  
 │                                                                 │  
 │  ┌─────────────────┐      ┌──────────────────────────────────┐ │  
 │  │  0delay Desktop │      │      0delay Daemon (Client)      │ │  
 │  │  App / CLI      │◄────►│  - File Watcher                  │ │  
 │  │                 │      │  - Encryption Layer (AES-256)    │ │  
 │  │  Drag & Drop UI │      │  - Transfer Queue                │ │  
 │  │  File Monitor   │      │  - Audit Logger                  │ │  
 │  └─────────────────┘      └──────────┬───────────────────────┘ │  
 └─────────────────────────────────────-│─────────────────────────┘  
                                        │  
                           Encrypted Tunnel  
                           (TLS 1.3 + AES-256-GCM)  
                           Direct P2P or Local Network  
                                        │  
 ┌──────────────────────────────────────│─────────────────────────┐  
 │                    LINUX AI SERVER   │                          │  
 │                                      ▼                          │  
 │  ┌───────────────────────────────────────────────────────────┐ │  
 │  │              0delay Server Daemon                         │ │  
 │  │                                                           │ │  
 │  │  ┌─────────────┐  ┌──────────────┐  ┌─────────────────┐ │ │  
 │  │  │ File Intake │  │  MCP Bridge  │  │  Response Queue │ │ │  
 │  │  │ & Validator │  │  Layer       │  │  & Dispatcher   │ │ │  
 │  │  └──────┬──────┘  └──────┬───────┘  └────────┬────────┘ │ │  
 │  │         │                │                    │           │ │  
 │  │         ▼                ▼                    │           │ │  
 │  │  ┌─────────────────────────────┐              │           │ │  
 │  │  │      Context Packager       │              │           │ │  
 │  │  │  - File metadata injection  │              │           │ │  
 │  │  │  - Schema detection         │              │           │ │  
 │  │  │  - History context          │              │           │ │  
 │  │  └──────────────┬──────────────┘              │           │ │  
 │  └─────────────────│─────────────────────────────│───────────┘ │  
 │                    │                             │              │  
 │                    ▼                             │              │  
 │  ┌─────────────────────────────────┐             │              │  
 │  │         AI AGENT                │─────────────┘              │  
 │  │  (Claude / GPT / Local LLM)     │                            │  
 │  │  Connected via MCP Protocol     │                            │  
 │  └─────────────────────────────────┘                            │  
 └─────────────────────────────────────────────────────────────────┘  
   
**Component Breakdown**  
| | |  
|-|-|  
| **Component** | **Role** |   
| **0delay Desktop App** | GUI for drag/drop sending, receive notifications, transfer history |   
| **0delay CLI** | Developer interface — pipe files directly from terminal |   
| **0delay Client Daemon** | Background process on user machine — handles encryption, queue, file watching |   
| **0delay Server Daemon** | Background process on Linux AI server — intake, validation, MCP bridge |   
| **Context Packager** | Injects metadata, schema, history into file payload before passing to agent |   
| **MCP Bridge** | Translates 0delay file events into MCP tool calls and resources |   
| **Audit Logger** | Cryptographically signs every transfer with timestamp and hash |   
   
![](data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAnEAAAACCAYAAAA3pIp+AAAABmJLR0QA/wD/AP+gvaeTAAAACXBIWXMAAA7EAAAOxAGVKw4bAAAANUlEQVR4nO3OMQ2AABAAsSNhZscYahheJwqQgQU2QtIq6DIze3UGAMBf3Gu1VcfXEwAAXrseoqcEQXyAWBgAAAAASUVORK5CYII=)  
**3. TECHNOLOGY STACK**  
**Core Language Choice**  
Primary Language:  Python 3.12+  
 Why Python:  
   - MCP SDK is Python-first (official Anthropic SDK)  
   - FastAPI for high-performance async endpoints  
   - Rich ecosystem for file processing (pandas, openpyxl, PyMuPDF)  
   - Cross-platform daemon support  
   - Fastest path to MCP marketplace listing  
   
 Secondary Language: Rust (for transfer protocol core)  
 Why Rust:  
   - Memory-safe encryption layer  
   - Zero-cost abstractions for file streaming  
   - Your existing C++ background transfers well  
   - Performance-critical transfer engine needs it  
   - Compiles to binaries for distribution (no Python dependency on client)  
   
 Desktop App: Tauri (Rust backend + HTML/CSS/JS frontend)  
 Why Tauri:  
   - Rust-based — consistent with core  
   - Lightweight (< 5MB vs Electron's 150MB)  
   - Native OS file system access  
   - Cross-platform: Windows, macOS, Linux  
   
**Full Stack**  
| | | |  
|-|-|-|  
| **Layer** | **Technology** | **Reason** |   
| **MCP Server** | Python 3.12 + mcp SDK | Official Anthropic SDK, fastest MCP compliance |   
| **Transfer Engine** | Rust + Tokio async runtime | Performance, memory safety, encryption |   
| **Desktop App** | Tauri + Vanilla JS | Lightweight, native, cross-platform |   
| **CLI** | Python Click / Rust Clap | Developer-first tooling |   
| **API Layer** | FastAPI + Uvicorn | Async, auto-docs, OpenAPI spec |   
| **Encryption** | AES-256-GCM + TLS 1.3 | Government-grade security |   
| **File Processing** | Pandas, PyMuPDF, openpyxl, Pillow | Intelligent file parsing |   
| **Message Queue** | Redis Streams | Reliable async file delivery |   
| **Audit Database** | SQLite (local) + PostgreSQL (server) | Lightweight client, robust server |   
| **Package Manager** | uv (Python) + Cargo (Rust) | Modern, fast dependency management |   
| **Config** | TOML | Human-readable, MCP-native format |   
   
![](data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAnEAAAACCAYAAAA3pIp+AAAABmJLR0QA/wD/AP+gvaeTAAAACXBIWXMAAA7EAAAOxAGVKw4bAAAANUlEQVR4nO3OMQ2AABAAsSNhwgJuUPYDMpnRgQU2QtIq6DIze3UGAMBf3Gu1VcfXEwAAXrseaHEEM+cJoFcAAAAASUVORK5CYII=)  
**4. PROTOCOL DESIGN**  
**The 0delay Transfer Protocol (0DTP)**  
0delay introduces its own transfer protocol layered on top of TLS. This is your moat.  
0DTP Packet Structure  
 ─────────────────────────────────────────────────────  
 │ HEADER (64 bytes)                                  │  
 │   magic:     0xDE1A4   (0delay signature)          │  
 │   version:   u8        (protocol version)          │  
 │   type:      u8        (SEND / ACK / PUSH / ERROR) │  
 │   session:   uuid_v4   (unique transfer session)   │  
 │   timestamp: u64       (unix nanoseconds)          │  
 │   checksum:  blake3    (file integrity hash)       │  
 ├───────────────────────────────────────────────────-│  
 │ CONTEXT BLOCK (variable)                           │  
 │   filename:  string                                │  
 │   mime_type: string                                │  
 │   file_size: u64                                   │  
 │   schema:    JSON       (detected file schema)     │  
 │   history:   JSON       (prior transfers context)  │  
 │   agent_hint: string   (suggested agent action)   │  
 ├────────────────────────────────────────────────────│  
 │ PAYLOAD (variable, AES-256-GCM encrypted)          │  
 │   file_bytes: bytes                                │  
 ├────────────────────────────────────────────────────│  
 │ SIGNATURE (64 bytes)                               │  
 │   ed25519 signature of header + context + hash    │  
 ─────────────────────────────────────────────────────  
   
**Transfer Types**  
SEND    →  User sends file to agent  
 PUSH    →  Agent pushes file to user machine  
 ACK     →  Receiver confirms receipt + integrity  
 ERROR   →  Transfer failed with reason code  
 WATCH   →  Register a folder for auto-trigger  
 UNWATCH →  Remove folder watch  
 STATUS  →  Query transfer queue status  
 HISTORY →  Retrieve transfer audit log  
   
**Connection Modes**  
Mode 1: DIRECT P2P  
   User machine ←──TLS tunnel──→ Linux AI Server  
   Best for: LAN, office networks, same datacenter  
   
 Mode 2: LOCAL NETWORK  
   User machine ←──LAN──→ Linux AI Server  
   Best for: air-gapped environments, government deployments  
   No internet required.  
   
 Mode 3: RELAY (future v2)  
   User ←──→ 0delay Relay Server ←──→ Agent  
   Best for: remote teams, cloud-hosted agents  
   Relay never sees decrypted content (end-to-end encrypted)  
   
![](data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAnEAAAACCAYAAAA3pIp+AAAABmJLR0QA/wD/AP+gvaeTAAAACXBIWXMAAA7EAAAOxAGVKw4bAAAANklEQVR4nO3OQQmAABRAsSfYxZo/kSGMYQLPJrCCNxG2BFtmZquOAAD4i3Ot7mr/egIAwGvXA4qrBdGuSdJuAAAAAElFTkSuQmCC)  
**5. REPOSITORY STRUCTURE**  
0delay/  
 │  
 ├── README.md                    # Project overview + quick start  
 ├── CONTRIBUTING.md              # How to contribute  
 ├── CHANGELOG.md                 # Version history  
 ├── LICENSE                      # MIT License (open core)  
 ├── Cargo.toml                   # Rust workspace manifest  
 ├── pyproject.toml               # Python project config (uv)  
 ├── Makefile                     # Build, test, lint commands  
 ├── .github/  
 │   ├── workflows/  
 │   │   ├── ci.yml               # CI pipeline  
 │   │   ├── release.yml          # Release automation  
 │   │   └── security.yml         # Security scanning  
 │   └── ISSUE_TEMPLATE/  
 │       ├── bug_report.md  
 │       └── feature_request.md  
 │  
 ├── docs/  
 │   ├── architecture.md          # This SDLC  
 │   ├── protocol-spec.md         # 0DTP specification  
 │   ├── mcp-integration.md       # How to use with Claude, etc.  
 │   ├── api-reference.md         # Full API docs  
 │   ├── security.md              # Security model  
 │   └── deployment/  
 │       ├── linux-server.md  
 │       ├── docker.md  
 │       └── government-airgap.md  
 │  
 ├── crates/                      # Rust workspace  
 │   ├── odelay-core/             # Core transfer protocol  
 │   │   ├── src/  
 │   │   │   ├── lib.rs  
 │   │   │   ├── protocol.rs      # 0DTP packet implementation  
 │   │   │   ├── encryption.rs    # AES-256-GCM + TLS  
 │   │   │   ├── transfer.rs      # File streaming engine  
 │   │   │   ├── integrity.rs     # BLAKE3 checksums + ed25519  
 │   │   │   └── errors.rs  
 │   │   └── Cargo.toml  
 │   │  
 │   ├── odelay-daemon-client/    # Client daemon (user machine)  
 │   │   ├── src/  
 │   │   │   ├── main.rs  
 │   │   │   ├── watcher.rs       # Filesystem watcher  
 │   │   │   ├── queue.rs         # Transfer queue  
 │   │   │   ├── receiver.rs      # Incoming file handler  
 │   │   │   └── audit.rs         # Local audit log  
 │   │   └── Cargo.toml  
 │   │  
 │   ├── odelay-daemon-server/    # Server daemon (Linux AI server)  
 │   │   ├── src/  
 │   │   │   ├── main.rs  
 │   │   │   ├── intake.rs        # File intake + validation  
 │   │   │   ├── context.rs       # Context packager  
 │   │   │   ├── dispatcher.rs    # Routes to agent via MCP  
 │   │   │   └── push.rs          # Agent output → user  
 │   │   └── Cargo.toml  
 │   │  
 │   └── odelay-cli/              # CLI binary  
 │       ├── src/  
 │       │   ├── main.rs  
 │       │   ├── commands/  
 │       │   │   ├── send.rs  
 │       │   │   ├── receive.rs  
 │       │   │   ├── watch.rs  
 │       │   │   ├── status.rs  
 │       │   │   └── history.rs  
 │       └── Cargo.toml  
 │  
 ├── packages/                    # Python workspace  
 │   ├── odelay-mcp/              # MCP server (Python)  
 │   │   ├── src/odelay_mcp/  
 │   │   │   ├── __init__.py  
 │   │   │   ├── server.py        # MCP server entry point  
 │   │   │   ├── tools/  
 │   │   │   │   ├── send_file.py  
 │   │   │   │   ├── receive_file.py  
 │   │   │   │   ├── watch_folder.py  
 │   │   │   │   ├── list_transfers.py  
 │   │   │   │   └── get_history.py  
 │   │   │   ├── resources/  
 │   │   │   │   ├── transfer_log.py  
 │   │   │   │   └── file_schema.py  
 │   │   │   ├── context/  
 │   │   │   │   ├── packager.py  # Metadata + schema injection  
 │   │   │   │   ├── detector.py  # File type detection  
 │   │   │   │   └── history.py   # Prior transfer context  
 │   │   │   └── security/  
 │   │   │       ├── auth.py  
 │   │   │       └── validator.py  
 │   │   ├── tests/  
 │   │   ├── pyproject.toml  
 │   │   └── README.md  
 │   │  
 │   └── odelay-api/              # REST API server  
 │       ├── src/odelay_api/  
 │       │   ├── __init__.py  
 │       │   ├── main.py          # FastAPI app  
 │       │   ├── routers/  
 │       │   │   ├── transfers.py  
 │       │   │   ├── sessions.py  
 │       │   │   ├── audit.py  
 │       │   │   └── health.py  
 │       │   ├── models/  
 │       │   │   ├── transfer.py  
 │       │   │   ├── session.py  
 │       │   │   └── audit.py  
 │       │   └── middleware/  
 │       │       ├── auth.py  
 │       │       └── ratelimit.py  
 │       ├── tests/  
 │       └── pyproject.toml  
 │  
 ├── desktop/                     # Tauri desktop app  
 │   ├── src-tauri/  
 │   │   ├── src/  
 │   │   │   └── main.rs  
 │   │   └── tauri.conf.json  
 │   ├── src/                     # Frontend  
 │   │   ├── index.html  
 │   │   ├── styles.css  
 │   │   └── app.js  
 │   └── package.json  
 │  
 ├── docker/  
 │   ├── Dockerfile.server        # Server daemon container  
 │   ├── Dockerfile.api           # API server container  
 │   └── docker-compose.yml       # Full stack local setup  
 │  
 ├── scripts/  
 │   ├── install.sh               # One-line Linux install  
 │   ├── install.ps1              # Windows install  
 │   └── dev-setup.sh             # Developer environment setup  
 │  
 └── examples/  
     ├── claude-agent/            # Example: 0delay + Claude agent  
     ├── openai-agent/            # Example: 0delay + OpenAI agent  
     ├── local-llm/               # Example: 0delay + Ollama  
     └── government-deployment/   # Example: air-gapped setup  
   
![](data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAnEAAAACCAYAAAA3pIp+AAAABmJLR0QA/wD/AP+gvaeTAAAACXBIWXMAAA7EAAAOxAGVKw4bAAAANklEQVR4nO3OMQ2AABAAsSNBACPq8MH2NpGACyywEZJWQZeZ2aszAAD+4l6rrTq+ngAA8Nr1AL/KBEe6dElaAAAAAElFTkSuQmCC)  
**6. MCP SERVER SPECIFICATION**  
**MCP Tools Exposed**  
MCP uses date-string versioned specifications negotiated during initialization. 0delay targets 2025-06-18 and forward.  
# packages/odelay-mcp/src/odelay_mcp/server.py  
   
 from mcp.server import Server  
 from mcp.server.models import InitializationOptions  
 import mcp.types as types  
   
 server = Server("0delay")  
   
 @server.list_tools()  
 async def list_tools() -> list[types.Tool]:  
     return [  
         types.Tool(  
             name="send_file",  
             description="""Send a file directly to the user's machine  
             without any web upload or browser interaction.  
             File is encrypted and delivered via 0delay tunnel.""",  
             inputSchema={  
                 "type": "object",  
                 "properties": {  
                     "file_path": {  
                         "type": "string",  
                         "description": "Absolute path to file on agent server"  
                     },  
                     "destination": {  
                         "type": "string",  
                         "description": "Target folder on user machine (optional)"  
                     },  
                     "message": {  
                         "type": "string",  
                         "description": "Message to accompany the file"  
                     },  
                     "priority": {  
                         "type": "string",  
                         "enum": ["normal", "urgent"],  
                         "default": "normal"  
                     }  
                 },  
                 "required": ["file_path"]  
             }  
         ),  
   
         types.Tool(  
             name="receive_file",  
             description="""Check for files sent by the user  
             and return their contents and metadata.  
             Returns the most recent unprocessed file.""",  
             inputSchema={  
                 "type": "object",  
                 "properties": {  
                     "session_id": {  
                         "type": "string",  
                         "description": "0delay session ID"  
                     },  
                     "file_type_filter": {  
                         "type": "string",  
                         "description": "Filter by file type e.g. xlsx, pdf, csv"  
                     }  
                 }  
             }  
         ),  
   
         types.Tool(  
             name="watch_folder",  
             description="""Register a folder on the user's machine  
             for automatic file watching. When a file is dropped  
             in the folder, it is automatically sent to this agent.""",  
             inputSchema={  
                 "type": "object",  
                 "properties": {  
                     "folder_path": {  
                         "type": "string",  
                         "description": "Absolute path to folder on user machine"  
                     },  
                     "trigger_action": {  
                         "type": "string",  
                         "description": "What to do when file arrives"  
                     },  
                     "file_pattern": {  
                         "type": "string",  
                         "description": "Glob pattern e.g. *.xlsx, *.pdf"  
                     }  
                 },  
                 "required": ["folder_path"]  
             }  
         ),  
   
         types.Tool(  
             name="list_transfers",  
             description="List all pending and completed file transfers in this session.",  
             inputSchema={  
                 "type": "object",  
                 "properties": {  
                     "status": {  
                         "type": "string",  
                         "enum": ["pending", "delivered", "failed", "all"],  
                         "default": "all"  
                     },  
                     "limit": {  
                         "type": "integer",  
                         "default": 10  
                     }  
                 }  
             }  
         ),  
   
         types.Tool(  
             name="get_file_context",  
             description="""Get intelligent context about a transferred file  
             including detected schema, prior transfer history,  
             and suggested agent actions.""",  
             inputSchema={  
                 "type": "object",  
                 "properties": {  
                     "transfer_id": {  
                         "type": "string",  
                         "description": "Transfer ID from list_transfers"  
                     }  
                 },  
                 "required": ["transfer_id"]  
             }  
         )  
     ]  
   
**MCP Resources Exposed**  
@server.list_resources()  
 async def list_resources() -> list[types.Resource]:  
     return [  
         types.Resource(  
             uri="odelay://transfers/log",  
             name="Transfer Audit Log",  
             description="Complete audit log of all file transfers in this session",  
            mimeType="application/json"  
         ),  
         types.Resource(  
             uri="odelay://session/status",  
             name="Session Status",  
             description="Current 0delay session info including connection quality",  
             mimeType="application/json"  
         ),  
         types.Resource(  
             uri="odelay://files/pending",  
             name="Pending Files",  
             description="Files queued for delivery to user machine",  
             mimeType="application/json"  
         )  
     ]  
   
![](data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAnEAAAACCAYAAAA3pIp+AAAABmJLR0QA/wD/AP+gvaeTAAAACXBIWXMAAA7EAAAOxAGVKw4bAAAANklEQVR4nO3OYQ1AABSAwc8mi5wvkwZyCKCAACr4Z7a7BLfMzFYdAQDwF+da3dX+9QQAgNeuB6feBdUJcyS2AAAAAElFTkSuQmCC)  
**7. API & ENDPOINTS**  
**REST API (FastAPI)**  
Base URL: http://localhost:40000/api/v1  
TRANSFERS  
 ─────────────────────────────────────────────────────────────────  
 POST   /transfers/send              Send file to agent  
 POST   /transfers/push              Agent pushes file to user  
 GET    /transfers                   List all transfers  
 GET    /transfers/{id}              Get transfer by ID  
 DELETE /transfers/{id}              Cancel pending transfer  
 GET    /transfers/{id}/status       Real-time transfer status  
   
 SESSIONS  
 ─────────────────────────────────────────────────────────────────  
 POST   /sessions                    Create new 0delay session  
 GET    /sessions/{id}               Get session details  
 DELETE /sessions/{id}               Terminate session  
 GET    /sessions/{id}/health        Check tunnel health  
   
 WATCHERS  
 ─────────────────────────────────────────────────────────────────  
 POST   /watchers                    Register folder watcher  
 GET    /watchers                    List active watchers  
 DELETE /watchers/{id}               Remove watcher  
   
 AUDIT  
 ─────────────────────────────────────────────────────────────────  
 GET    /audit/log                   Full audit log  
 GET    /audit/log/{transfer_id}     Audit entry for transfer  
 GET    /audit/export                Export audit log (CSV/JSON)  
   
 HEALTH  
 ─────────────────────────────────────────────────────────────────  
 GET    /health                      Service health check  
 GET    /health/tunnel               Tunnel connection status  
 GET    /health/queue                Queue depth and status  
   
**CLI Commands**  
# Send a file to your agent  
 odelay send ./report.xlsx --agent my-agent --message "Process this"  
   
 # Watch a folder — auto-sends any file dropped in it  
 odelay watch ~/Desktop/agent-inbox --trigger "process inventory"  
   
 # Check status of all transfers  
 odelay status  
   
 # View transfer history  
 odelay history --last 20  
   
 # Start the client daemon  
 odelay daemon start  
   
 # Configure a new agent connection  
 odelay connect --host 192.168.1.100 --port 40001 --name my-agent  
   
 # Generate a new keypair for a session  
 odelay keygen --output ~/.odelay/keys/  
   
 # Export audit log  
 odelay audit export --format csv --output ./audit-$(date).csv  
   
![](data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAnEAAAACCAYAAAA3pIp+AAAABmJLR0QA/wD/AP+gvaeTAAAACXBIWXMAAA7EAAAOxAGVKw4bAAAANUlEQVR4nO3OMQ2AUBBAsUfyRTCh9VRgEBGsWGAjJK2CbjNzVGcAAPzFtapV7V9PAAB47X4AEWgEMAY9+pUAAAAASUVORK5CYII=)  
**8. DATA FLOW & SECURITY**  
**Send Flow (User → Agent)**  
1. User drags file into 0delay Desktop App / drops in watched folder  
 2. Client daemon detects file  
 3. Context Packager reads file:  
    - Detects MIME type  
    - Extracts schema (Excel columns, PDF pages, CSV headers)  
    - Attaches transfer history for this file type  
    - Generates agent_hint based on file content  
 4. Packet is constructed (0DTP format)  
 5. Payload encrypted with AES-256-GCM  
 6. Packet signed with ed25519 private key  
 7. Packet transmitted over TLS 1.3 tunnel to server daemon  
 8. Server daemon verifies signature  
 9. Server daemon verifies BLAKE3 checksum  
 10. File written to secure intake directory  
 11. MCP Bridge calls receive_file tool  
 12. Agent receives file + full context via MCP  
 13. ACK sent back to client  
 14. Client shows "Delivered" notification  
   
**Push Flow (Agent → User)**  
1. Agent generates output file (report.pdf, analysis.xlsx, etc.)  
 2. Agent calls send_file MCP tool with file path  
 3. MCP Bridge triggers server daemon push  
 4. File is context-packaged, encrypted, signed  
 5. Transmitted over established tunnel to client daemon  
 6. Client daemon verifies integrity  
 7. File written directly to user's designated receive folder  
 8. Desktop app shows notification: "New file from agent: report.pdf"  
 9. User sees file in their folder — no download needed  
   
**Security Model**  
ENCRYPTION  
   ├── Transport:    TLS 1.3 (all communications)  
   ├── Payload:      AES-256-GCM (file contents)  
   ├── Integrity:    BLAKE3 (file hash)  
   └── Identity:     ed25519 keypairs (session authentication)  
   
 AUTHENTICATION  
   ├── Session-based keypairs (generated per connection)  
   ├── Optional: pre-shared key for air-gapped environments  
   └── No cloud auth — fully local key management  
   
 AUDIT  
   ├── Every transfer: timestamp, file hash, sender, receiver  
   ├── Cryptographically signed audit entries  
   ├── Tamper-evident log (hash chain)  
   └── Exportable for compliance (NDPR, GDPR, HIPAA)  
   
 ZERO TRUST PRINCIPLES  
   ├── Files never stored on relay servers  
   ├── Keys never leave the session  
   ├── Each session generates fresh keypairs  
   └── File contents encrypted before leaving source machine  
   
![](data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAnEAAAACCAYAAAA3pIp+AAAABmJLR0QA/wD/AP+gvaeTAAAACXBIWXMAAA7EAAAOxAGVKw4bAAAANUlEQVR4nO3OMQ2AABAAsSNhZscaUpheJwqQgQU2QtIq6DIze3UGAMBf3Gu1VcfXEwAAXrseopcEQ2uoYnwAAAAASUVORK5CYII=)  
**9. DEVELOPMENT PHASES**  
**Phase 0 — Foundation (Weeks 1–2)**  
□ Repository setup and structure  
 □ Rust workspace initialised (odelay-core)  
 □ Python workspace initialised (uv)  
 □ CI/CD pipeline (GitHub Actions)  
 □ Basic 0DTP packet structure in Rust  
 □ AES-256-GCM encryption working  
 □ BLAKE3 integrity checking working  
 □ Basic TLS tunnel between two processes  
 □ Unit tests for core protocol  
   
**Phase 1 — Core Transfer (Weeks 3–5)**  
□ Server daemon: file intake and validation  
 □ Client daemon: file watching (notify-rs crate)  
 □ Basic send flow: file reaches server daemon  
 □ Basic push flow: file reaches client machine  
 □ SQLite audit log on client side  
 □ PostgreSQL audit log on server side  
 □ Transfer queue with Redis Streams  
 □ CLI: odelay send, odelay status  
 □ Integration test: full send/push cycle  
   
**Phase 2 — Intelligence Layer (Weeks 6–8)**  
□ Context Packager: MIME detection  
 □ Context Packager: Excel schema extraction (openpyxl)  
 □ Context Packager: PDF metadata (PyMuPDF)  
 □ Context Packager: CSV header detection (pandas)  
 □ Transfer history context injection  
 □ Agent hint generation  
 □ Folder watcher with pattern matching  
 □ Auto-trigger on file drop  
   
**Phase 3 — MCP Server (Weeks 9–11)**  
□ MCP server skeleton (Python mcp SDK)  
 □ send_file tool implementation  
 □ receive_file tool implementation  
 □ watch_folder tool implementation  
 □ list_transfers tool implementation  
 □ get_file_context tool implementation  
 □ MCP resources: transfer log, session status  
 □ Test with Claude Desktop (MCP client)  
 □ Test with other MCP-compatible clients  
 □ Publish to MCP registry (open source)  
   
**Phase 4 — Desktop App (Weeks 12–14)**  
□ Tauri project setup  
 □ Drag and drop UI  
 □ Transfer progress indicators  
 □ Receive notifications (OS native)  
 □ Connection manager UI  
 □ Transfer history view  
 □ Settings: receive folder, watched folders, agent connections  
 □ macOS, Windows, Linux builds  
   
**Phase 5 — Hardening & Launch (Weeks 15–16)**  
□ Security audit (self + community)  
 □ Performance benchmarks  
 □ Load testing (concurrent transfers)  
 □ Documentation complete  
 □ Example integrations (Claude, Ollama, OpenAI)  
 □ GitHub release v1.0.0  
 □ Hacker News launch post  
 □ MCP marketplace listing  
 □ Reddit r/LocalLLaMA post  
   
![](data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAnEAAAACCAYAAAA3pIp+AAAABmJLR0QA/wD/AP+gvaeTAAAACXBIWXMAAA7EAAAOxAGVKw4bAAAANUlEQVR4nO3OMQ2AUBBAsUfyVTCg9UygEBVsWGAjJK2CbjNzVGcAAPzFtapV7V9PAAB47X4AEWIEM8iQs0EAAAAASUVORK5CYII=)  
**10. TOOLS & DEVOPS**  
**Development Tools**  
Code Quality  
 ├── ruff          Python linting + formatting  
 ├── mypy          Python type checking  
 ├── clippy        Rust linting  
 ├── rustfmt       Rust formatting  
 └── pre-commit    Git hooks for quality gates  
   
 Testing  
 ├── pytest        Python unit + integration tests  
 ├── cargo test    Rust unit tests  
 ├── locust        Load testing  
 └── hypothesis    Property-based testing (Python)  
   
 Security  
 ├── bandit        Python security scanner  
 ├── cargo-audit   Rust dependency vulnerability scanner  
 ├── trivy         Container image scanning  
 └── semgrep       Static analysis security rules  
   
 Documentation  
 ├── mkdocs        Documentation site  
 ├── pdoc          Python API docs  
 └── rustdoc       Rust API docs  
   
**CI/CD Pipeline (GitHub Actions)**  
# .github/workflows/ci.yml  
   
 on: [push, pull_request]  
   
 jobs:  
   test-python:  
     - Install uv  
     - Run pytest with coverage  
     - Run ruff + mypy  
     - Run bandit security scan  
   
   test-rust:  
     - Run cargo test  
     - Run cargo clippy  
     - Run cargo audit  
   
   build-binaries:  
     - Build Rust binaries (Linux, macOS, Windows)  
     - Build Tauri desktop app  
     - Upload artifacts  
   
   security-scan:  
     - Run trivy on Docker images  
     - Run semgrep rules  
     - SAST report  
   
**Infrastructure**  
Development:    Docker Compose (local full stack)  
 Staging:        Single Linux VPS (DigitalOcean / Hetzner)  
 Production:     TBD — customer self-hosted first  
 Monitoring:     Prometheus + Grafana (self-hosted)  
 Logs:           Loki (self-hosted)  
 Secrets:        .env files (local), Vault (future)  
   
![](data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAnEAAAACCAYAAAA3pIp+AAAABmJLR0QA/wD/AP+gvaeTAAAACXBIWXMAAA7EAAAOxAGVKw4bAAAANklEQVR4nO3OQQmAABRAsSfYxZo/jVEMYQLPJrCCNxG2BFtmZquOAAD4i3Ot7mr/egIAwGvXA4rLBc059ysnAAAAAElFTkSuQmCC)  
**11. TESTING STRATEGY**  
**Test Pyramid**  
                    ┌─────────────┐  
                     │  E2E Tests  │  — Full send/push cycle  
                     │    (few)    │    across real tunnel  
                     ├─────────────┤  
                     │ Integration │  — Component interactions  
                     │   Tests     │    MCP tools, API endpoints  
                     │  (some)     │  
                     ├─────────────┤  
                     │  Unit Tests │  — Protocol logic, encryption,  
                     │   (many)    │    context packager, validators  
                     └─────────────┘  
   
**Critical Test Cases**  
Security Tests  
   □ File arrives with correct BLAKE3 hash  
   □ Tampered file is rejected at server  
   □ Replay attack fails (session nonce)  
   □ Man-in-middle attack fails (TLS pinning)  
   □ Invalid keypair rejected  
   
 Intelligence Tests  
   □ Excel file schema correctly extracted  
   □ PDF metadata correctly injected  
   □ CSV headers correctly detected  
   □ Transfer history correctly attached  
   □ Agent hint generated for known file types  
   
 Protocol Tests  
   □ 0DTP packet serialises/deserialises correctly  
   □ Large files (>1GB) stream correctly  
   □ Network interruption handled gracefully  
   □ Queue survives daemon restart  
   □ ACK timeout triggers retry  
   
 MCP Tests  
   □ All tools callable from Claude Desktop  
   □ Resource URIs resolve correctly  
   □ Tool schemas validate against MCP spec  
   □ Error responses match MCP error format  
   
![](data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAnEAAAACCAYAAAA3pIp+AAAABmJLR0QA/wD/AP+gvaeTAAAACXBIWXMAAA7EAAAOxAGVKw4bAAAANklEQVR4nO3OQQmAABRAsSfYxZo/jVEMYQLPJrCCNxG2BFtmZquOAAD4i3Ot7mr/egIAwGvXA4rLBc059ysnAAAAAElFTkSuQmCC)  
**12. DEPLOYMENT ARCHITECTURE**  
**Self-Hosted (MVP — Default)**  
User installs 0delay Desktop App on their machine  
   └── Client daemon runs in background (startup)  
   
 Admin installs 0delay Server on Linux AI server  
   └── Server daemon runs as systemd service  
   └── MCP server runs alongside AI agent  
   
 Connection: Direct TLS over LAN or internet  
 No cloud. No relay. No third party.  
   
**Installation**  
# Linux server — one line install  
 curl -sSL https://get.0delay.dev | bash  
   
 # macOS/Windows/Linux client  
 # Download from: https://0delay.dev/download  
 # Or: brew install 0delay  
   
 # Configure connection  
 odelay connect --host your-ai-server.local --name "My Agent"  
   
 # Start sending  
 odelay send ./my-report.xlsx  
   
**systemd Service (Linux Server)**  
# /etc/systemd/system/odelay.service  
 [Unit]  
 Description=0delay Server Daemon  
 After=network.target  
   
 [Service]  
 Type=simple  
 User=odelay  
 ExecStart=/usr/local/bin/odelay-server daemon  
 Restart=always  
 RestartSec=5  
   
 [Install]  
 WantedBy=multi-user.target  
   
![](data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAnEAAAACCAYAAAA3pIp+AAAABmJLR0QA/wD/AP+gvaeTAAAACXBIWXMAAA7EAAAOxAGVKw4bAAAANklEQVR4nO3OQQmAABRAsSfYxZo/kSGMYQLPJrCCNxG2BFtmZquOAAD4i3Ot7mr/egIAwGvXA4qrBdGuSdJuAAAAAElFTkSuQmCC)  
**13. WHAT MAKES 0DELAY INDISTINGUISHABLE**  
These are the features that cannot be replicated by a weekend project:  
01. THE PROTOCOL  
     0DTP is a defined, versioned, open specification.  
     Anyone can implement it. Ashir Tech maintains it.  
     This is how HTTP works. This is how you own an ecosystem.  
   
 02. INTELLIGENT CONTEXT  
     No other file transfer tool injects schema, history  
     and agent hints into the file payload automatically.  
     The agent doesn't just receive a file.  
     It receives a file that already knows what it is.  
   
 03. ZERO CLOUD GUARANTEE  
     Cryptographically provable — no file touches a  
     third-party server. Audit log is signed and verifiable.  
     This is a compliance document, not a marketing claim.  
   
 04. AIR-GAP MODE  
     Works on local networks with zero internet.  
     No other MCP tool does this.  
     This unlocks government and military markets.  
   
 05. FOLDER INTELLIGENCE  
     Drop a file in a folder → agent processes it  
     automatically. No prompt. No upload. Just happens.  
     This is the closest thing to having an AI employee.  
   
 06. AUDIT TRAIL  
     Every transfer cryptographically signed and logged.  
     Tamper-evident chain of custody for every file.  
     NDPR, GDPR, HIPAA compliant out of the box.  
   
 07. AGENT-AGNOSTIC  
     Works with Claude, GPT, Gemini, Ollama, any MCP client.  
     You are not locked into one AI provider.  
     0delay is the infrastructure beneath all of them.  
   
![](data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAnEAAAACCAYAAAA3pIp+AAAABmJLR0QA/wD/AP+gvaeTAAAACXBIWXMAAA7EAAAOxAGVKw4bAAAANklEQVR4nO3OMQ2AABAAsSNBACPiUML0NpGACyywEZJWQZeZ2aszAAD+4l6rrTq+ngAA8Nr1AL/SBEZwuCSwAAAAAElFTkSuQmCC)  
**ASHIR TECH — ashir.world**  
   
 *Born from Ash. Rising for the World.*  
*0delay SDLC v1.0 — Confidential — Ashir Tech 2026*  
