
import click
from typing import Dict
from ..states.techspec_state import CrawlResult, TechspecWorkflowState
from pathlib import Path
from src.tools.firecrawl.firecrawl import FirecrawlClient

def scrap_urls(state: TechspecWorkflowState) -> TechspecWorkflowState:

    if not state.get("urls"):
        if "errors" not in state:
            state["errors"] = []
        state["errors"].append("No URLs to crawl")
        return state
    click.echo("Scraping documentation...")

    try:
        config = state.get('config')
        if not config or not config.firecrawl_api_key:
            raise ValueError("Firecrawl API key not configured")
        firecrawl_client = FirecrawlClient(config.firecrawl_api_key)
    except Exception as e:
        click.echo(f"Failed to initialize Firecrawl client: {e}")
        if "errors" not in state:
            state["errors"] = []
        state["errors"].append(f"Failed to initialize Firecrawl client: {e}")
        if "metadata" not in state:
            state["metadata"] = {}
        state["metadata"]["scraping_failed"] = True
        return state

    output_dir = state.get("output_dir")
    if not output_dir:
        raise ValueError("Output directory not configured")
    markdown_dir = Path("output") / "markdown"
    markdown_dir.mkdir(parents=True, exist_ok=True)
    urls = state["urls"]
    crawl_results: Dict[str, CrawlResult] = dict()

    try:
        click.echo(f"Scraping {len(urls)} URLs using Firecrawl...")
        # Use the existing batch processing method
        raw_results = firecrawl_client.scrape_urls_batch(urls, markdown_dir)

        # Convert to our typed format
        for url, result in raw_results.items():
            crawl_result: CrawlResult = {
                "success": result["success"],
                "filepath": result["filepath"],
                "content_length": result["content_length"],
                "error": result["error"],
                "url": url
            }
            crawl_results[url] = crawl_result

    except Exception as e:
        error_msg = f"Error during crawling: {str(e)}"
        if "errors" not in state:
            state["errors"] = []
        state["errors"].append(error_msg)
        click.echo(f"\nError: {error_msg}")
        return state

    successful_crawls = []
    failed_crawls = []
    markdown_files = []

    for url in crawl_results:
        result: CrawlResult = crawl_results[url]
        if result["success"] and result["filepath"]:
            successful_crawls.append((url, result))
            markdown_files.append(Path(result["filepath"]))
            click.echo(f"{url} → {Path(result['filepath']).name}")
        else:
            failed_crawls.append((url, result))
            click.echo(f"{url}: {result['error']}")
            if "errors" not in state:
                state["errors"] = []
            state["errors"].append(f"Crawling failed for {url}: {result['error']}")

    # Update state
    state["crawl_results"] = crawl_results
    state["markdown_files"] = markdown_files
    if "metadata" not in state:
        state["metadata"] = {}
    state["metadata"]["successful_crawls"] = len(successful_crawls)
    state["metadata"]["failed_crawls"] = len(failed_crawls)

    if not successful_crawls:
        if "errors" not in state:
            state["errors"] = []
        state["errors"].append("No URLs were successfully crawled")
        click.echo("\nError: No URLs were successfully crawled")

    if failed_crawls:
        state["warnings"].append(f"{len(failed_crawls)} URL(s) failed to crawl")
        click.echo(f"\nWarning: {len(failed_crawls)} URL(s) failed to crawl")

    return state