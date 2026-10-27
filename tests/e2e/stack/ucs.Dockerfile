ARG UCS_IMAGE=juspaydotin/hyperswitch-prism:latest
FROM ghcr.io/grpc-ecosystem/grpc-health-probe:v0.4.38 AS probe
FROM ${UCS_IMAGE}
COPY --from=probe /ko-app/grpc-health-probe /bin/grpc_health_probe
