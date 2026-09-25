# syntax=docker/dockerfile:1
#
# Development image for the canonical frontend (see
# docs/adr-0001-frontend-canonical-directory.md). Runs the Vite dev server so
# the full stack in docker-compose.yml comes up with one command.

FROM node:20-alpine

WORKDIR /app

COPY package.json package-lock.json ./
RUN npm ci

COPY . .

# node:20-alpine ships an unprivileged `node` user.
RUN chown -R node:node /app
USER node

EXPOSE 5173

# busybox wget is available on alpine; no need to install curl.
HEALTHCHECK --interval=10s --timeout=5s --start-period=20s --retries=5 \
  CMD wget -q --spider http://127.0.0.1:5173/ || exit 1

CMD ["npx", "vite", "--host", "0.0.0.0", "--port", "5173", "--strictPort"]
