
from pathlib import Path
from typing import List

class FileManager:
    
    def __init__(self, base_path: str = None):
        if base_path is None:
            self.base_path = Path(__file__).parent.parent.parent.parent  # to root of grace
        else:
            self.base_path = Path(__file__).parent.parent.parent.parent / Path(base_path) # to root of grace
        self.base_path.mkdir(parents=True, exist_ok=True)
    
    def update_base_path(self, new_base_path: str) -> None:
        self.base_path = Path(__file__).parent.parent.parent.parent / Path(new_base_path)
        self.base_path.mkdir(parents=True, exist_ok=True)

    def list_files(self, extension: str = ".md") -> list[Path]:
        return list(self.base_path.rglob(f"*{extension}"))

    def read_file(self, file_path: Path) -> str:
        if self.check_file_exists(file_path):
            with open(self.base_path / file_path, 'r', encoding='utf-8') as f:
                return f.read()
        return ""
    
    def get_all_files(self, folder_path: Path) -> List[Path]:
        full_folder_path = self.base_path / folder_path
        if not full_folder_path.exists():
            return []
        
        if not full_folder_path.is_dir():
            return [full_folder_path.relative_to(self.base_path)]
        
        # Get all files recursively using rglob
        file_paths = []
        for file_path in full_folder_path.rglob("*"):
            if file_path.is_file():
                # Return path relative to base_path as string
                relative_path = file_path.relative_to(self.base_path)
                file_paths.append(Path(relative_path))
        
        return file_paths
    
    def get_all_files_as_texts(self, folder_path: Path) -> List[str]:
        file_paths = self.get_all_files(folder_path)
        file_texts = []
        for file_path in file_paths:
            content = self.read_file(file_path)
            file_texts.append(content)
        return file_texts

    def write_file(self, file_path: Path, content: str, mode="w") -> None:
        full_path = self.base_path / file_path
        full_path.parent.mkdir(parents=True, exist_ok=True)
        with open(full_path, mode, encoding='utf-8') as f:
            f.write(content)

    def save_tech_spec(self, content: str,  filename: str = "tech_spec.md") -> Path:
        self.write_file(Path("specs") / Path(filename), content)
        return Path(filename)
    
    def check_file_exists(self, filename: str) -> bool:
        file_path = self.base_path / filename
        return file_path.exists()

    def write_binary_file(self, file_path: Path, content: bytes) -> Path:
        self.write_file(file_path, content, "wb")

    def get_file_size(self, file_path: Path) -> int:
        full_path = self.base_path / file_path
        if full_path.exists():
            import os
            return os.path.getsize(full_path)
        return 0