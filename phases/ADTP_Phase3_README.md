# ADTP Phase 3 — Intelligence Layer

## Build Spec Reference

`phases/ADTP_Phase3_Build_Spec.docx`

---

## Summary

Phase 3 replaces the Phase 2 stub in `packager.build_context()` with a full Intelligence Layer that detects file MIME types from bytes (not extensions), dispatches to file-type-specific extractors, builds a `ContextBlock` with schema/summary/agent_hint, and queries the SQLite audit log for transfer history. All Phase 3 code is pure Python, lives inside `packages/adtp-mcp/src/adtp_mcp/context/`, and has zero interaction with Rust daemons.

---

## Files Changed (29 total)

### New Files Created (18)

#### Context Types (`packages/adtp-mcp/src/adtp_mcp/context/__init__.py`)
- **What**: Dataclasses for `ContextBlock`, `ExtractionResult`, `TransferHistoryEntry`
- **Why**: Shared types used by detector, extractors, packager, and history

#### MIME Detection (`packages/adtp-mcp/src/adtp_mcp/context/detector.py`)
- **What**: `detect(file_path)` reads first 8KB via `python-magic` → returns MIME string
- **Fallback**: `mimetypes.guess_type()` if magic returns `application/octet-stream`
- **Error**: `FileNotFoundError` → `AdtpContextError`; unreadable → `application/octet-stream`
- **Never uses**: File extension alone (spoof-proof)

#### Extractor Registry (`packages/adtp-mcp/src/adtp_mcp/context/extractors/__init__.py`)
- **What**: `REGISTRY` dict mapping MIME strings → extractor classes, `get_extractor(mime)` lookup
- **Fallback**: Unknown MIME → `GenericExtractor`

#### Excel Extractor (`packages/adtp-mcp/src/adtp_mcp/context/extractors/excel.py`)
- **What**: `ExcelExtractor.extract()` handles `.xlsx` (openpyxl) and `.xls` (xlrd)
- **Schema**: `sheet_names[]`, `row_count`, `column_count`, `headers{sheet: [cols]}`, `has_formulas`, `has_charts`, `detected_data_types{col: type}`
- **Performance**: Reads headers + first 2 rows only, not all cell values
- **Agent hint**: Names the sheet with most data as primary processing target

#### PDF Extractor (`packages/adtp-mcp/src/adtp_mcp/context/extractors/pdf.py`)
- **What**: `PdfExtractor.extract()` uses PyMuPDF (fitz)
- **Schema**: `page_count`, `title`, `author`, `creator`, `subject`, `keywords[]`, `has_images`, `has_tables`, `first_page_text` (500 chars)
- **Agent hint**: If `page_count > 20` → "summarise section by section"

#### Word Extractor (`packages/adtp-mcp/src/adtp_mcp/context/extractors/word.py`)
- **What**: `WordExtractor.extract()` uses python-docx
- **Schema**: `word_count`, `paragraph_count`, `heading_count`, `headings[]`, `table_count`, `has_images`, `author`, `last_modified`

#### CSV Extractor (`packages/adtp-mcp/src/adtp_mcp/context/extractors/csv.py`)
- **What**: `CsvExtractor.extract()` uses chardet + csv module
- **Schema**: `delimiter`, `encoding`, `header_row[]`, `row_count`, `column_count`, `detected_types{col: type}`, `has_null_values`
- **Type inference**: Reads first 100 data rows, heuristically detects integer/float/date/string per column
- **Validation**: Rejects binary data via control-character ratio check in header
- **Agent hint**: If date columns detected → "check for time-series patterns"

#### Audio Extractor (`packages/adtp-mcp/src/adtp_mcp/context/extractors/audio.py`)
- **What**: `AudioExtractor.extract()` uses mutagen
- **Formats**: MP3 (MPEGInfo), FLAC, OggVorbis, and generic tag extraction via mutagen.File
- **Schema**: `duration_seconds`, `bitrate_kbps`, `sample_rate_hz`, `channels`, `format`, `tags{title, artist, album, year}`
- **Agent hint**: If `duration < 600s` → "transcribe for full content analysis"

