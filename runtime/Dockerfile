FROM node:24-bookworm-slim
RUN npm install --global --ignore-scripts @openai/codex@0.166.0 && npm cache clean --force
COPY runtime/egress.mjs /opt/workflow-factory/egress.mjs
LABEL org.workflow-factory.runtime="1" org.workflow-factory.codex="0.166.0"
USER node
WORKDIR /workspace
CMD ["node", "-e", "setInterval(() => {}, 3600000)"]
