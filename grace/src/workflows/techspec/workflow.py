import asyncio
import click
from typing import Dict, Any, Literal, Optional
from pathlib import Path
from src.config import get_config
from langgraph.graph import StateGraph, START, END
from .states.techspec_state import TechspecWorkflowState
from datetime import datetime
from .nodes import collect_urls, scrap_urls, llm_analysis, output_node, mock_server, enhance_spec, field_analysis
class TechspecWorkflow:
    def __init__(self):
        self.graph = self._build_workflow_graph()

    def _build_workflow_graph(self):

        # Create state graph
        workflow = StateGraph(TechspecWorkflowState)

        # Add nodes for each step
        workflow.add_node("collect_urls", collect_urls)
        workflow.add_node("crawling", scrap_urls)
        workflow.add_node("llm_analysis", llm_analysis)
        workflow.add_node("enhance_spec", enhance_spec)
        workflow.add_node("field_analysis", field_analysis)
        workflow.add_node("mock_server", lambda state: asyncio.run(mock_server(state)))
        workflow.add_node("output", output_node)
        workflow.add_node("end", lambda state: state)  # Terminal node

        # Add edges to define workflow flow



        workflow.add_conditional_edges(
            START,
            self._should_continue_for_tech_spec_from_folder,
            {
                "collect_urls": "collect_urls",
                "llm_analysis": "llm_analysis"
            }
        )

        workflow.add_conditional_edges(
            "collect_urls",
            self._should_continue_after_url_collection,
            {
                "crawling": "crawling",
                "end": "end"
            }
        )
        
        workflow.add_conditional_edges(
            "crawling", 
            self._should_continue_after_crawling,
            {
                "llm_analysis": "llm_analysis",
                "end": "end"
            }
        )
        
        workflow.add_conditional_edges(
            "llm_analysis",
            self._should_continue_after_llm,
            {
                "enhance_spec": "enhance_spec",
                "mock_server": "mock_server",
                "output": "output",
                "end": "end"
            }
        )
        
        # Enhancement step flows to field analysis
        workflow.add_edge("enhance_spec", "field_analysis")

        # Field analysis flows to mock_server or output
        workflow.add_conditional_edges(
            "field_analysis",
            self._should_continue_after_field_analysis,
            {
                "mock_server": "mock_server",
                "output": "output",
                "end": "end"
            }
        )
                
        workflow.add_conditional_edges(
            "mock_server",
            self._should_continue_after_mock_server,
            {
                "output": "output",
                "end": "end"
            }
        )
        
        workflow.add_edge("output", "end")
        workflow.add_edge("end", END)

        # Compile the graph
        return workflow.compile()
    def _should_continue_for_tech_spec_from_folder(self, state: TechspecWorkflowState) -> Literal["collect_urls", "llm_analysis"]:
        if state["folder"] != None:
            click.echo(f"Using existing docs in side {state['folder']}")
            return "llm_analysis"
        return "collect_urls"
    
    def _should_continue_after_url_collection(self, state: TechspecWorkflowState) -> Literal["crawling", "end"]:
        if not state["urls"]:
            click.echo("No URLs collected. Ending workflow.")
            return "end"
        return "crawling"

    def _should_continue_after_crawling(self, state: TechspecWorkflowState) -> Literal["llm_analysis", "end"]:
        if not state["markdown_files"]:
            click.echo("No files successfully crawled. Ending workflow.")
            return "end"
        return "llm_analysis"

    def _should_continue_after_llm(self, state: TechspecWorkflowState) -> Literal["enhance_spec", "mock_server", "output", "end"]:
        # Check if enhancement is enabled and we have a spec
        if state.get("enhance") and state.get("tech_spec"):
            return "enhance_spec"
        # Check if mock server generation is enabled and we have a spec
        if (state.get("mock_server") and state.get("tech_spec")):
            return "mock_server"
        return "output"

    def _should_continue_after_field_analysis(self, state: TechspecWorkflowState) -> Literal["mock_server", "output", "end"]:
        # After field analysis, check if mock server is enabled
        if state.get("mock_server") and (state.get("enhanced_spec") or state.get("tech_spec")):
            return "mock_server"
        return "output"


    def _should_continue_after_mock_server(self, state: TechspecWorkflowState) -> str:
        if state.get("errors"):
            return "end"
        return "output"
    

     
    async def execute(self,
                     connector_name: str,
                     folder: Optional[str],
                     urls_file: Optional[str] = None,
                     output_dir: Optional[str] = None,
                     mock_server: bool = False,
                     enhance: bool = False,
                     test_only: bool = False,
                     verbose: bool = False) -> Dict[str, Any]:
        """Execute the techspec workflow."""
        config = get_config().getTechSpecConfig()

        # Convert output_dir to Path object
        output_path = Path(output_dir) if output_dir else Path(config.output_dir)

        # Initialize state
        initial_state: TechspecWorkflowState = {
            "connector_name": connector_name,
            "urls_file": urls_file,
            "urls": [],
            "visited_urls": [],
            "folder" : folder or None,
            "output_dir": output_path,
            "mock_server" : mock_server,
            "enhance": enhance,
            "config": config,
            "test_only": test_only,
            "verbose": verbose,
            "final_output": {},
            "warnings": [],
            "error": None,
            "errors": [],
            "metadata": {"workflow_started": True, "timestamp": datetime.now().isoformat()},
        }

        try:
            # Execute the workflow graph
            result = await self.graph.ainvoke(initial_state)

            return {
                "success": result["error"] == None,
                "connector_name": result["connector_name"],
                "output": result["final_output"],
                "metadata": result["metadata"],
                "error": result["error"],
                "validation_status": result["validation_results"] if result["validation_results"] else "unknown",
                "files_generated": result["final_output"].get("summary", {}).get("total_files", 0) if result["final_output"] else 0
            }

        except Exception as e:
            return {
                "success": False,
                "connector_name": connector_name,
                "output": {},
                "metadata": {"error": str(e), "workflow_failed": True},
                "error": str(e),
                "validation_status": "failed",
                "files_generated": 0
            }
        



# Factory function for easy workflow creation
def create_techspec_workflow() -> TechspecWorkflow:
    return TechspecWorkflow()


# CLI integration function
async def run_techspec_workflow(connector_name: str,
                               folder: Optional[str],
                                urls_file: Optional[str] = None,
                               output_dir: Optional[str] = None,
                               mock_server: bool = False,
                               enhance: bool = False,
                               test_only: bool = False,
                               verbose: bool = False,
                               ) -> Dict[str, Any]:
    workflow = create_techspec_workflow()
    return await workflow.execute(
        connector_name=connector_name,
        folder=folder,
        urls_file=urls_file,
        output_dir=output_dir,
        mock_server=mock_server,
        enhance=enhance,
        test_only=test_only,
        verbose=verbose
    )