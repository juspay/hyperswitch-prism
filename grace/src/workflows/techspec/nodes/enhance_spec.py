"""Claude Agent SDK enhancement node — reviews scraped MDs against generated tech spec."""

import asyncio
from pathlib import Path
from typing import List

import click
from rich.console import Console

from ..states.techspec_state import TechspecWorkflowState
from src.config import get_config
from src.tools.filemanager.filemanager import FileManager
from ._claude_display import display_tool_use, display_text, display_thinking, display_result

console = Console()

# Grace project root (where enhancer.md and analysis.md live)
GRACE_ROOT = Path(__file__).parent.parent.parent.parent.parent


def _read_enhancer_prompt() -> str:
    """Read the enhancer.md prompt from grace root."""
    enhancer_path = GRACE_ROOT / "enhacer.md "
    if not enhancer_path.exists():
        # Try without trailing space
        enhancer_path = GRACE_ROOT / "enhacer.md"
    if not enhancer_path.exists():
        enhancer_path = GRACE_ROOT / "enhancer.md"
    if not enhancer_path.exists():
        raise FileNotFoundError(
            f"enhancer.md not found in {GRACE_ROOT}. "
            "Looked for: enhacer.md, enhancer.md"
        )
    return enhancer_path.read_text(encoding="utf-8")


def _build_enhancement_prompt(
    enhancer_instructions: str,
    connector_name: str,
    tech_spec_filepath: str,
    markdown_file_paths: List[str],
) -> str:
    """Build the prompt that instructs Claude to read files sequentially.
    
    Instead of embedding all content inline, provides file paths so Claude
    uses its Read tool to process files one by one with visible progress.
    """
    # Replace hardcoded references with dynamic connector name
    prompt = enhancer_instructions
    prompt = prompt.replace("Airwallex", connector_name)
    prompt = prompt.replace("airwallex", connector_name.lower())
    prompt = prompt.replace("output/airwallex", f"output/{connector_name.lower()}")

    # Build file listing
    files_listing = "\n".join(f"  {i+1}. {path}" for i, path in enumerate(markdown_file_paths))

    full_prompt = f"""{prompt}

--- CONNECTOR NAME ---
{connector_name}

--- TECHNICAL SPECIFICATION FILE (this is the file you must edit in-place) ---
{tech_spec_filepath}

--- SOURCE MARKDOWN DOCUMENTATION FILES (process each one sequentially) ---
{files_listing}

CRITICAL WORKFLOW — EDIT THE SPEC IN-PLACE:
You must modify the technical specification file directly. Do NOT output a new spec at the end.
Follow this exact loop for EACH source file:

1. First, use the Read tool to read the technical specification file: {tech_spec_filepath}
2. Understand the current structure and identify gaps
3. Then, for EACH source markdown documentation file listed above:
   a. Use the Read tool to read ONE source file
   b. Extract relevant information (API endpoints, request/response formats, authentication, error handling, etc.)
   c. Use the Edit tool to UPDATE the technical specification file ({tech_spec_filepath}) with the new information
   d. Confirm what you added/changed
   e. Move to the NEXT source file — do NOT proceed until the current file's changes are written
4. After processing ALL source files, do a final Read of the spec to verify completeness

RULES:
- ALWAYS edit the spec file in-place using Edit tool — never output a new document
- Process files ONE AT A TIME: Read source → Edit spec → next source
- Preserve existing correct information in the spec
- Add missing details, don't duplicate existing content
- Flag any conflicting information between files"""

    return full_prompt


