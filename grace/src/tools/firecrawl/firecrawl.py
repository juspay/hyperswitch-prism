import requests
from pathlib import Path
from typing import List, Dict, Optional, Tuple
import time
import click
from src.utils.transformations import sanitize_filename
from src.tools.filemanager.filemanager import FileManager

class FirecrawlClient:

    def __init__(self, api_key: str, base_url: Optional[str] = None):
        self.api_key = api_key
        self.base_url = base_url or "https://api.firecrawl.dev/v0"
        self.session = requests.Session()
        self.session.headers.update({
            "Authorization": f"Bearer {self.api_key}",
            "Content-Type": "application/json"
        })
    
    def scrape_url(self, url: str) -> Tuple[bool, str, str]:
        try:
            payload = {
                "url": url,
                "formats": ["markdown"],
                "onlyMainContent": True,
                "includeTags": ["title", "meta"],
                "excludeTags": ["nav", "footer", "aside", "script", "style"]
            }
            
            response = self.session.post(f"{self.base_url}/scrape", json=payload)
            
            if response.status_code == 200:
                data = response.json()
                if data.get("success"):
                    markdown_content = data.get("data", {}).get("markdown", "")
                    if markdown_content:
                        return True, markdown_content, ""
                    else:
                        return False, "", "No markdown content returned"
                else:
                    error_msg = data.get("error", "Unknown error")
                    return False, "", f"Firecrawl API error: {error_msg}"
            else:
                return False, "", f"HTTP {response.status_code}: {response.text}"
                
        except requests.exceptions.RequestException as e:
            return False, "", f"Network error: {str(e)}"
        except Exception as e:
            return False, "", f"Unexpected error: {str(e)}"
    
    def scrape_urls_batch(self, urls: List[str], output_dir: Path) -> Dict[str, Dict]:
        results = {}
        filemanager = FileManager(base_path=str(output_dir))
        for idx, url in enumerate(urls, start=1):
            filename = sanitize_filename(url)
            if filemanager.read_file(filename).strip().replace("/n", "") != "":
                click.echo(f"File {filename} already exists, skipping...")
                content = filemanager.read_file(Path(filename))
                results[url] = {
                        "success": True,
                        "filepath": str(filemanager.base_path / filename),
                        "content_length": len(content),
                        "error": None
                }
                continue

            success, content, error = self.scrape_url(url)
            click.echo(f"Scraped {url}: {'Success' if success else 'Failed'}")
            if success:
                # Save markdown content to file
                try:
                    content = f"""
                                # Documentation for {url}   
                                **Source URL:** {url}
                                ---
                                {content}
                    """
                    filemanager.write_file(filename, content)

                    results[url] = {
                        "success": True,
                        "filepath": str(filemanager.base_path / filename),
                        "content_length": len(content),
                        "error": None
                    }
                except Exception as e:
                    results[url] = {
                        "success": False,
                        "filepath": None,
                        "content_length": 0,
                        "error": f"File write error: {str(e)}"
                    }
            else:
                results[url] = {
                    "success": False,
                    "filepath": None,
                    "content_length": 0,
                    "error": error
                }

            # Small delay to be respectful to the API
            time.sleep(0.5)

            # After every 10 URLs, wait for 1 minute before continuing
            if idx % 10 == 0 and idx < len(urls):
                print("Waiting for 1 minute before continuing...")
                time.sleep(60)

        return results
    
    def test_connection(self) -> Tuple[bool, str]:
        try:
            # Test with a simple URL
            test_url = "https://httpbin.org/html"
            success, content, error = self.scrape_url(test_url)
            
            if success:
                return True, "Firecrawl API connection successful"
            else:
                return False, f"Firecrawl API test failed: {error}"
                
        except Exception as e:
            return False, f"Connection test error: {str(e)}"