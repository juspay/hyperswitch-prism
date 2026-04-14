.PHONY: help install dev dev-stop build start logs clean up down restart

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
	@echo "    down          Stop and remove containers (docker-compose down)"
	@echo "    logs          View container logs (follow mode)"
	@echo ""
	@echo "  Utility:"
	@echo "    restart       Restart containers (down + up)"
	@echo "    clean         Remove node_modules and dist"
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
	@echo ""
	@echo "E-Commerce Payment Demo is running!"
	@echo "  App: http://localhost:3000"
	@echo "  Health: http://localhost:3000/health"
	@echo ""

down:
	docker-compose down

logs:
	docker-compose logs -f

# Convenience aliases
restart: down up

# Cleanup
clean:
	rm -rf node_modules dist package-lock.json
