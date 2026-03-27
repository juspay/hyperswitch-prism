from typing import Dict, Any, List, Optional, TypedDict
from pathlib import Path
from typing_extensions import TypedDict
from src.config import TechSpecConfig


class CrawlResult(TypedDict):
    """Result from crawling a single URL."""
    success: bool
    filepath: Optional[str]
    content_length: int
    error: Optional[str]
    url: str

class WorkflowMetadata(TypedDict, total=False):
    """Metadata about the workflow execution."""
    start_time: float
    end_time: float
    duration: float
    total_urls: int
    successful_crawls: int
    failed_crawls: int
    spec_generated: bool
    estimated_tokens: Dict[str, int]
    mock_server_generated: bool
    workflow_started: bool
    timestamp: str
    scraping_failed: bool


class TechspecWorkflowState(TypedDict, total=False):
    """Complete state for the API documentation processing workflow."""

    # Configuration
    config: TechSpecConfig
    output_dir: Path
    folder : Optional[str]

    # Workflow control
    connector_name: Optional[str]
    mock_server: bool
    test_only: bool
    verbose: bool

    urls_file: Optional[str]
    # Input data
    urls: List[str]
    visited_urls: List[str]

    # Processing results
    crawl_results: Dict[str, CrawlResult]
    file_name : str
    markdown_files: List[Path]
    tech_spec: str
    spec_filepath: Path


    # Mock server results
    mock_server_dir: Path
    mock_server_process: Any
    mock_server_data: Dict[str, Any]

    # Error tracking
    errors: List[str]
    warnings: List[str]
    error: Optional[str]

    # Workflow metadata
    metadata: WorkflowMetadata

    # Output
    final_output: Dict[str, Any]
    validation_results: Dict[str, Any]

    # Node control flags
    node_config: Dict[str, Dict[str, Any]]

    # Claude Agent SDK enhancement
    enhance: bool  # Whether to run enhancement steps
    enhanced_spec: str  # Tech spec after Claude Agent enhancement
    enhanced_spec_filepath: Path  # Path to enhanced spec file
    field_dependency_analysis: str  # Field dependency analysis output
    field_dependency_filepath: Path  # Path to field dependency analysis file