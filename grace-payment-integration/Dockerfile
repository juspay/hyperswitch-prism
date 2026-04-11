FROM --platform=linux/arm64 node:20-alpine AS builder

# Install build dependencies for the native FFI library
RUN apk add --no-cache curl build-base
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

# Copy the full repo for building the Rust FFI library
WORKDIR /repo
COPY . /repo

# Build the FFI shared library
RUN cd /repo && cargo build --release -p ffi

# Build the Node.js application
WORKDIR /app
COPY grace-payment-integration/package.json grace-payment-integration/package-lock.json* ./
RUN npm install --production=false

COPY grace-payment-integration/tsconfig.json ./
COPY grace-payment-integration/src/ ./src/

RUN npm run build

# Copy the built .so into the expected location
RUN mkdir -p node_modules/hs-paylib/dist/src/payments/generated && \
    cp /repo/target/release/libconnector_service_ffi.so node_modules/hs-paylib/dist/src/payments/generated/

# --- Runtime stage ---
FROM --platform=linux/arm64 node:20-alpine

WORKDIR /app

COPY --from=builder /app/dist ./dist
COPY --from=builder /app/node_modules ./node_modules
COPY --from=builder /app/package.json ./

EXPOSE 3000

CMD ["node", "dist/index.js"]
