from __future__ import annotations

import time
from pathlib import Path

from adtp_mcp.context import ExtractionResult


class AudioExtractor:
    @staticmethod
    def extract(file_path: str | Path) -> ExtractionResult:
        start = time.perf_counter()
        schema: dict = {}
        summary = ""
        agent_hint = ""
        error: str | None = None

        try:
            from mutagen import File
            from mutagen.mp3 import MPEGInfo
            from mutagen.flac import FLAC
            from mutagen.oggvorbis import OggVorbis

            audio = File(str(file_path), easy=False)
            if audio is None:
                raise ValueError("Unrecognised audio format")

            duration = 0
            bitrate = 0
            sample_rate = 0
            channels = 0
            fmt = Path(file_path).suffix.lstrip(".").upper()
            tags: dict[str, str] = {}

            if isinstance(audio, FLAC):
                duration = audio.info.length if audio.info else 0
                sample_rate = audio.info.sample_rate if audio.info else 0
                channels = audio.info.channels if audio.info else 0
                bitrate = (
                    audio.info.bitrate // 1000
                    if audio.info and audio.info.bitrate
                    else 0
                )
                if audio.tags:
                    for key in ("title", "artist", "album", "date"):
                        val = audio.tags.get(key)
                        if val:
                            tags[key] = (
                                str(val[0]) if isinstance(val, list) else str(val)
                            )
            elif isinstance(audio, OggVorbis):
                duration = audio.info.length if audio.info else 0
                sample_rate = audio.info.sample_rate if audio.info else 0
                channels = audio.info.channels if audio.info else 0
                bitrate = (
                    audio.info.bitrate_nominal // 1000
                    if audio.info and audio.info.bitrate_nominal
                    else 0
                )
                if audio.tags:
                    for key in ("title", "artist", "album", "date"):
                        val = audio.tags.get(key)
                        if val:
                            tags[key] = (
                                str(val[0]) if isinstance(val, list) else str(val)
                            )
            elif hasattr(audio, "tags") and audio.tags:
                if hasattr(audio, "info") and isinstance(audio.info, MPEGInfo):
                    duration = audio.info.length
                    bitrate = audio.info.bitrate // 1000
                    sample_rate = audio.info.sample_rate
                    channels = audio.info.channels
                elif hasattr(audio, "info") and audio.info:
                    duration = getattr(audio.info, "length", 0)
                    bitrate = getattr(audio.info, "bitrate", 0) // 1000
                    sample_rate = getattr(audio.info, "sample_rate", 0)
                    channels = getattr(audio.info, "channels", 0)

                for key in (
                    "title",
                    "artist",
                    "album",
                    "date",
                    "\xa9nam",
                    "\xa9ART",
                    "\xa9alb",
                    "\xa9day",
                ):
                    val = audio.tags.get(key)
                    if val:
                        tag_key = {
                            "\xa9nam": "title",
                            "\xa9ART": "artist",
                            "\xa9alb": "album",
                            "\xa9day": "date",
                        }.get(key, key)
                        tags[tag_key] = (
                            str(val[0]) if isinstance(val, list) else str(val)
                        )
            else:
                if hasattr(audio, "info") and audio.info:
                    duration = getattr(audio.info, "length", 0)
                    bitrate = getattr(audio.info, "bitrate", 0) // 1000
                    sample_rate = getattr(audio.info, "sample_rate", 0)
                    channels = getattr(audio.info, "channels", 0)

            schema = {
                "duration_seconds": round(duration, 2),
                "bitrate_kbps": bitrate,
                "sample_rate_hz": sample_rate,
                "channels": channels,
                "format": fmt,
                "tags": tags,
            }

            title_str = f" — {tags.get('title', '')}" if tags.get("title") else ""
            summary = f"Audio file{title_str}, {duration:.1f}s, {bitrate} kbps, {fmt}"

            if duration < 600:
                agent_hint = (
                    "Audio is under 10 minutes — transcribe for full content analysis"
                )
            else:
                agent_hint = f"Audio is {duration:.0f}s long — summarise key topics"

        except Exception as e:
            error = str(e)
            summary = "Failed to extract audio metadata"
            agent_hint = "Binary file — process by filename and size only"

        elapsed = int((time.perf_counter() - start) * 1000)
        return ExtractionResult(
            schema=schema,
            summary=summary[:200],
            agent_hint=agent_hint,
            extraction_ms=elapsed,
            error=error,
        )
