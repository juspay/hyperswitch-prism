# Modes of Usage

Prism can be used in two modes: **Library (SDK)** or **Microservice (gRPC)**.

## Library Mode (Recommended)

In this scenario, Prism SDK is directly embedded into your application. This is the recommended mode for most use cases, because it is,

- Simple to integrate and deploy within your existing server and cloud resources
- No additional network hops
- Full type safety at compile time

## Microservice Mode

You may deploy Prism as a standalone service only if you need service isolation due to 

- **Independent deployments** - Being able to update Prism without redeploying your main application
- **Fast release cycles** - Decoupling the SDK released from your app server releases, and you wish to update PRism more frequently than you application

Choose this mode only if you need isolation for fast and independent deployments, because this mode comples with additional complxity to handle - such as deployment, privisioning of resources, uptime and monitoring.
