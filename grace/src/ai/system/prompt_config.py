import yaml
from pathlib import Path
from typing import Dict, Any, Optional, List


class PromptConfig:

    def __init__(self, config_path: Optional[str] = None, promptfile: Optional[str] = "prompts.yaml"):
        if config_path is None:
            # Default to prompts.yaml in the same directory
            self.config_path: Path = Path(__file__).parent / promptfile
        else:
            self.config_path = Path(config_path)

        self._prompts: Dict[str, Any] = {}
        self._load_prompts()

    def _load_prompts(self) -> None:
        try:
            with open(self.config_path, 'r', encoding='utf-8') as f:
                self._prompts = yaml.safe_load(f) or {}
        except FileNotFoundError:
            raise FileNotFoundError(f"Prompts configuration file not found: {self.config_path}")
        except yaml.YAMLError as e:
            raise ValueError(f"Error parsing YAML file: {e}")

    def get(self, prompt_name: str, **kwargs: Any) -> str:
        if prompt_name not in self._prompts:
            raise KeyError(f"Prompt '{prompt_name}' not found in configuration")

        prompt = self._prompts[prompt_name]

        if kwargs:
            return str(prompt.format(**kwargs))
        return str(prompt)

    def get_with_values(self, prompt_name: str, values: Dict[str, str]) -> str:
        prompt = self.get(prompt_name)

        for key, value in values.items():
            prompt = prompt.replace(f"{{{key}}}", value)
        return prompt

    def get_all(self) -> Dict[str, Any]:
        return self._prompts.copy()

    def reload(self) -> None:
        self._load_prompts()

    @property
    def prompt_names(self) -> List[str]:
        return list(self._prompts.keys())


# Singleton instance for easy access
_prompt_config_instance: Optional[PromptConfig] = None


def prompt_config(config_path: Optional[str] = None, promptfile: Optional[str] = "prompts.yaml") -> PromptConfig:
    global _prompt_config_instance

    if _prompt_config_instance is None:
        _prompt_config_instance = PromptConfig(config_path, promptfile)

    return _prompt_config_instance