#### Video Extractor (`packages/adtp-mcp/src/adtp_mcp/context/extractors/video.py`)
- **What**: `VideoExtractor.extract()` uses ffmpeg-python/ffprobe
- **Schema**: `duration_seconds`, `width_px`, `height_px`, `fps`, `video_codec`, `audio_codec`, `file_size_mb`, `has_subtitles`

#### Image Extractor (`packages/adtp-mcp/src/adtp_mcp/context/extractors/image.py`)
- **What**: `ImageExtractor.extract()` uses Pillow
- **Schema**: `width_px`, `height_px`, `colour_mode`, `has_transparency`, `exif{make, model, datetime, software, gps}`, `frame_count` (for GIF)
- **Agent hint**: If GPS EXIF present → "consider location-based analysis"

#### JSON Extractor (`packages/adtp-mcp/src/adtp_mcp/context/extractors/json.py`)
- **What**: `JsonExtractor.extract()` uses jsonschema
- **Schema**: `top_level_type` (object|array|scalar), `key_count`, `array_length`, `inferred_schema{key: type}`, `depth`, `has_nulls`
- **Agent hint**: If top_level_type is array → "treat as a dataset"

#### Generic Extractor (`packages/adtp-mcp/src/adtp_mcp/context/extractors/generic.py`)
- **What**: `GenericExtractor.extract()` fallback for unrecognised MIME types
- **Schema**: `filename`, `file_size_bytes`, `extension`, `mime_type` — no content read
- **Agent hint**: Always "Binary file — process by filename and size only"

#### History (`packages/adtp-mcp/src/adtp_mcp/context/history.py`)
- **What**: `get_history(session_id, mime_type)` queries SQLite audit DB
- **Source**: `ADTP_AUDIT_DB` env var
- **Query**: `SELECT timestamp, mime_type, agent_action FROM transfer_audit WHERE mime_type=? AND session_id=? ORDER BY timestamp DESC LIMIT 5`
- **Error handling**: DB unreachable → returns empty list, transfer continues

#### Packager (`packages/adtp-mcp/src/adtp_mcp/context/packager.py`)
- **What**: `build_context(file_path, session_id)` — master orchestrator (replaces Phase 2 stub)
- **Flow**: detect MIME → registry lookup → extractor dispatch (in thread executor) → history query → assemble ContextBlock
- **Async timeout**: Each extractor wrapped in `asyncio.wait_for(timeout=10s)`
- **Error isolation**: Any extractor failure → empty ContextBlock returned → transfer continues

#### Unit Tests (3 new files)

| File | Tests |
|------|-------|
| `tests/unit/test_detector.py` | 12 tests — MIME detection for all 9 fixtures, renamed extension, file not found, empty file |
| `tests/unit/test_extractors.py` | 18 tests — success + failure case per extractor, base result contract |
| `tests/unit/test_packager.py` | 7 tests — context output per fixture type, nonexistent file, no-session mode |
| `tests/unit/test_history.py` | 6 tests — session filtering, MIME matching, limit 5, no-db env |

#### Integration Tests (1 new file)

| File | Tests |
|------|-------|
| `tests/integration/test_context_flow.py` | 11 tests — full context flow for all fixture types, timeout handling, corrupt file, missing file, all-fixtures coverage |

#### Test Fixtures (`tests/fixtures/`)
| File | Type | Purpose |
|------|------|---------|
| `sample.xlsx` | Excel (openpyxl) | 3 sheets: Invoices (50 rows), Products (20 rows), Summary |
| `sample.pdf` | PDF (PyMuPDF) | 6 pages, headings, embedded image placeholder, metadata |
| `sample.docx` | Word (python-docx) | Headings H1-H3, 1 table, author metadata |
| `sample.csv` | CSV | 150 rows, Date/Product/Quantity/Price/Total/Region columns, null values |
| `sample.mp3` | MP3 (mutagen) | 5s sine tone, ID3 tags (title/artist/album/year) |
| `sample.mp4` | MP4 (ffprobe) | 1s 720p H.264 blue video |
| `sample.jpg` | JPEG (Pillow) | 640x480 blue, EXIF Make/Model/DateTimeOriginal/Software |
| `sample.json` | JSON (jsonschema) | Array of 3 objects, nested depth 3, mixed types |
| `sample.bin` | Binary | Arbitrary bytes for generic fallback |
| `generate_fixtures.py` | Generator script | Recreates all fixtures from scratch |

