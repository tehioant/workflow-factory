FROM node:24-bookworm-slim
RUN npm install --global --ignore-scripts @openai/codex@0.166.0 && npm cache clean --force
COPY runtime/egress.mjs /opt/workflow-factory/egress.mjs
COPY runtime/keeper.mjs runtime/keeper-protocol.mjs /opt/workflow-factory/
LABEL org.workflow-factory.runtime="1" org.workflow-factory.codex="0.166.0"
USER node
WORKDIR /workspace
CMD ["node", "/opt/workflow-factory/keeper.mjs"]
