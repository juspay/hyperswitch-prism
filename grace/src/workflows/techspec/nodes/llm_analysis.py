"""LLM processing node for the workflow."""
from typing import List
import click

from ..states.techspec_state import TechspecWorkflowState
from src.ai.ai_service import AIService
from src.utils.ai_utils import estimate_token_usage
from src.config import get_config
from pathlib import Path
from src.tools.filemanager.filemanager import FileManager
from rich.progress import Progress, SpinnerColumn, TextColumn
def llm_analysis(state: TechspecWorkflowState) -> TechspecWorkflowState:
    if not state.get("markdown_files") and state.get("folder") == None:
        if "errors" not in state:
            state["errors"] = []
        state["errors"].append("No markdown files to process")
        click.echo("Error: No markdown files to process")
        return state

    click.echo(f"\nStep 2: Generating technical specification...")

    # Initialize LLM client
    try:
        ai_config = get_config().getAiConfig()
        llm_client = AIService(ai_config)
    except Exception as e:
        error_msg = f"Failed to initialize LLM client: {str(e)}"
        if "errors" not in state:
            state["errors"] = []
        state["errors"].append(error_msg)
        click.echo(f"Error: {error_msg}")
        return state


    output_dir = state.get("output_dir")
    if not output_dir:
        raise ValueError("Output directory not configured")
    filemanager = FileManager(
        base_path=str(output_dir)
    )
    if state["folder"] != None:
        filemanager.update_base_path("")
        state["markdown_files"] = filemanager.get_all_files( Path(state["folder"]))

    # Show token estimation
    try:
        token_estimate = estimate_token_usage(filemanager, state["markdown_files"], ai_config)
        if "error" not in token_estimate:
            if "metadata" not in state:
                state["metadata"] = {}
            state["metadata"]["estimated_tokens"] = token_estimate
            click.echo(f"Estimated tokens: ~{token_estimate['estimated_input_tokens']} input + {token_estimate['max_output_tokens']} output")
    except Exception as e:
        pass

    # Generate tech spec
    try:
        with Progress(SpinnerColumn(), TextColumn("[progress.description]{task.description}")) as progress:
            task = progress.add_task("Generating technical specification...", start=False)
            progress.start_task(task)
            spec_success, tech_spec, spec_error = llm_client.generate_tech_spec(
                filemanager,
                state["markdown_files"]
            )
            progress.stop_task(task)

            if spec_success and tech_spec:
                # Save the tech spec
                filemanager.update_base_path(output_dir)
                # Use connector_name if available, otherwise generate filename from LLM
                if state.get("connector_name"):
                    state["file_name"] = f"{state['connector_name']}.md"
                else:
                    state["file_name"] = llm_client.get_file_name(tech_spec) + ".md"
                spec_filepath = filemanager.save_tech_spec(tech_spec, 
                                                           state["file_name"])

                # Update state
                state["tech_spec"] = tech_spec
                state["spec_filepath"] = spec_filepath
                if "metadata" not in state:
                    state["metadata"] = {}
                state["metadata"]["spec_generated"] = True
                click.echo(f"\nTechnical specification generated!")
                click.echo(f"Saved to: {spec_filepath}")
            else:
                if "errors" not in state:
                    state["errors"] = []
                state["errors"].append(f"Tech spec generation failed: {spec_error}")
                if "metadata" not in state:
                    state["metadata"] = {}
                state["metadata"]["spec_generated"] = False
                click.echo(f"\nError generating tech spec: {spec_error}")

    except Exception as e:
        error_msg = f"Error during tech spec generation: {str(e)}"
        if "errors" not in state:
            state["errors"] = []
        state["errors"].append(error_msg)
        if "metadata" not in state:
            state["metadata"] = {}
        state["metadata"]["spec_generated"] = False
        click.echo(f"\nError: {error_msg}")

    return state