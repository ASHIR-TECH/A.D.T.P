from __future__ import annotations

import time
from pathlib import Path

from adtp_mcp.context import ExtractionResult


class VideoExtractor:
    @staticmethod
    def extract(file_path: str | Path) -> ExtractionResult:
        start = time.perf_counter()
        schema: dict = {}
        summary = ""
        agent_hint = ""
        error: str | None = None

        try:
            import ffmpeg

            path_str = str(Path(file_path).resolve())
            probe = ffmpeg.probe(path_str, v="error")

            format_info = probe.get("format", {})
            duration = float(format_info.get("duration", 0))
            file_size = int(format_info.get("size", 0))

            video_stream = None
            audio_stream = None
            has_subtitles = False

            for stream in probe.get("streams", []):
                codec_type = stream.get("codec_type")
                if codec_type == "video" and video_stream is None:
                    video_stream = stream
                elif codec_type == "audio" and audio_stream is None:
                    audio_stream = stream
                elif codec_type == "subtitle":
                    has_subtitles = True

            width = int(video_stream.get("width", 0)) if video_stream else 0
            height = int(video_stream.get("height", 0)) if video_stream else 0
            fps_parts = (video_stream or {}).get("r_frame_rate", "0/1").split("/")
            fps = (
                round(int(fps_parts[0]) / int(fps_parts[1]), 2)
                if len(fps_parts) == 2 and int(fps_parts[1]) > 0
                else 0
            )
            video_codec = (video_stream or {}).get("codec_name", "")
            audio_codec = (
                (audio_stream or {}).get("codec_name", "") if audio_stream else ""
            )

            schema = {
                "duration_seconds": round(duration, 2),
                "width_px": width,
                "height_px": height,
                "fps": fps,
                "video_codec": video_codec,
                "audio_codec": audio_codec,
                "file_size_mb": round(file_size / (1024 * 1024), 2),
                "has_subtitles": has_subtitles,
            }

            summary = f"Video {width}x{height}, {duration:.0f}s, {video_codec.upper()}, {round(file_size / (1024 * 1024), 1)}MB"
            agent_hint = f"Video is {duration:.0f}s — analyse visual and audio content"

        except Exception as e:
            error = str(e)
            summary = "Failed to extract video metadata"
            agent_hint = "Binary file — process by filename and size only"

        elapsed = int((time.perf_counter() - start) * 1000)
        return ExtractionResult(
            schema=schema,
            summary=summary[:200],
            agent_hint=agent_hint,
            extraction_ms=elapsed,
            error=error,
        )
