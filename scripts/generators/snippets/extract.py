"""
Snippet Extraction Utility

Extracts code blocks between [START marker] and [END marker] comments.
Used by scenario documentation generator to pull code from example files.
"""

import re
from pathlib import Path
from typing import Optional


def extract_snippet(file_path: Path, marker: str) -> Optional[str]:
    """
    Extract code between # [START marker] and # [END marker] comments.
    
    Args:
        file_path: Path to the source file
        marker: The marker name (e.g., 'stripe_config', 'authorize_flow')
    
    Returns:
        The extracted code block, or None if markers not found
    """
    if not file_path.exists():
        return None
    
    content = file_path.read_text()
    
    # Pattern matches both Python (#) and Rust/JavaScript/Kotlin (//) comments
    pattern = rf'(?:#|//)\s*\[START {re.escape(marker)}\](.*?)(?:#|//)\s*\[END {re.escape(marker)}\]'
    
    match = re.search(pattern, content, re.DOTALL)
    if match:
        # Clean up the extracted code
        code = match.group(1).strip()
        # Remove leading whitespace common to all lines
        lines = code.split('\n')
        if lines:
            # Find minimum indentation (excluding empty lines)
            min_indent = min(
                len(line) - len(line.lstrip()) 
                for line in lines 
                if line.strip()
            ) if any(line.strip() for line in lines) else 0
            
            # Remove common indentation
            lines = [line[min_indent:] if len(line) >= min_indent else line 
                    for line in lines]
            code = '\n'.join(lines)
        
        return code
    
    return None


def extract_all_snippets(file_path: Path) -> dict[str, str]:
    """
    Extract all marked snippets from a file.
    
    Returns:
        Dict mapping marker names to their code blocks
    """
    if not file_path.exists():
        return {}
    
    content = file_path.read_text()
    snippets = {}
    
    # Find all START markers
    pattern = r'(?:#|//)\s*\[START (\w+)\]'
    for match in re.finditer(pattern, content):
        marker = match.group(1)
        snippet = extract_snippet(file_path, marker)
        if snippet:
            snippets[marker] = snippet
    
    return snippets


def inject_snippet(template: str, marker: str, code: str) -> str:
    """
    Replace {{ snippets.marker }} in template with actual code.
    
    Args:
        template: The Jinja2 template string
        marker: The marker name
        code: The code to inject
    
    Returns:
        Template with snippet injected
    """
    placeholder = f'{{{{ snippets.{marker} }}}}'
    return template.replace(placeholder, code)


# Language file extensions
LANGUAGE_EXTENSIONS = {
    'python': 'py',
    'javascript': 'js',
    'kotlin': 'kt',
    'rust': 'rs'
}


def get_scenario_example_path(scenario_key: str, language: str, 
                               examples_dir: Path = Path('examples/scenarios')) -> Path:
    """
    Get the path to a scenario example file.
    
    Args:
        scenario_key: The scenario key (e.g., 'checkout_card')
        language: The programming language
        examples_dir: Base directory for examples
    
    Returns:
        Path to the example file
    """
    ext = LANGUAGE_EXTENSIONS.get(language, language)
    return examples_dir / f'{scenario_key}.{ext}'
