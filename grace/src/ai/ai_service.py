from pathlib import Path
from typing import Any, List, Optional, Tuple, Union

try:
    import litellm  # type: ignore[import-untyped]
except ImportError:
    litellm = None  # type: ignore[assignment]

from src.config import get_config
from src.types.config import AIConfig
from src.utils.ai_utils import combine_markdown_files

from .system.prompt_config import prompt_config


class AIService:
    config: AIConfig

    def __init__(self, config: Union[AIConfig, None] = None):
        if litellm is None:
            raise ImportError(
                "litellm package is required. Install with: pip install litellm"
            )

        self.config = config or get_config().getAiConfig()
        if self.config.base_url:
            litellm.api_base = self.config.base_url
        litellm.api_key = self.config.api_key

        # Enable context window fallback as suggested by LiteLLM
        litellm.context_window_fallback_dict = {
            "claude-sonnet-4-5": ["claude-sonnet-4", "claude-sonnet-4-20250514"],
            "glm-latest": ["claude-sonnet-4-5", "claude-sonnet-4-20250514"],
        }

    def generate(
        self, messages: Any, max_tokens: Optional[int] = None
    ) -> Tuple[str, bool, str]:
        try:
            # Use config max_tokens if not provided
            if max_tokens is None:
                max_tokens = self.config.max_tokens

            completion_args = {
                "model": self.config.model_id,
                "messages": messages,
                "max_tokens": max_tokens,
                "api_key": self.config.api_key,
                "temperature": self.config.temperature,
            }
            if self.config.base_url:
                completion_args["api_base"] = self.config.base_url
            response = litellm.completion(**completion_args)
            result = response.choices[0].message["content"]
            if not result or not result.strip():
                return "", False, "No content generated"
            return result, True, ""

        except Exception as e:
            return "", False, str(e)

    async def vision_generate(
        self, messages: Any, max_tokens: Optional[int] = None
    ) -> Any:
        completion_args = {
            "model": self.config.vision_model_id,
            "messages": messages,
            "api_key": self.config.api_key,
            "temperature": 0.1,
        }
        if max_tokens is not None:
            completion_args["max_tokens"] = max_tokens

        if self.config.base_url:
            completion_args["api_base"] = self.config.base_url

        # Use async completion
        response = await litellm.acompletion(**completion_args)
        result = response.choices[0].message.content
        if not result or not result.strip():
            return ""
        return result

    def generate_tech_spec(
        self, filemanager, markdown_files: List[Path]
    ) -> Tuple[bool, Optional[str], Optional[str]]:
        try:
            from src.utils.ai_utils import chunk_content_by_tokens, estimate_tokens

            combined_content: List[str] = combine_markdown_files(
                filemanager, markdown_files
            )
            if not combined_content or len(combined_content) == 0:
                return False, "", "No content found in markdown files"

            # Convert to the format expected by chunking
            pages = [
                {"url": f"file_{i}", "content": content}
                for i, content in enumerate(combined_content)
            ]

            # Estimate total tokens
            total_tokens = sum(estimate_tokens(page["content"]) for page in pages)
            print(f"Total content: ~{total_tokens:,} tokens from {len(pages)} pages")

            # Chunk into smaller pieces (80k tokens per chunk to leave room for prompt + output)
            # Context window for glm-latest is 202k, so: 80k input + prompt + 16k output = ~100k total per request
            chunks = chunk_content_by_tokens(pages, max_tokens_per_chunk=80000)
            print(f"Split into {len(chunks)} chunks")

            prompt = (
                prompt_config().get_with_values(
                    "techspecPrompt", {"content": "check in user message"}
                )
                or ""
            )

            # Generate for each chunk with reduced max_tokens
            chunk_results = []
            for i, chunk in enumerate(chunks):
                chunk_tokens = sum(estimate_tokens(page["content"]) for page in chunk)
                # Calculate safe max_tokens: leave room for input + prompt + safety margin
                # glm-latest context: 202k, so max_output = 202k - chunk_tokens - prompt_tokens - safety_margin
                prompt_tokens = estimate_tokens(prompt)
                safe_max_tokens = min(
                    16384, max(4096, 200000 - chunk_tokens - prompt_tokens - 10000)
                )

                print(
                    f"Processing chunk {i + 1}/{len(chunks)} (~{chunk_tokens:,} tokens, max_output: {safe_max_tokens})..."
                )

                # Combine pages into a single user message with clear separators
                # to ensure the system prompt is not diluted by multiple user messages
                chunk_content_parts = [
                    f"--- Document {j + 1} ---\n{page['content']}"
                    for j, page in enumerate(chunk)
                ]
                combined_chunk_content = "\n\n".join(chunk_content_parts)

                messages = [
                    {"role": "system", "content": prompt},
                    {"role": "user", "content": combined_chunk_content},
                ]

                tech_spec, success, error = self.generate(
                    messages, max_tokens=safe_max_tokens
                )
                if not success:
                    if chunk_results:
                        print(
                            f"Warning: Chunk {i + 1} failed, returning partial results"
                        )
                        return True, "\n\n".join(chunk_results), None
                    return False, None, error

                chunk_results.append(tech_spec)

            # Combine if multiple chunks
            if len(chunk_results) > 1:
                # For large documents split into many chunks, just concatenate instead of combining
                # to avoid hitting output token limits
                total_result_tokens = sum(
                    estimate_tokens(result) for result in chunk_results
                )

                # If the combined results would be too large for a single LLM call,
                # just concatenate them directly
                if total_result_tokens > 60000:  # Conservative threshold
                    print(
                        f"Combined results too large (~{total_result_tokens:,} tokens), "
                        f"concatenating {len(chunk_results)} chunks directly..."
                    )
                    return True, "\n\n".join(chunk_results), None

                # Otherwise, use LLM to merge and deduplicate
                combine_prompt = """You are a technical writer. Your task is to combine multiple parts of a technical specification into a single cohesive document.

Instructions:
1. Merge all parts into a unified document
2. Remove any duplicate information
3. Ensure consistency in terminology and formatting
4. Maintain all crucial technical details from each part
5. Organize the content logically"""

                # Combine chunk results with clear part markers
                combined_parts = [
                    f"--- Part {i + 1} of {len(chunk_results)} ---\n{result}"
                    for i, result in enumerate(chunk_results)
                ]

                # Calculate safe max_tokens for output
                combine_tokens = sum(
                    estimate_tokens(part) for part in combined_parts
                ) + estimate_tokens(combine_prompt)

                # The output should be roughly the size of the input (deduplication may reduce it)
                # Leave room for context: 200k total - input - prompt - 10k safety = output budget
                safe_combine_max = min(
                    32768, max(16384, 200000 - combine_tokens - 10000)
                )

                print(
                    f"Combining {len(chunk_results)} chunks (~{combine_tokens:,} tokens, max_output: {safe_combine_max})..."
                )

                # Combine all parts into a single user message (same pattern as chunking)
                combined_content = "\n\n".join(combined_parts)
                messages = [
                    {"role": "system", "content": combine_prompt},
                    {"role": "user", "content": combined_content},
                ]
                final_spec, success, error = self.generate(
                    messages, max_tokens=safe_combine_max
                )
                if not success:
                    print(
                        "Warning: Could not combine chunks, returning concatenated results"
                    )
                    return True, "\n\n".join(chunk_results), None
                return True, final_spec, None

            return True, chunk_results[0], None

        except Exception as e:
            return False, None, str(e)

    def get_file_name(
        self, tech_spec: str, connector: bool = True, base_name: str = "tech_spec"
    ) -> str:
        try:
            # Truncate tech spec to first 2000 chars to save tokens
            truncated_spec = tech_spec[:2000] if len(tech_spec) > 2000 else tech_spec
            
            prompt = (
                prompt_config().get_with_values(
                    "techspecFileNamePrompt",
                    {
                        "tech_spec": truncated_spec or "",
                    },
                )
                or ""
            )
            name = self.generate([{"role": "user", "content": prompt}], max_tokens=20)
            # Clean up the response - remove any extra text, quotes, or formatting
            cleaned_name = name[0].strip().split('\n')[0].split('.')[0]
            cleaned_name = cleaned_name.strip('"\'` ').replace(" ", "")
            # Remove path separators & other unsafe characters from the name
            cleaned_name = cleaned_name.replace("/", "").replace("\\", "").replace(":", "")
            # If the LLM returned something too long (likely a sentence), fall back
            if len(cleaned_name) > 40:
                cleaned_name = base_name
            return cleaned_name if cleaned_name else base_name
        except Exception as e:
            return base_name

    def generate_mock_server(
        self, tech_spec: str
    ) -> Tuple[bool, Optional[dict], Optional[str]]:
        try:
            prompt = (
                prompt_config().get_with_values(
                    "techspecMockServerPrompt", {"tech_spec": tech_spec or ""}
                )
                or ""
            )
            messages = [
                {"role": "user", "content": prompt},
            ]
            response, success, error = self.generate(messages)
            if not success:
                return False, None, error

            return True, response, None

        except Exception as e:
            return False, None, str(e)
