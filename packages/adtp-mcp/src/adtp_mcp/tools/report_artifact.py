from __future__ import annotations

from pydantic import BaseModel

from adtp_mcp.bridge.tasks import report_artifact as bridge_report_artifact
from adtp_mcp.security.validator import validate_all_strings


class ReportArtifactInput(BaseModel):
    task_id: str
    file_path: str
    artifact_name: str


class ReportArtifactResult(BaseModel):
    task_id: str
    artifact_name: str
    state: str


async def report_artifact(input_data: ReportArtifactInput) -> dict:
    validate_all_strings(
        (
            ("task_id", input_data.task_id),
            ("file_path", input_data.file_path),
            ("artifact_name", input_data.artifact_name),
        )
    )
    result = await bridge_report_artifact(
        input_data.task_id, input_data.file_path, input_data.artifact_name
    )
    return ReportArtifactResult(
        task_id=result.get("task_id", input_data.task_id),
        artifact_name=input_data.artifact_name,
        state=result.get("state", "artifact_ready"),
    ).model_dump()
