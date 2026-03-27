import re
from urllib.parse import urlparse
from typing import List, Tuple

def validate_url(url: str) -> Tuple[bool, str]:
    if not url:
        return False, "URL cannot be empty"
    
    if not url.startswith(('http://', 'https://')):
        return False, "URL must start with http:// or https://"
    
    try:
        parsed = urlparse(url)
        if not parsed.netloc:
            return False, "URL must contain a valid domain"
        if not parsed.scheme in ('http', 'https'):
            return False, "URL scheme must be http or https"
        return True, ""
    except Exception as e:
        return False, f"Invalid URL format: {str(e)}"
    

def validate_urls_batch(urls: List[str]) -> Tuple[List[str], List[Tuple[str, str]]]:
    valid_urls = []
    invalid_urls = []
    for url in urls:
        is_valid, error = validate_url(url)
        if is_valid:
            valid_urls.append(url)
        else:
            invalid_urls.append((url, error))
    
    return valid_urls, invalid_urls