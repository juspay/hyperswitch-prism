
PROTO_DIR=../../crates/types-traits/grpc-api-types/proto
PROTO_FILES=$(wildcard $(PROTO_DIR)/*.proto)
PYTHON_OUT=./generated

generate:
	@echo "Generating Python code from proto files..."
	@mkdir -p $(PYTHON_OUT)
	@python -m grpc_tools.protoc \
	        -I $(PROTO_DIR) \
	        --python_out=$(PYTHON_OUT) \
	        --pyi_out=$(PYTHON_OUT) \
	        --grpc_python_out=$(PYTHON_OUT) \
	        $(PROTO_FILES)
	@touch $(PYTHON_OUT)/__init__.py
	@echo "Code generation complete!"
