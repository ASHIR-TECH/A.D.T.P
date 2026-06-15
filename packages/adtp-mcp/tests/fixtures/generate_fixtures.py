"""Generate test fixture files for Phase 3 extractor tests.
Run once: python tests/fixtures/generate_fixtures.py
"""

import csv
import json
import os
import tempfile

FIXTURES_DIR = os.path.dirname(os.path.abspath(__file__))


def gen_xlsx():
    try:
        from openpyxl import Workbook
        from openpyxl.styles import Font
    except ImportError:
        return
    wb = Workbook()
    ws1 = wb.active
    ws1.title = "Invoices"
    ws1.append(["Invoice ID", "Date", "Amount", "Paid"])
    for i in range(1, 51):
        ws1.append([f"INV-{i:04d}", f"2024-0{(i % 9) + 1:02d}-{i % 28 + 1:02d}", round(100.0 + i * 10.5, 2), i % 3 != 0])
    ws2 = wb.create_sheet("Products")
    ws2.append(["SKU", "Name", "Price", "Stock"])
    for i in range(1, 21):
        ws2.append([f"SKU-{i:04d}", f"Product {i}", round(9.99 + i, 2), i * 10])
    ws3 = wb.create_sheet("Summary")
    ws3.append(["Metric", "Value"])
    ws3.append(["Total Revenue", 52500.00])
    ws3.append(["Total Products", 20])
    path = os.path.join(FIXTURES_DIR, "sample.xlsx")
    wb.save(path)
    print(f"Created {path}")


def gen_pdf():
    try:
        import fitz
    except ImportError:
        return
    doc = fitz.open()
    doc.set_metadata({"title": "Sample Report", "author": "ASHIR Tech", "subject": "Phase 3 Testing", "creator": "ADTP"})
    for i in range(6):
        page = doc.new_page()
        page.insert_text((72, 72), f"Section {i + 1}: Analysis Report", fontsize=16)
        page.insert_text((72, 120), f"This is paragraph content for section {i + 1}.", fontsize=11)
        for j in range(5):
            y = 160 + j * 40
            page.insert_text((72, y), f"• Data point {j + 1} for section {i + 1}: sample observation.", fontsize=11)
        if i == 2:
            page.insert_text((300, 400), "[Placeholder for chart image]", fontsize=9)
    path = os.path.join(FIXTURES_DIR, "sample.pdf")
    doc.save(path)
    doc.close()
    print(f"Created {path}")


def gen_docx():
    try:
        from docx import Document
        from docx.shared import Inches, Pt
    except ImportError:
        return
    doc = Document()
    core = doc.core_properties
    core.author = "ASHIR Tech"
    core.last_modified_by = "ADTP Test"
    doc.add_heading("Executive Summary", level=1)
    doc.add_paragraph("This document summarises the key findings from the Phase 3 analysis.")
    doc.add_heading("Methodology", level=2)
    doc.add_paragraph("Data was collected from multiple sources and processed using standard analytical methods.")
    doc.add_paragraph("The sample size consisted of 1000 observations across 15 variables.")
    doc.add_heading("Results", level=2)
    doc.add_paragraph("Key findings indicate a strong correlation between the primary variables.", style="Normal")
    doc.add_heading("Table of Metrics", level=3)
    table = doc.add_table(rows=4, cols=3)
    table.cell(0, 0).text = "Metric"
    table.cell(0, 1).text = "Value"
    table.cell(0, 2).text = "Change"
    for i in range(1, 4):
        table.cell(i, 0).text = f"KPI {i}"
        table.cell(i, 1).text = f"{round(100 + i * 23.5, 1)}"
        table.cell(i, 2).text = f"+{i * 5}%"
    doc.add_heading("Conclusion", level=1)
    doc.add_paragraph("The analysis confirms the initial hypothesis. Further investigation is recommended.")
    path = os.path.join(FIXTURES_DIR, "sample.docx")
    doc.save(path)
    print(f"Created {path}")


