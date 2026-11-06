ARG UCS_IMAGE=juspaydotin/hyperswitch-prism@sha256:56bbd606a8c75d4014eba6e3bbc88a8ba0433aca29949361979037e61dccb367
FROM ghcr.io/grpc-ecosystem/grpc-health-probe:v0.4.38@sha256:cacececcc3e94fde691141b80c34e288bc2743323d4f450274a0c636d07400a9 AS probe
FROM ${UCS_IMAGE}
COPY --from=probe /ko-app/grpc-health-probe /bin/grpc_health_probe
