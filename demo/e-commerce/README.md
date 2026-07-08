# E-Commerce Payment Demo

A simple e-commerce website demonstrating the **hyperswitch-prism** payment library with Stripe (USD), GlobalPay (EUR), and Adyen (high-value payments) connectors.

## Features

- 🛒 Product catalog with cart functionality
- 💳 Embedded checkout (no redirects)
- 🔄 Smart routing: High-value (> $50) → Adyen, USD → Stripe, EUR → GlobalPay
- 💸 Payment authorization and refunds
- 🎨 Modern, responsive UI
- 🐳 Docker support with docker-compose

## Prerequisites

- Node.js >= 18.0.0
- npm or yarn
- Docker and docker-compose (optional)
- Stripe test account
- GlobalPay test account
- Adyen test account (optional, for high-value payments)

### Platform Requirements

⚠️ **Important**: This demo uses the `hyperswitch-prism` SDK which contains platform-specific native libraries compiled for **x86_64 (AMD64)** architecture.

#### Supported Platforms

| Platform | Architecture | Status | Notes |
|----------|--------------|--------|-------|
| macOS (Intel) | x86_64 | ✅ Supported | Native support |
| macOS (Apple Silicon) | arm64 | ✅ Supported via Docker | Uses x86_64 emulation |
| Linux | x86_64 | ✅ Supported | Native support |
| Linux | arm64 | ❌ Not supported | No ARM64 binaries |
| Windows (WSL2) | x86_64 | ✅ Supported | Use x86_64 Linux distro |

#### Docker Platform

The `Dockerfile` explicitly specifies `--platform=linux/amd64` to ensure compatibility with the FFI library. On Apple Silicon Macs, Docker will automatically use QEMU emulation (Rosetta 2).

If you encounter shared library errors like:
```
Error loading shared library ld-linux-x86-64.so.2
```
or
```
version `GLIBC_2.38' not found
```

Ensure your Docker Desktop is configured to use:
- **Platform**: linux/amd64 (set in Dockerfile)
- **Base Image**: Ubuntu 24.04 (provides glibc 2.39)

Do not modify the Dockerfile to use Alpine Linux or ARM64 images, as the SDK's native FFI library requires glibc 2.38+ and x86_64 architecture.

## Quick Start

### Option 1: Local Development

```bash
# 1. Install dependencies
make install
# or: npm install

# 2. Configure environment
cp .env.example .env
# Edit .env with your API keys

# 3. Start development server
make dev
# or: npm run dev
```

### Option 2: Docker

```bash
# 1. Configure environment
cp .env.example .env
# Edit .env with your API keys

# 2. Build and start
make docker-build
make docker-up
# or: docker-compose up -d

# 3. View logs
make logs
# or: docker-compose logs -f
```

### 4. Open in Browser

Navigate to [http://localhost:3000](http://localhost:3000)

## Makefile Commands

```bash
make help              # Show all available commands

# Local Development
make install           # Install npm dependencies
make dev               # Start development server with hot reload
make build             # Build TypeScript to JavaScript
make start             # Start production server

# Docker
make docker-build      # Build Docker image
make docker-up         # Start containers
make docker-down       # Stop containers
make docker-logs       # View container logs

# Utility
make stop              # Stop docker containers (alias for docker-down)
make restart           # Restart docker containers
make logs              # View docker logs (alias for docker-logs)
make clean             # Remove node_modules and dist
```

## Architecture

```
demo/e-commerce/
├── server/
│   ├── index.ts           # Express server entry point
│   ├── config.ts          # Connector configurations
│   ├── types.ts           # TypeScript types
│   └── routes/
│       ├── index.ts       # Route aggregator
│       ├── auth.ts        # SDK session endpoint
│       └── payments.ts    # Payment endpoints
├── client/
│   ├── index.html         # Storefront
│   ├── checkout.html      # Checkout page
│   ├── css/styles.css     # Styles
│   └── js/
│       ├── app.js         # Main app logic
│       ├── checkout.js    # Checkout flow
│       ├── stripe-sdk.js  # Stripe Payment Element
│       └── globalpay-sdk.js # GlobalPay Checkout
├── Dockerfile             # Docker build file
├── docker-compose.yml     # Docker compose configuration
├── Makefile               # Build and run commands
├── package.json
├── tsconfig.json
└── README.md
```

## API Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/auth/sdk-session` | Get SDK session for client tokenization |
| POST | `/api/payments/token-authorize` | Authorize payment with token |
| POST | `/api/payments/refund` | Refund a payment |
| GET | `/api/payments/:id` | Get payment status |
| GET | `/health` | Health check |

## Payment Flow

### USD Payments (Stripe)

1. User selects products with USD currency
2. Server creates Stripe PaymentIntent
3. Client loads Stripe Payment Element
4. User enters card details
5. Payment confirmed via `stripe.confirmPayment()`
6. Server authorizes with hyperswitch-prism

### EUR Payments (GlobalPay)

1. User selects products with EUR currency
2. Server creates GlobalPay access token
3. Client loads GlobalPay card form
4. User enters card details
5. Card tokenized via GlobalPay SDK
6. Server authorizes with hyperswitch-prism

## Environment Variables

```bash
# Server Configuration
PORT=3000
NODE_ENV=development
BASE_URL=http://localhost:3000

# Stripe Configuration (USD payments)
STRIPE_API_KEY=sk_test_xxx
STRIPE_PUBLISHABLE_KEY=pk_test_xxx

# GlobalPay Configuration (EUR payments)
GLOBALPAY_APP_ID=xxx
GLOBALPAY_APP_KEY=xxx
GLOBALPAY_PUBLISHABLE_KEY=xxx
```

## Testing

### Test Cards

**Stripe Test Cards:**
- `4242 4242 4242 4242` - Visa (success)
- `4000 0025 0000 3155` - 3D Secure
- `4000 0000 0000 0002` - Decline

**GlobalPay Test Cards:**
- `4263970000005262` - Visa (success)
- `4000 0000 0000 0002` - Decline

### Testing Refunds

After a successful payment:
1. Click "Process Refund" button
2. Refund will be processed for the full amount

## Payment Status Codes

| Code | Status |
|------|--------|
| 8 | CHARGED |
| 6 | AUTHORIZED |
| 4 | REFUND_SUCCESS |
| 3 | REFUND_PENDING |

## Troubleshooting

### "Failed to initialize payment session"

- Check that your API keys are correct in `.env`
- Verify the server is running
- Check browser console for errors

### "Payment failed"

- Use test cards from the list above
- Ensure you're using test API keys
- Check server logs for details

### Docker Issues

```bash
# Rebuild without cache
docker-compose build --no-cache

# View container logs
docker-compose logs -f

# Check container status
docker-compose ps
```

## License

MIT