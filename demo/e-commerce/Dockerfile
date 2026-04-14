# Build stage
FROM node:18-alpine AS builder

# Install build dependencies for native modules
RUN apk add --no-cache cmake make g++ python3

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

# Production stage
FROM node:18-alpine AS production

# Install runtime dependencies
RUN apk add --no-cache cmake make g++ python3 libstdc++

WORKDIR /app

# Copy SDK for production
COPY sdk/javascript /app/sdk/javascript

# Copy package.json and install production deps
COPY demo/e-commerce/package.json /app/demo/e-commerce/

WORKDIR /app/demo/e-commerce
RUN npm install --omit=dev

# Remove build tools after npm install
RUN apk del cmake make g++ python3

# Copy built files from builder
COPY --from=builder /app/demo/e-commerce/dist ./dist

# Copy client files
COPY demo/e-commerce/client ./client

# Expose port
EXPOSE 3000

# Set environment
ENV NODE_ENV=production
ENV PORT=3000

# Start server
CMD ["node", "dist/server/index.js"]