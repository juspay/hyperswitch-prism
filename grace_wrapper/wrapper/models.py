"""Task models for Grace Queue Wrapper."""

from dataclasses import dataclass, field
from datetime import datetime
from enum import Enum
from typing import Optional
import uuid


class TaskStatus(Enum):
    """Task status states."""
    PENDING = "pending"
    PROCESSING = "processing"
    COMPLETED = "completed"
    FAILED = "failed"


@dataclass
class Task:
    """Represents a connector integration task."""
    connector: str
    flow: str
    input_file: str
    id: str = field(default_factory=lambda: str(uuid.uuid4())[:8])
    status: TaskStatus = TaskStatus.PENDING
    created_at: datetime = field(default_factory=datetime.now)
    started_at: Optional[datetime] = None
    completed_at: Optional[datetime] = None
    error_message: Optional[str] = None
    result: Optional[dict] = None

    def to_dict(self) -> dict:
        """Convert task to dictionary for serialization."""
        return {
            "id": self.id,
            "connector": self.connector,
            "flow": self.flow,
            "input_file": self.input_file,
            "status": self.status.value,
            "created_at": self.created_at.isoformat() if self.created_at else None,
            "started_at": self.started_at.isoformat() if self.started_at else None,
            "completed_at": self.completed_at.isoformat() if self.completed_at else None,
            "error_message": self.error_message,
            "result": self.result,
        }

    @classmethod
    def from_line(cls, line: str) -> Optional["Task"]:
        """Parse a task from a tasks.txt line.

        Format: connector|flow|input_file
        Example: paypal|Authorize|./inputs/paypal.txt
        """
        line = line.strip()
        if not line or line.startswith("#"):
            return None

        parts = line.split("|")
        if len(parts) != 3:
            return None

        connector, flow, input_file = parts
        return cls(
            connector=connector.strip(),
            flow=flow.strip(),
            input_file=input_file.strip(),
        )

    def __str__(self) -> str:
        return f"Task({self.id}: {self.connector}/{self.flow} [{self.status.value}])"
