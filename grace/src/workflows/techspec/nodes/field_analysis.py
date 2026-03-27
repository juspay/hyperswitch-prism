"""Claude Agent SDK field dependency analysis node — performs API sequence analysis using analysis.md."""

import asyncio
from pathlib import Path

import click
from rich.console import Console

from ..states.techspec_state import TechspecWorkflowState
from src.config import get_config
from src.tools.filemanager.filemanager import FileManager
from ._claude_display import display_tool_use, display_text, display_thinking, display_result

console = Console()

# Grace project root (where analysis.md lives)
GRACE_ROOT = Path(__file__).parent.parent.parent.parent.parent


def _read_analysis_prompt() -> str:
    """Read the analysis.md prompt from grace root."""
    analysis_path = GRACE_ROOT / "analysis.md"
    if not analysis_path.exists():
        raise FileNotFoundError(
            f"analysis.md not found at {analysis_path}."
        )
    return analysis_path.read_text(encoding="utf-8")


def _build_analysis_prompt(
    analysis_instructions: str,
    connector_name: str,
    tech_spec_filepath: str,
) -> str:
    """Build the prompt for the field dependency analysis step.
    
    Instead of embedding the full tech spec, provides the file path so Claude
    uses its Read tool to read and analyze it with visible progress.
    """
    full_prompt = f"""{analysis_instructions}

--- CONNECTOR NAME ---
{connector_name}

--- TECHNICAL SPECIFICATION FILE (edit this file in-place) ---
{tech_spec_filepath}

CRITICAL WORKFLOW — EDIT THE TECH SPEC IN-PLACE:
You must add the API sequence and field dependency analysis DIRECTLY into the technical specification file.
Do NOT create a separate analysis document. Do NOT just output text.

Follow this process:
1. Use the Read tool to read the technical specification file: {tech_spec_filepath}
2. Analyze the specification to identify all API flows
3. For each flow, determine:
   - Field source categorization (USER_PROVIDED, PREVIOUS_API, UNDECIDED)
   - Prerequisite API call chains and their sequence
   - Complete field dependency map
4. Use the Edit tool to ADD the following sections to the END of the tech spec file:
   - "## API Call Sequences" — showing the ordered sequence for each flow
   - "## Field Dependency Analysis" — showing field sources and prerequisites
   - "## UNDECIDED Fields" — listing fields needing clarification with specific questions
5. Do a final Read to verify the spec now contains the analysis

RULES:
- ALWAYS use Edit tool to modify the tech spec file — never output a separate document
- Preserve ALL existing content in the spec
- Append the analysis sections at the end
- Show your reasoning as you analyze each flow"""

    return full_prompt


