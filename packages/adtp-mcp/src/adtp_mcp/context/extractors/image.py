from __future__ import annotations

import time
from pathlib import Path

from adtp_mcp.context import ExtractionResult


class ImageExtractor:
    @staticmethod
    def extract(file_path: str | Path) -> ExtractionResult:
        start = time.perf_counter()
        schema: dict = {}
        summary = ""
        agent_hint = ""
        error: str | None = None

        try:
            from PIL import Image

            img = Image.open(str(file_path))
            width, height = img.size
            colour_mode = img.mode
            has_transparency = colour_mode in ("RGBA", "LA", "PA") or (
                colour_mode == "P" and "transparency" in img.info
            )

            frame_count = 1
            try:
                from PIL import ImageSequence

                frame_count = sum(1 for _ in ImageSequence.Iterator(img))
            except Exception:
                pass

            exif_data: dict = {}
            exif = img._getexif()
            if exif:
                from PIL.ExifTags import TAGS

                gps_ifd = exif.get(0x8825)
                gps_data: dict = {}
                if isinstance(gps_ifd, dict):
                    from PIL.ExifTags import GPSTAGS

                    for tag_id, value in gps_ifd.items():
                        tag_name = GPSTAGS.get(tag_id, str(tag_id))
                        gps_data[tag_name] = str(value)

                for tag_id, value in exif.items():
                    tag_name = TAGS.get(tag_id, str(tag_id))
                    if tag_name in ("Make", "Model", "DateTimeOriginal", "Software"):
                        exif_data[tag_name] = str(value)

                if gps_data:
                    exif_data["gps"] = gps_data

            schema = {
                "width_px": width,
                "height_px": height,
                "colour_mode": colour_mode,
                "has_transparency": has_transparency,
                "exif": exif_data,
                "frame_count": frame_count,
            }

            make_model = ""
            if exif_data.get("Make") and exif_data.get("Model"):
                make_model = f" {exif_data['Make']} {exif_data['Model']}"
            summary = f"Image {width}x{height}{make_model}, {colour_mode}, {frame_count} frame(s)"

            if exif_data.get("gps"):
                agent_hint = "GPS location data is available in image EXIF — consider location-based analysis"
            else:
                agent_hint = f"Image is {width}x{height} — analyse visual content and extract text if present"

            img.close()

        except Exception as e:
            error = str(e)
            summary = "Failed to extract image metadata"
            agent_hint = "Binary file — process by filename and size only"

        elapsed = int((time.perf_counter() - start) * 1000)
        return ExtractionResult(
            schema=schema,
            summary=summary[:200],
            agent_hint=agent_hint,
            extraction_ms=elapsed,
            error=error,
        )
