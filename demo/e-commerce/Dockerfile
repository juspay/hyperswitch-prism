# Build stage - Force x86_64 platform for FFI compatibility
# Using Ubuntu 24.04 for glibc 2.39 which is compatible with the FFI library
FROM --platform=linux/amd64 ubuntu:24.04 AS builder

# Install Node.js and build tools
RUN apt-get update && apt-get install -y \
    curl \
    ca-certificates \
    cmake \
    make \
    g++ \
    python3 \
    && curl -fsSL https://deb.nodesource.com/setup_22.x | bash - \
    && apt-get install -y nodejs \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy SDK to correct relative path (for file:../../sdk/javascript in package.json)
COPY sdk/javascript /app/sdk/javascript

# Copy demo app
COPY demo/e-commerce/package.json /app/demo/e-commerce/
COPY demo/e-commerce/tsconfig.json /app/demo/e-commerce/

# Install dependencies
WORKDIR /app/demo/e-commerce
RUN npm install

# Copy source code
COPY demo/e-commerce/server ./server

# Build TypeScript
RUN npm run build

# Production stage - Force x86_64 platform for FFI compatibility
# Using Ubuntu 24.04 for glibc 2.39 which is compatible with the FFI library
FROM --platform=linux/amd64 ubuntu:24.04 AS production

# Install Node.js (production only needs runtime, not build tools)
RUN apt-get update && apt-get install -y \
    curl \
    ca-certificates \
    && curl -fsSL https://deb.nodesource.com/setup_22.x | bash - \
    && apt-get install -y nodejs \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy SDK for production
COPY sdk/javascript /app/sdk/javascript

# Copy package.json and install production deps
COPY demo/e-commerce/package.json /app/demo/e-commerce/

WORKDIR /app/demo/e-commerce
RUN npm install --omit=dev

# Copy built files from builder
COPY --from=builder /app/demo/e-commerce/dist ./dist

# Copy client files for static serving
COPY demo/e-commerce/client ./client

# Expose port
EXPOSE 3000

# Set environment
ENV NODE_ENV=production
ENV PORT=3000

# Start server
CMD ["node", "dist/index.js"]
