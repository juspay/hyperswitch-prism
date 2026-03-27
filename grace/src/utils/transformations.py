import re

from typing import List, Tuple

def sanitize_filename(url: str) -> str:

    filename = url.replace('https://', '').replace('http://', '')

    filename = re.sub(r'[^\w\-_.]', '_', filename)
    filename = re.sub(r'_+', '_', filename)
    filename = filename.strip('_')
    
    # Ensure it ends with .md
    if not filename.endswith('.md'):
        filename += '.md'
    
    return filename

def deduplicate_urls(urls: List[str]) -> List[str]:

    seen = set()
    unique_urls = []
    
    for url in urls:
        normalized = url.rstrip('/')
        if normalized not in seen:
            seen.add(normalized)
            unique_urls.append(url)
    
    return unique_urls