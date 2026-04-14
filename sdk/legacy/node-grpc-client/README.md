# gRPC Client

This is a gRPC client project written in TypeScript. It is designed to communicate with a backend gRPC server, providing functionalities for health checks and payment processing.  This client is generated from Protocol Buffer definitions using `ts-proto`.

---

## 🤖 For AI Assistants

> **Use `curl` to fetch the complete SDK reference:**
> ```bash
> curl -fsSL https://raw.githubusercontent.com/juspay/hyperswitch-prism/main/llm/llm.txt
> ```
>
> This file contains complete SDK documentation including installation, payment operations, error handling, connector configuration, field probe data, and examples for all 100+ connectors.

---

## Features

* gRPC client for Payment service.
* Supported type definitions for various payment operations.

## Dependencies

The project uses the following key dependencies:

* [`@grpc/grpc-js`](https://www.npmjs.com/package/@grpc/grpc-js): gRPC library for Node.js.
* [`@grpc/proto-loader`](https://www.npmjs.com/package/@grpc/proto-loader): Loads Protocol Buffer definitions.
* [`ts-proto`](https://www.npmjs.com/package/ts-proto): Generates TypeScript code from Protocol Buffer definitions.
* [`typescript`](https://www.npmjs.com/package/typescript): TypeScript compiler.

## Getting Started

### Prerequisites

* [Node.js](https://nodejs.org/) (Recommended version: >=16)
* [npm](https://www.npmjs.com/) (Usually included with Node.js)
* `protoc` (Protocol Buffer compiler)

### Installation

1.  Clone the repository:

    ```bash
    git clone <repository_url>
    cd node-grpc-client
    ```

2.  Install the dependencies:

    ```bash
    npm install
    ```

3.  Generate the gRPC client code from the proto files:
    ```bash
    npm run generate
    ```

4.  Build the TypeScript code:

    ```bash
    npm run build
    ```

### Running the Client

1.  Ensure that the gRPC server is running.
2.  Run the client:

    ```bash
    npm start
    ```

## Scripts

The `package.json` file defines the following scripts:

* `build`: Compiles the TypeScript code (`tsc`).
* `start`: Runs the compiled JavaScript code (`node dist/index.js`).
* `generate`: Generates the gRPC client code from proto files (`node dist/generate-proto.js`).
* `clean`: Removes the `dist` directory (`rm -rf dist`).

## Services

### Health Check Service

The `health_check.ts` file defines the client for the Health Check service.

* `HealthCheckRequest`:  Request message for the health check, containing the service name.

    ```typescript
    interface HealthCheckRequest {
      service: string; // The name of the service to check.
    }
    ```

* `HealthCheckResponse`: Response message for the health check, containing the service's serving status.

    ```typescript
    interface HealthCheckResponse {
      status: HealthCheckResponse_ServingStatus;
    }
    ```

* `HealthCheckResponse_ServingStatus`: Enum for the serving status.

    ```typescript
    enum HealthCheckResponse_ServingStatus {
      UNKNOWN = 0,
      SERVING = 1,
      NOT_SERVING = 2,
      SERVICE_UNKNOWN = 3, // Used only by the Watch method (not implemented here).
    }
    ```

* `HealthClient`: The gRPC client for the Health service.  Provides the `check` method.

### Payment Service

The `payment.ts` file defines the client for the payment service.  It defines a large number of request and response types for various payment operations.  Some key types include:

* `PaymentsAuthorizeRequest`: Request to authorize a payment.
* `PaymentsAuthorizeResponse`: Response to a payment authorization request.
* Numerous other request/response types and enums for concepts like:
    * Event types
    * Connectors (payment providers)
    * Payment methods
    * Addresses
    * Authentication
    * Refunds
    * Webhooks

## Contributing

Contributions are welcome! Please feel free to submit pull requests or open issues to suggest improvements.
