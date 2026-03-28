"""Worker process for executing Grace integration tasks."""

import asyncio
import sys
from pathlib import Path
from typing import Dict, Any

# Add Grace to path
GRACE_ROOT = Path(__file__).parent.parent.parent / "grace"
sys.path.insert(0, str(GRACE_ROOT))

from src.workflows.integration_workflow import run_integration_workflow
from .queue import TaskQueue
from .config import get_config


def run_worker(task_queue: TaskQueue, worker_id: int):
    """Run a worker process that consumes tasks from queue.

    This function runs in a separate process and continuously
    picks up tasks until queue is empty.
    """
    print(f"[Worker {worker_id}] Started")

    while True:
        # Get next task (blocks until available or timeout)
        task_id = task_queue.get_task(timeout=1)

        if task_id is None:
            # Timeout - check if we should exit
            if task_queue.is_empty():
                print(f"[Worker {worker_id}] Queue empty, exiting")
                break
            continue

        # Get task details
        task_dict = task_queue.get_task_dict(task_id)
        if not task_dict:
            print(f"[Worker {worker_id}] Task {task_id} not found")
            continue

        connector = task_dict["connector"]
        flow = task_dict["flow"]
        input_file = task_dict["input_file"]

        print(f"[Worker {worker_id}] Processing: {connector}/{flow}")
        task_queue.mark_processing(task_id)

        try:
            # Read developer input if provided
            developer_thoughts = ""
            input_path = Path(input_file)
            if input_path.exists():
                developer_thoughts = input_path.read_text()
                print(f"[Worker {worker_id}] Loaded input from {input_file}")

            # Find techspec
            config = get_config()
            specs_path = Path(config["specs_path"])
            techspec_path = specs_path / f"{connector.lower()}.md"

            if not techspec_path.exists():
                # Try capitalized version
                techspec_path = specs_path / f"{connector.capitalize()}.md"

            if not techspec_path.exists():
                raise FileNotFoundError(
                    f"Techspec not found for {connector}. "
                    f"Expected at: {specs_path}/{connector}.md"
                )

            print(f"[Worker {worker_id}] Using techspec: {techspec_path}")

            # Run Grace integration workflow
            result = asyncio.run(run_integration_workflow(
                connector_name=connector,
                flow=flow,
                techspec_path=str(techspec_path),
                verbose=True
            ))

            if result.get("success"):
                print(f"[Worker {worker_id}] ✅ Completed: {connector}/{flow}")
                task_queue.mark_completed(task_id, result)
            else:
                error = result.get("error", "Unknown error")
                print(f"[Worker {worker_id}] ❌ Failed: {connector}/{flow} - {error}")
                task_queue.mark_failed(task_id, error)

        except Exception as e:
            print(f"[Worker {worker_id}] ❌ Exception: {e}")
            task_queue.mark_failed(task_id, str(e))

    print(f"[Worker {worker_id}] Exited")