def enhance_spec(state: TechspecWorkflowState) -> TechspecWorkflowState:
    """Enhance the tech spec using Claude Agent SDK with enhancer.md prompt."""

    tech_spec = state.get("tech_spec")
    if not tech_spec:
        console.print("[yellow]Skipping enhancement: No tech spec to enhance[/yellow]")
        return state

    click.echo(f"\nStep 3: Enhancing technical specification with Claude Agent SDK...")

    try:
        # Load the enhancer prompt
        enhancer_instructions = _read_enhancer_prompt()
    except FileNotFoundError as e:
        console.print(f"[red]Error: {e}[/red]")
        state.setdefault("errors", []).append(str(e))
        return state

    # Resolve absolute paths for markdown files so Claude can read them
    output_dir = state.get("output_dir")
    markdown_files = state.get("markdown_files", [])
    connector_name = state.get("connector_name") or state.get("file_name", "unknown")

    filemanager = FileManager(base_path=str(output_dir))

    if state.get("folder"):
        filemanager.update_base_path("")

    # Resolve markdown file paths to absolute paths
    markdown_abs_paths = []
    for md_file in markdown_files:
        abs_path = (filemanager.base_path / md_file).resolve()
        if abs_path.exists():
            markdown_abs_paths.append(str(abs_path))
        else:
            console.print(f"[yellow]Warning: File not found: {abs_path}[/yellow]")

    if not markdown_abs_paths:
        console.print("[yellow]No markdown source docs found, skipping enhancement[/yellow]")
        return state

    # Resolve tech spec file path
    spec_filepath = state.get("spec_filepath")
    if spec_filepath:
        tech_spec_abs_path = str((filemanager.base_path / "specs" / spec_filepath).resolve())
    else:
        # Fallback: write tech spec to a temp file so Claude can read it
        temp_spec_path = Path(output_dir).resolve() / "specs" / f"{connector_name.lower()}_temp_spec.md"
        temp_spec_path.parent.mkdir(parents=True, exist_ok=True)
        temp_spec_path.write_text(tech_spec, encoding="utf-8")
        tech_spec_abs_path = str(temp_spec_path)

    console.print(f"[dim]Tech spec file: {tech_spec_abs_path}[/dim]")
    console.print(f"[dim]Source files to process: {len(markdown_abs_paths)}[/dim]")
    for i, p in enumerate(markdown_abs_paths, 1):
        console.print(f"[dim]  {i}. {Path(p).name}[/dim]")

    # Build the prompt with file paths (not content)
    full_prompt = _build_enhancement_prompt(
        enhancer_instructions, connector_name, tech_spec_abs_path, markdown_abs_paths
    )

    # Get Claude Agent SDK config
    claude_config = get_config().getClaudeAgentConfig()

    if not claude_config.enabled:
        console.print("[yellow]Claude Agent SDK is disabled, skipping enhancement[/yellow]")
        return state

    try:
        from claude_agent_sdk import ClaudeSDKClient, ClaudeAgentOptions, ResultMessage, AssistantMessage

        # Build environment variables for LiteLLM proxy
        env_vars = {}
        if claude_config.api_key:
            env_vars["ANTHROPIC_API_KEY"] = claude_config.api_key
        if claude_config.base_url:
            env_vars["ANTHROPIC_BASE_URL"] = claude_config.base_url

        # Resolve output_dir to absolute path for Claude Agent SDK
        abs_output_dir = Path(output_dir).resolve() if output_dir else Path.cwd()

        options = ClaudeAgentOptions(
            allowed_tools=["Read", "Write", "Edit", "Glob", "Grep"],
            permission_mode="bypassPermissions",
            cwd=str(abs_output_dir),
            env=env_vars,
            max_turns=claude_config.max_turns,
        )
        if claude_config.model:
            options.model = claude_config.model

        # Run the Claude Agent SDK with a dedicated session for enhancement
        enhanced_result_parts = []
        turn_count = 0

        async def run_enhancement():
            nonlocal turn_count
            # Create a new ClaudeSDKClient session
            client = ClaudeSDKClient(options)
            
            try:
                await client.connect()
                
                # Send the prompt and receive responses
                await client.query(full_prompt)
                async for message in client.receive_response():
                    # console.print(message)  # Add spacing between turns
                    if isinstance(message, AssistantMessage):
                        turn_count += 1
                        for block in message.content:
                            if hasattr(block, "name") and hasattr(block, "input"):
                                # ToolUseBlock — show which tool Claude is calling
                                tool_name = block.name
                                tool_input = block.input or {}
                                display_tool_use(turn_count, tool_name, tool_input)
                            elif hasattr(block, "text"):
                                # TextBlock — Claude's reasoning / output text
                                text = block.text.strip()
                                if text:
                                    enhanced_result_parts.append(block.text)
                                    display_text(turn_count, text)
                            elif hasattr(block, "thinking"):
                                # ThinkingBlock — Claude's internal reasoning
                                display_thinking(turn_count, block.thinking)
                    elif isinstance(message, ResultMessage):
                        if message.result:
                            enhanced_result_parts.append(message.result)
                        display_result(message)
            finally:
                await client.disconnect()

        console.print()
        console.rule("[bold cyan]Claude Agent: Enhancing Specification[/bold cyan]")
        console.print()
        asyncio.run(run_enhancement())
        console.rule("[bold cyan]Enhancement Complete[/bold cyan]")
        console.print()

        # Read the enhanced spec back from disk (Claude edited it in-place)
        spec_path = Path(tech_spec_abs_path)
        if spec_path.exists():
            enhanced_spec = spec_path.read_text(encoding="utf-8")

            state["enhanced_spec"] = enhanced_spec
            state["enhanced_spec_filepath"] = spec_path

            console.print(f"[green]✓[/green] Tech spec enhanced in-place: {tech_spec_abs_path}")
            click.echo(f"Enhancement complete ({turn_count} turns)!")
        else:
            console.print("[yellow]Warning: Tech spec file not found after enhancement[/yellow]")
            state.setdefault("warnings", []).append("Enhancement: spec file not found after edit")

    except ImportError:
        error_msg = "claude-agent-sdk not installed. Install with: pip install claude-agent-sdk"
        console.print(f"[red]Error: {error_msg}[/red]")
        state.setdefault("errors", []).append(error_msg)
    except Exception as e:
        error_msg = f"Error during spec enhancement: {str(e)}"
        console.print(f"[red]Error: {error_msg}[/red]")
        state.setdefault("errors", []).append(error_msg)

    return state
