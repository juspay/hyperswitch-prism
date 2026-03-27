
from typing import List


from src.tools.filemanager.filemanager import FileManager
from src.utils.validations import  validate_url
from src.utils.transformations import deduplicate_urls

from ..states.techspec_state import TechspecWorkflowState
from rich.console import Console
from pathlib import Path
console = Console()

def _parse_urls_from_input(input_text: str) -> List[str]:
    """Parse URLs from input text, splitting by newlines only."""
    if not input_text.strip():
        return []
    
    # Split by newlines and return non-empty lines as URLs
    urls = [line.strip() for line in input_text.split('\n') if line.strip()]
    return urls


def collect_urls(state: TechspecWorkflowState) -> TechspecWorkflowState:
    urls = []
    if state.get("urls_file"):
        urls_file = state["urls_file"]
        filemanager = FileManager()
        try:
            files = filemanager.get_all_files(urls_file)
            for file in files:
                file_content = filemanager.read_file(file)
                parsed_urls = _parse_urls_from_input(file_content)
                if not parsed_urls:
                    console.print("[yellow]No URLs found in the file.[/yellow]")
                else:
                    for url in parsed_urls:
                        is_valid, error = validate_url(url)
                        if not is_valid:
                            console.print(f"[red]Invalid URL in file: {url} - {error}[/red]")
                            continue
                        urls.append(url)
            if urls:
                console.print(f"[green]Collected {len(urls)} URLs from file: {urls_file}[/green]")
        except Exception as e:
            console.print(f"[red]Error reading URLs from file: {e}[/red]")
            if "errors" not in state:
                state["errors"] = []
            state["errors"].append(f"Error reading URLs from file: {e}")

    # Collect multi-line input until an empty line is entered (after content)
    lines = []
    console.print("\n\n[bold]Enter URLs (one per line). Press Enter on an empty line to generate techspec:[/bold]")
    while True:
        try:
            line = input()
            is_empty = not line.strip()
            
            if is_empty:
                # If we have content and get an empty line, finish
                # If no content yet, allow one empty line (for two consecutive newlines case)
                if urls or lines: 
                    break
            else:
                lines.append(line)
        except EOFError:
            break
    
    # Combine all lines into a single input string
    user_input = '\n'.join(lines)
    
    if user_input.strip():
        parsed_urls = _parse_urls_from_input(user_input)
        if not parsed_urls:
            console.print("No URLs found in input")
        else:
            for url in parsed_urls:
                is_valid, error = validate_url(url)
                if not is_valid:
                    console.print(f"[red]Invalid URL: {url} - {error}[/red]")
                    # state["warning"].append(f"Invalid URL: {url} - {error}")
                    continue

                urls.append(url)
                console.print(f"[green]Added from input: {url}[/green]")

    urls = deduplicate_urls(urls)

    state["urls"] = urls
    if "metadata" not in state:
        state["metadata"] = {}
    state["metadata"]["total_urls"] = len(urls)
    if urls:
        console.print(f"\nProcessing {len(urls)} URL(s):")
        for i, url in enumerate(urls, 1):
            console.print(f"  {i}. {url}")
    else:
        console.print("No valid URLs provided.")
        state["errors"].append("No valid URLs collected")
    return state