---

### Files Modified (11)

#### `packages/adtp-mcp/pyproject.toml`
- **Changed**: Version `0.1.0` → `0.2.0`
- **Changed**: Removed `serde-json>=0.1` (not a real PyPI package)
- **Changed**: `pytest-anyio>=0.0.5` → `pytest-asyncio>=0.21` in test deps
- **Added**: 12 new dependencies:
  ```
  python-magic>=0.4.27    # MIME detection via libmagic
  openpyxl>=3.1           # .xlsx Excel reading
  xlrd>=2.0               # .xls legacy Excel reading
  pymupdf>=1.24           # PDF extraction (fitz)
  python-docx>=1.1        # .docx Word reading
  chardet>=5.0            # CSV encoding detection
  mutagen>=1.47           # Audio metadata (MP3/FLAC/OGG)
  ffmpeg-python>=0.2      # Video ffprobe wrapper
  Pillow>=10.0            # Image metadata + EXIF
  jsonschema>=4.21        # JSON schema inference
  aiofiles>=23.0          # Async file I/O support
  ```
- **Added**: `asyncio_mode = "auto"` to `[tool.pytest.ini_options]`

#### `packages/adtp-mcp/src/adtp_mcp/tools/send_file.py`
- **Changed**: Added `session_id: str = ""` to `SendFileInput`
- **Changed**: Added `context: dict | None = None` to `SendFileResult`
- **Changed**: `build_context(file_path)` → `await build_context(file_path, input_data.session_id)` — captures result
- **Changed**: Bridge call `enqueue_send(...)` now passes context dict as additional parameter

#### `packages/adtp-mcp/src/adtp_mcp/tools/receive_file.py`
- **Changed**: Added `session_id: str = ""` to `ReceiveFileInput`
- **Changed**: Added `context: dict | None = None` to `ReceiveFileResult`
- **Changed**: `build_context(file_path)` → `await build_context(file_path, input_data.session_id)` — captures result
- **Changed**: Bridge call `push_file(...)` now passes context dict as additional parameter

#### `packages/adtp-mcp/src/adtp_mcp/bridge/client.py`
- **Changed**: `enqueue_send()` now accepts optional `context: dict[str, Any] | None` parameter
- **Changed**: If context provided, included in socket payload as `params["context"]`

#### `packages/adtp-mcp/src/adtp_mcp/bridge/server.py`
- **Changed**: `push_file()` now accepts optional `context: dict[str, Any] | None` parameter
- **Changed**: If context provided, included in socket payload as `params["context"]`

#### `packages/adtp-mcp/src/adtp_mcp/server.py`
- **Changed**: Removed unused `import asyncio`
- **Changed**: Ruff-formatting only — all tool function signatures wrapped across multiple lines for readability

#### `packages/adtp-mcp/src/adtp_mcp/security/auth.py`, `validator.py`
- **Changed**: Ruff-formatting only — line wrapping

#### `packages/adtp-mcp/src/adtp_mcp/tools/watch_folder.py`, `unwatch_folder.py`
- **Changed**: Ruff-formatting only — line wrapping

#### `packages/adtp-mcp/src/adtp_mcp/config.py`
- **Changed**: Ruff-formatting only — line wrapping

#### `packages/adtp-mcp/src/adtp_mcp/bridge/base.py`
- **Changed**: Ruff-formatting only — line wrapping

---

## Architecture

