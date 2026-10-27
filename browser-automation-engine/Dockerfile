# The build context is restricted by .dockerignore; credential/profile files never enter it.
FROM node:22-bookworm-slim AS dependencies
WORKDIR /app
COPY worker/package.json worker/package-lock.json ./
RUN npm ci --ignore-scripts

FROM dependencies AS build
COPY tsconfig.json tsconfig.worker.json ./
COPY src ./src
RUN npm run build

FROM node:22-bookworm-slim AS runtime
WORKDIR /app
ENV PLAYWRIGHT_BROWSERS_PATH=/opt/browsers
COPY --from=dependencies /app/node_modules/playwright ./node_modules/playwright
COPY --from=dependencies /app/node_modules/playwright-core ./node_modules/playwright-core
COPY --from=dependencies /app/node_modules/ipaddr.js ./node_modules/ipaddr.js
RUN node node_modules/playwright/cli.js install --with-deps --only-shell chromium \
    && rm -rf /var/lib/apt/lists/* \
    && chmod -R a+rX /opt/browsers \
    && useradd --uid 10001 --no-create-home browserworker
RUN apt-get update && apt-get install -y --no-install-recommends util-linux tini \
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /app/dist ./dist
USER 10001:10001
# Set NNP after the runtime's AppArmor transition, before any Node/browser code.
ENTRYPOINT ["/usr/bin/setpriv", "--no-new-privs", "/usr/bin/tini", "-g", "--", "node", "/app/dist/cli.js"]
