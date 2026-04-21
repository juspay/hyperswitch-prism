# Project Brief: Connector Service

## Project Overview
The Connector Service is an open-source, stateless merchant payments abstraction service built using gRPC. It serves as the "Linux moment" for payments, providing a unified contract across multiple payment processors.

## Core Requirements
1. Unified payment processor contract
2. Support for multiple payment processors (Stripe, Adyen, Razorpay, etc.)
3. Payment lifecycle management operations
4. Multi-language client SDKs
5. Stateless architecture
6. gRPC-based communication

## Project Goals
1. Liberate merchants from single payment processor lock-in
2. Enable seamless switching between payment processors
3. Provide a standardized interface for payment operations
4. Support global payment processor integration through community contributions
5. Ensure scalability and portability

## Project Scope
- gRPC service implementation
- Connector integrations for various payment processors
- Client SDKs in multiple languages
- Payment operation support:
  - Authorization
  - Capture
  - Refund
  - Chargeback
  - Dispute
  - Webhook normalization

## Success Criteria
1. Successful integration with multiple payment processors
2. Seamless switching between processors
3. Comprehensive payment operation support
4. Active community contributions
5. Production-ready stability and reliability 