.PHONY: help install dev dev-stop build start stop restart logs clean up docker-up docker-down docker-logs

# Default target
help:
	@echo "E-Commerce Payment Demo - Available commands:"
	@echo ""
	@echo "  Local Development:"
	@echo "    install       Install npm dependencies"
	@echo "    dev           Start development server with hot reload"
	@echo "    dev-stop      Stop development server"
	@echo "    build         Build TypeScript to JavaScript"
	@echo "    start         Start production server"
	@echo ""
	@echo "  Docker:"
	@echo "    up            Build and start containers (docker-compose up --build)"
	@echo "    docker-up     Start containers (docker-compose up)"
	@echo "    docker-down   Stop containers (docker-compose down)"
	@echo "    docker-logs   View container logs"
	@echo ""
	@echo "  Utility:"
	@echo "    clean         Remove node_modules and dist"
	@echo "    restart       Restart docker containers"
	@echo "    logs          View docker logs (follow mode)"
	@echo "    stop          Stop docker containers"
	@echo ""

# Local Development
install:
	npm install

dev:
	npx tsx watch server/index.ts

dev-stop:
	@pkill -f "tsx watch server/index.ts" 2>/dev/null && echo "Development server stopped" || echo "No development server running"

build:
	npm run build

start:
	node dist/server/index.js

# Docker Commands
up:
	docker-compose up --build -d


down:
	docker-compose down

logs:
	docker-compose logs -f

# Convenience aliases
stop: docker-down

restart: docker-down docker-up

logs: docker-logs

# Cleanup
clean:
	rm -rf node_modules dist package-lock.json