def gen_csv():
    path = os.path.join(FIXTURES_DIR, "sample.csv")
    with open(path, "w", newline="") as f:
        writer = csv.writer(f)
        writer.writerow(["Date", "Product", "Quantity", "Price", "Total", "Region"])
        for i in range(1, 151):
            writer.writerow([
                f"2024-0{(i % 12) + 1:02d}-{(i % 28) + 1:02d}",
                f"Product-{chr(65 + (i % 10))}",
                i % 20 + 1,
                round(10.0 + (i % 50) * 1.5, 2),
                round((i % 20 + 1) * (10.0 + (i % 50) * 1.5), 2),
                ["North", "South", "East", "West"][i % 4],
            ])
    print(f"Created {path}")


def gen_mp3():
    """Create a minimal valid MP3 file with ID3 tags using mutagen."""
    try:
        from mutagen.id3 import ID3, TIT2, TPE1, TALB, TDRC
    except ImportError:
        return
    path = os.path.join(FIXTURES_DIR, "sample.mp3")
    # Minimal MP3 sync frame
    with open(path, "wb") as f:
        f.write(b"\xff\xfb\x90\x00" * 100)
    try:
        audio = ID3(path)
        audio.add(TIT2(encoding=3, text="Sample Track"))
        audio.add(TPE1(encoding=3, text="Test Artist"))
        audio.add(TALB(encoding=3, text="Test Album"))
        audio.add(TDRC(encoding=3, text="2024"))
        audio.save()
    except Exception:
        pass
    print(f"Created {path}")


def gen_mp4():
    """Create a minimal valid MP4 file."""
    path = os.path.join(FIXTURES_DIR, "sample.mp4")
    # Minimal MP4 ftyp box
    with open(path, "wb") as f:
        f.write(b"\x00\x00\x00\x20ftypmp42\x00\x00\x00\x00mp42mp41")
        f.write(b"\x00\x00\x00\x08moov")
    print(f"Created {path}")


def gen_jpg():
    """Create a JPEG with EXIF metadata."""
    try:
        from PIL import Image
        from PIL.ExifTags import Base
    except ImportError:
        return
    path = os.path.join(FIXTURES_DIR, "sample.jpg")
    img = Image.new("RGB", (640, 480), color="blue")
    exif = img.getexif()
    exif[Base.Make] = "CameraMaker"
    exif[Base.Model] = "CameraModel"
    exif[Base.DateTimeOriginal] = "2024:06:14 12:00:00"
    exif[Base.Software] = "ADTP Test"
    img.save(path, exif=exif)
    print(f"Created {path}")


def gen_json():
    path = os.path.join(FIXTURES_DIR, "sample.json")
    data = [
        {"id": 1, "name": "Alice", "age": 30, "email": "alice@test.com", "address": {"city": "NYC", "zip": "10001"}, "tags": ["admin", "user"]},
        {"id": 2, "name": "Bob", "age": 25, "email": "bob@test.com", "address": {"city": "LA", "zip": "90001"}, "tags": ["user"]},
        {"id": 3, "name": "Charlie", "age": 35, "email": "charlie@test.com", "address": {"city": "CHI", "zip": "60601"}, "tags": ["admin", "editor"]},
    ]
    with open(path, "w") as f:
        json.dump(data, f, indent=2)
    print(f"Created {path}")


def gen_bin():
    path = os.path.join(FIXTURES_DIR, "sample.bin")
    with open(path, "wb") as f:
        f.write(b"\x00" * 256 + b"\xff" * 256 + b"\xde\xad\xbe\xef" * 64)
    print(f"Created {path}")


if __name__ == "__main__":
    gen_xlsx()
    gen_pdf()
    gen_docx()
    gen_csv()
    gen_mp3()
    gen_mp4()
    gen_jpg()
    gen_json()
    gen_bin()
    print("All fixtures generated.")