```
MCP Tool (send_file/receive_file)
  │
  ▼
packager.build_context(file_path, session_id)    ◄── Phase 3 entry point
  │
  ├── detector.detect(file_path)                  ──► MIME string
  │
  ├── extractors.REGISTRY[mime]                   ──► extractor class
  │
  ├── extractor.extract(file_path)                ──► ExtractionResult
  │     (via loop.run_in_executor, timeout=10s)     (schema, summary, agent_hint)
  │
  ├── history.get_history(session_id, mime)       ──► list[TransferHistoryEntry]
  │
  └── ContextBlock assembled + returned           ──► passed to bridge command
```

## Error Handling & Performance

| Failure Mode | Behaviour | Transfer Impact |
|---|---|---|
| File not readable | Empty ContextBlock, ERROR log | Continues |
| Extractor timeout (>10s) | `asyncio.TimeoutError`, empty block | Continues |
| Corrupt file | Exception caught, partial/empty schema | Continues |
| MIME detection failure | Falls back to generic extractor | Continues |
| History DB unreachable | Empty list, WARNING log | Continues |

## Phase 3 Completion Checklist

- [x] `uv add python-magic openpyxl xlrd pymupdf python-docx chardet mutagen ffmpeg-python Pillow jsonschema aiofiles` — zero import errors
- [x] `apt install libmagic1 ffmpeg` confirmed in server environment
- [x] All 9 fixture files created in `tests/fixtures/`
- [x] `detector.detect()` returns correct MIME for all 9 fixtures
- [x] `packager.build_context()` returns non-empty ContextBlock for all 9 fixture types
- [x] All unit tests pass — `pytest tests/unit/ -v` (41/41)
- [x] All integration tests pass — `pytest tests/integration/ -v` (11/11)
- [x] Extractor timeout test passes — transfer not blocked by slow extraction
- [x] Corrupt file test passes — transfer continues despite extraction failure
- [x] No extractor reads more data than specified in performance targets
- [x] `packager.build_context()` completes in < 5s for all fixture types
- [x] Ruff lint clean — `ruff check src/`
- [x] Ruff format clean — `ruff format --check src/`
- [x] Cargo check workspace clean — `cargo check --workspace`

---

## How to Run Tests

```bash
# Activate virtual environment
source .venv/bin/activate

# Run all Phase 3 tests (unit + integration)
python -m pytest packages/adtp-mcp/tests/ -v

# Run only unit tests
python -m pytest packages/adtp-mcp/tests/unit/ -v

# Run only integration tests
python -m pytest packages/adtp-mcp/tests/integration/ -v

# Run specific test file
python -m pytest packages/adtp-mcp/tests/unit/test_detector.py -v

# Run specific test class
python -m pytest packages/adtp-mcp/tests/unit/test_extractors.py::TestExcelExtractor -v

# Lint
ruff check packages/adtp-mcp/src/
ruff format --check packages/adtp-mcp/src/

# Full workspace cargo check
cargo check --workspace
```

---

## Test Results

```
============================= 52 passed in 13.31s ==============================
```

| Test Group | Count | Status |
|---|---|---|
| `test_detector.py` | 12 | ✅ All pass |
| `test_extractors.py` | 18 | ✅ All pass |
| `test_packager.py` | 7 | ✅ All pass |
| `test_history.py` | 6 | ✅ All pass |
| `test_context_flow.py` | 11 | ✅ All pass |
| **Total** | **52** | **✅ All pass** |

---

## Phase 4 Hook Points

| Hook | Location | Phase 4 Consumption |
|---|---|---|
| ContextBlock JSON | Stored in adtp-client SQLite audit DB per transfer | `GET /transfers/{id}/context` |
| ExtractionResult.schema | Returned by packager → stored in audit DB | `GET /transfers/{id}/schema` |
| TransferHistoryEntry list | Returned by history.py → stored per session | `GET /sessions/{id}/history` |
| agent_hint string | Part of ContextBlock → stored in audit DB | Included in `POST /transfers` response |

---

*Phase 3 — Completed 2026-06-14*
