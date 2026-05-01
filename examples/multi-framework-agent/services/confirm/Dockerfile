# Confirm service — Node + @sbo3l/autogen (AutoGen framework boundary).
#
# Build context is repo root. We pre-build the @sbo3l/autogen integration
# at /workspace and let the service's package.json reference it via
# `file:/workspace/integrations/autogen`.

FROM node:20-slim AS base

WORKDIR /workspace

# Build the @sbo3l/autogen integration first (so its dist/ is ready).
COPY integrations/autogen /workspace/integrations/autogen
RUN cd /workspace/integrations/autogen && \
    npm install --no-audit --no-fund --silent && \
    npm run build

WORKDIR /app
COPY examples/multi-framework-agent/services/confirm/package.json /app/
COPY examples/multi-framework-agent/services/confirm/tsconfig.json /app/
COPY examples/multi-framework-agent/services/confirm/server.ts /app/

RUN npm install --no-audit --no-fund --silent

EXPOSE 8003
CMD ["npx", "tsx", "server.ts"]
