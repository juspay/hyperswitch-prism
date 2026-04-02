"""
Simple logging utility for the documentation generator.

Replaces print statements with proper logging levels.
"""

import logging
import sys
from pathlib import Path


def setup_logging(
    level: int = logging.INFO,
    log_file: Path = None,
    format_string: str = None
) -> logging.Logger:
    """
    Setup logging for the documentation generator.
    
    Args:
        level: Logging level (default: INFO)
        log_file: Optional file to log to
        format_string: Custom format string
        
    Returns:
        Configured logger
    """
    if format_string is None:
        format_string = "%(levelname)s: %(message)s"
    
    # Get or create logger
    logger = logging.getLogger("docgen")
    logger.setLevel(level)
    
    # Remove existing handlers to avoid duplicates
    logger.handlers = []
    
    # Console handler
    console_handler = logging.StreamHandler(sys.stderr)
    console_handler.setLevel(level)
    console_formatter = logging.Formatter(format_string)
    console_handler.setFormatter(console_formatter)
    logger.addHandler(console_handler)
    
    # File handler (optional)
    if log_file:
        file_handler = logging.FileHandler(log_file)
        file_handler.setLevel(logging.DEBUG)  # Always log debug to file
        file_formatter = logging.Formatter(
            "%(asctime)s - %(name)s - %(levelname)s - %(message)s"
        )
        file_handler.setFormatter(file_formatter)
        logger.addHandler(file_handler)
    
    return logger


def get_logger() -> logging.Logger:
    """Get the docgen logger."""
    return logging.getLogger("docgen")


# Convenience functions for quick logging
def debug(msg: str):
    """Log debug message."""
    get_logger().debug(msg)


def info(msg: str):
    """Log info message."""
    get_logger().info(msg)


def warning(msg: str):
    """Log warning message."""
    get_logger().warning(msg)


def error(msg: str):
    """Log error message."""
    get_logger().error(msg)


class ProgressTracker:
    """Track progress of long-running operations."""
    
    def __init__(self, total: int, description: str = "Processing"):
        self.total = total
        self.current = 0
        self.description = description
        self.logger = get_logger()
        
    def update(self, item: str = None):
        """Update progress."""
        self.current += 1
        if item:
            self.logger.info(f"{self.description}: {self.current}/{self.total} - {item}")
        else:
            self.logger.info(f"{self.description}: {self.current}/{self.total}")
            
    def finish(self):
        """Mark progress as complete."""
        self.logger.info(f"{self.description}: complete ({self.total}/{self.total})")
