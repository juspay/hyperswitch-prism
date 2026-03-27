"""Shared display helpers for streaming Claude Agent SDK activity to the terminal."""

import json
from rich.console import Console

console = Console()


def display_tool_use(turn: int, tool_name: str, tool_input: dict) -> None:
    """Display a tool-use event with a formatted summary."""
    summary = _summarise_tool_input(tool_name, tool_input)
    console.print(
        f"  [dim]Turn {turn}[/dim]  [bold yellow]⚡ {tool_name}[/bold yellow]  {summary}"
    )


def _summarise_tool_input(tool_name: str, tool_input: dict) -> str:
    """Return a one-line summary of the tool call input."""
    lower = tool_name.lower()
    if lower in ("read", "readfile"):
        return f"[dim]→ {tool_input.get('file_path', tool_input.get('path', ''))!s}[/dim]"
    if lower in ("glob", "globtool"):
        return f"[dim]→ pattern: {tool_input.get('pattern', '')}[/dim]"
    if lower in ("grep", "greptool"):
        pattern = tool_input.get("pattern", tool_input.get("regex", ""))
        path = tool_input.get("path", tool_input.get("include", ""))
        return f"[dim]→ /{pattern}/ in {path}[/dim]"
    if lower in ("edit", "editfile"):
        return f"[dim]→ {tool_input.get('file_path', tool_input.get('path', ''))}[/dim]"
    if lower in ("bash", "execute"):
        cmd = tool_input.get("command", "")
        if len(cmd) > 80:
            cmd = cmd[:77] + "..."
        return f"[dim]→ $ {cmd}[/dim]"
    try:
        compact = json.dumps(tool_input, default=str)
        if len(compact) > 100:
            compact = compact[:97] + "..."
        return f"[dim]{compact}[/dim]"
    except Exception:
        return ""


def display_text(turn: int, text: str) -> None:
    """Show a short preview of Claude's text output."""
    preview = text.replace("\n", " ").strip()
    if len(preview) > 120:
        preview = preview[:117] + "..."
    console.print(f"  [dim]Turn {turn}[/dim]  [green]💬[/green] {preview}")


def display_thinking(turn: int, thinking: str) -> None:
    """Show a short preview of Claude's internal reasoning."""
    preview = thinking.replace("\n", " ").strip()
    if len(preview) > 120:
        preview = preview[:117] + "..."
    console.print(f"  [dim]Turn {turn}[/dim]  [magenta]🧠 Thinking:[/magenta] [dim]{preview}[/dim]")


def display_result(result_msg) -> None:
    """Show the final result summary from the Claude Agent SDK."""
    parts = []
    if hasattr(result_msg, "usage") and result_msg.usage:
        usage = result_msg.usage
        tokens_in = getattr(usage, "input_tokens", None) or (usage.get("input_tokens") if isinstance(usage, dict) else None)
        tokens_out = getattr(usage, "output_tokens", None) or (usage.get("output_tokens") if isinstance(usage, dict) else None)
        if tokens_in or tokens_out:
            parts.append(f"tokens: {tokens_in or '?'} in / {tokens_out or '?'} out")
    if hasattr(result_msg, "total_cost_usd") and result_msg.total_cost_usd is not None:
        parts.append(f"cost: ${result_msg.total_cost_usd:.4f}")
    summary = "  │  ".join(parts) if parts else "done"
    console.print(f"  [bold green]✓ Result[/bold green]  [dim]{summary}[/dim]")