def field_analysis(state: TechspecWorkflowState) -> TechspecWorkflowState:
    """Perform API field dependency analysis using Claude Agent SDK with analysis.md prompt."""

    # Use enhanced spec if available, otherwise fall back to original tech spec
    tech_spec = state.get("enhanced_spec") or state.get("tech_spec")
    if not tech_spec:
        console.print("[yellow]Skipping field analysis: No tech spec available[/yellow]")
        return state

    connector_name = state.get("connector_name") or state.get("file_name", "unknown")
    click.echo(f"\nStep 4: Running API field dependency analysis for {connector_name}...")

    try:
        analysis_instructions = _read_analysis_prompt()
    except FileNotFoundError as e:
        console.print(f"[red]Error: {e}[/red]")
        state.setdefault("errors", []).append(str(e))
        return state

    # Resolve the tech spec file path so Claude can read it from disk
    output_dir = state.get("output_dir")
    filemanager = FileManager(base_path=str(output_dir))

    # Determine which spec file to point Claude at
    enhanced_spec_filepath = state.get("enhanced_spec_filepath")
    spec_filepath = state.get("spec_filepath")

    if enhanced_spec_filepath and Path(enhanced_spec_filepath).exists():
        tech_spec_abs_path = str(Path(enhanced_spec_filepath).resolve())
    elif spec_filepath:
        tech_spec_abs_path = str((filemanager.base_path / "specs" / spec_filepath).resolve())
    else:
        # Fallback: write spec to a temp file so Claude can read it
        temp_path = Path(output_dir).resolve() / "specs" / f"{connector_name.lower()}_analysis_input.md"
        temp_path.parent.mkdir(parents=True, exist_ok=True)
        temp_path.write_text(tech_spec, encoding="utf-8")
        tech_spec_abs_path = str(temp_path)

    console.print(f"[dim]Analyzing spec file: {tech_spec_abs_path}[/dim]")

    # Build prompt with file path (not content)
    full_prompt = _build_analysis_prompt(analysis_instructions, connector_name, tech_spec_abs_path)

    # Get Claude Agent SDK config
    claude_config = get_config().getClaudeAgentConfig()

    if not claude_config.enabled:
        console.print("[yellow]Claude Agent SDK is disabled, skipping field analysis[/yellow]")
        return state

    output_dir = state.get("output_dir")

    try:
        from claude_agent_sdk import ClaudeSDKClient, ClaudeAgentOptions, ResultMessage, AssistantMessage

        # Build environment variables for LiteLLM proxy
        env_vars = {}
        if claude_config.api_key:
            env_vars["ANTHROPIC_API_KEY"] = claude_config.api_key
        if claude_config.base_url:
            env_vars["ANTHROPIC_BASE_URL"] = claude_config.base_url

        # Create analysis output directory
        analysis_dir = Path(output_dir).resolve() / "field-analysis" if output_dir else Path.cwd() / "field-analysis"
        analysis_dir.mkdir(parents=True, exist_ok=True)

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

        # Run the Claude Agent SDK with a dedicated session for field analysis
        analysis_result_parts = []
        turn_count = 0

        async def run_analysis():
            nonlocal turn_count
            # Create a new ClaudeSDKClient session
            client = ClaudeSDKClient(options)
            
            try:
                await client.connect()
                
                # Send the prompt and receive responses
                await client.query(full_prompt)
                async for message in client.receive_response():
                    # console.print(message) 
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
                                    analysis_result_parts.append(block.text)
                                    display_text(turn_count, text)
                            elif hasattr(block, "thinking"):
                                # ThinkingBlock — Claude's internal reasoning
                                display_thinking(turn_count, block.thinking)
                    elif isinstance(message, ResultMessage):
                        if message.result:
                            analysis_result_parts.append(message.result)
                        display_result(message)
            finally:
                await client.disconnect()

        console.print()
        console.rule("[bold cyan]Claude Agent: Field Dependency Analysis[/bold cyan]")
        console.print()
        asyncio.run(run_analysis())
        console.rule("[bold cyan]Field Analysis Complete[/bold cyan]")
        console.print()

        # Read the updated spec back from disk (Claude edited it in-place)
        spec_path = Path(tech_spec_abs_path)
        if spec_path.exists():
            updated_spec = spec_path.read_text(encoding="utf-8")

            state["field_dependency_analysis"] = updated_spec
            state["field_dependency_filepath"] = spec_path

            console.print(f"[green]✓[/green] API sequence added to tech spec: {tech_spec_abs_path}")
            click.echo(f"Field dependency analysis complete ({turn_count} turns)!")
        else:
            console.print("[yellow]Warning: Tech spec file not found after analysis[/yellow]")
            state.setdefault("warnings", []).append("Field analysis: spec file not found after edit")

    except ImportError:
        error_msg = "claude-agent-sdk not installed. Install with: pip install claude-agent-sdk"
        console.print(f"[red]Error: {error_msg}[/red]")
        state.setdefault("errors", []).append(error_msg)
    except Exception as e:
        error_msg = f"Error during field analysis: {str(e)}"
        console.print(f"[red]Error: {error_msg}[/red]")
        state.setdefault("errors", []).append(error_msg)

    return state
