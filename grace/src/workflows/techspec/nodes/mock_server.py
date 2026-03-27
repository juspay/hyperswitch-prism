import json
import re
import subprocess
import time
from pathlib import Path
from typing import Dict, Any

from rich.console import Console
from rich.progress import Progress, SpinnerColumn, TextColumn
from src.ai.ai_service import AIService
from ..states.techspec_state import TechspecWorkflowState as WorkflowState
from src.config import get_config

console = Console()


class MockServerGenerationError(Exception):
    pass


async def mock_server(state: WorkflowState) -> WorkflowState:
    # Check if mock server generation is enabled
    if not state.get("mock_server", False):
        console.print("[dim]Skipping mock server generation (not enabled)[/dim]")
        return state
    
    # Check for technical specification content from various sources
    tech_spec_content = None
    
    # Priority order for finding tech spec content
    content_sources = [
        state.get("tech_spec"),
    ]
    
    for content in content_sources:
        if content:
            tech_spec_content = content
            break
    
    if not tech_spec_content:
        error_msg = "No technical specification or content available for mock server generation"
        state.setdefault("errors", []).append(error_msg)
        console.print(f"[red]Error:[/red] {error_msg}")
        return state
    
    console.print(f"\n[bold]Generating mock server...[/bold]")
    
    try:
        # Create mock server directory
        mock_server_dir = state["output_dir"] / "mock-server"
        mock_server_dir.mkdir(exist_ok=True)
        
        with Progress(
            SpinnerColumn(),
            TextColumn("[progress.description]{task.description}"),
            console=console,
        ) as progress:
            # Step 1: Generate server code with AI
            ai_task = progress.add_task("Generating server code with AI...", total=None)
            
            ai_response = await _generate_server_code(tech_spec_content, state)
            
            progress.update(ai_task, description="AI generation complete!")
            
            # Step 2: Parse the AI response
            parse_task = progress.add_task("Parsing AI response...", total=None)
            
            parsed_data = _parse_ai_response(ai_response)
            
            progress.update(parse_task, description="Response parsed!")
            
            # Step 3: Create project files
            files_task = progress.add_task("Creating project files...", total=None)
            
            _create_project_files(mock_server_dir, parsed_data)
            
            progress.update(files_task, description="Project files created!")
            
            # Step 4: Install dependencies
            deps_task = progress.add_task("Installing npm dependencies...", total=None)
            
            _install_dependencies(mock_server_dir)
            
            progress.update(deps_task, description="Dependencies installed!")
            
            # Step 5: Start server (optional)
            server_task = progress.add_task("Starting mock server...", total=None)
            
            server_process = _start_mock_server(mock_server_dir)
            
            progress.update(server_task, description="Mock server started!")
        
        # Update state with results
        state.setdefault("metadata", {})["mock_server_generated"] = True
        state["mock_server_dir"] = mock_server_dir
        state["mock_server_process"] = server_process
        state["mock_server_data"] = parsed_data
        
        console.print(f"\n[green]✓[/green] Mock server generated successfully!")
        console.print(f"[dim]Server directory: {mock_server_dir}[/dim]")
        
        if server_process:
            console.print(f"[dim]Server PID: {server_process.pid}[/dim]")
            console.print(f"[dim]API documentation: {mock_server_dir}/api_docs.md[/dim]")
        
        # Try to open in VS Code
        try:
            subprocess.run(["code", str(mock_server_dir)], timeout=5)
            console.print(f"[green]💻[/green] Opened project in VS Code")
        except (subprocess.TimeoutExpired, FileNotFoundError):
            console.print(f"[yellow]💻[/yellow] VS Code not available")
    
    except Exception as e:
        error_msg = f"Mock server generation failed: {str(e)}"
        state.setdefault("errors", []).append(error_msg)
        state.setdefault("metadata", {})["mock_server_generated"] = False
        console.print(f"[red]Error:[/red] {error_msg}")
    
    return state


async def _generate_server_code(tech_spec: str, state: WorkflowState) -> str:
    try:
        ai_config = get_config().getAiConfig()
        llm_client = AIService(ai_config)
    except Exception as e:
        error_msg = f"Failed to initialize LLM client: {str(e)}"
        if "errors" not in state:
            state["errors"] = []
        state["errors"].append(error_msg)
        return state
    spec_success, mock_server_code, spec_error= llm_client.generate_mock_server(tech_spec)
    if spec_success and mock_server_code:
        return mock_server_code
    else:
        raise MockServerGenerationError(f"Mock server code generation failed: {spec_error}")



def _parse_ai_response(ai_response: str) -> Dict[str, Any]:
    """Parse AI response to extract JSON."""
    # Remove markdown code block markers
    clean_json = re.sub(r'```json\n?', '', ai_response)
    clean_json = re.sub(r'\n?```$', '', clean_json).strip()
    
    try:
        parsed_data = json.loads(clean_json)
        
        # Validate required fields
        required_fields = ["server_js", "package_json", "info"]
        for field in required_fields:
            if field not in parsed_data:
                raise MockServerGenerationError(f"Missing required field: {field}")
        
        return parsed_data
        
    except json.JSONDecodeError as e:
        raise MockServerGenerationError(f"Failed to parse AI response as JSON: {str(e)}")


def _create_project_files(project_dir: Path, parsed_data: Dict[str, Any]) -> None:
    """Create project directory and files."""
    files = {
        "server.js": parsed_data["server_js"],
        "package.json": parsed_data["package_json"],
        "api_docs.md": parsed_data["info"]
    }
    
    for filename, content in files.items():
        file_path = project_dir / filename
        with open(file_path, 'w', encoding='utf-8') as f:
            f.write(content)
        console.print(f"[green]✓[/green] Created {filename}")


def _install_dependencies(project_dir: Path) -> None:
    """Install npm dependencies."""
    try:
        result = subprocess.run(
            ["npm", "install"],
            cwd=project_dir,
            capture_output=True,
            text=True,
            timeout=300
        )
        
        if result.returncode == 0:
            console.print(f"[green]✓[/green] Dependencies installed successfully")
        else:
            console.print(f"[yellow]⚠️[/yellow] npm install warnings: {result.stderr}")
            
    except subprocess.TimeoutExpired:
        raise MockServerGenerationError("npm install timed out after 5 minutes")
    except FileNotFoundError:
        raise MockServerGenerationError("npm not found - please install Node.js")


def _start_mock_server(project_dir: Path):
    """Start the mock server."""
    try:
        # Start server in background
        process = subprocess.Popen(
            ["node", "server.js"],
            cwd=project_dir,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True
        )
        
        # Give the server a moment to start
        time.sleep(2)
        
        # Check if process is still running
        if process.poll() is None:
            console.print(f"[green]✓[/green] Mock server started with PID: {process.pid}")
            return process
        else:
            stdout, stderr = process.communicate()
            raise MockServerGenerationError(f"Server failed to start: {stderr}")
            
    except FileNotFoundError:
        raise MockServerGenerationError("Node.js not found - please install Node.js")
    except Exception as e:
        raise MockServerGenerationError(f"Failed to start server: {str(e)}")