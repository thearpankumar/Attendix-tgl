# Stage 1: Build
FROM node:24-slim AS builder
ARG VITE_BASE_PATH=/
ENV VITE_BASE_PATH=$VITE_BASE_PATH

WORKDIR /app
COPY package*.json ./

# Use Docker BuildKit cache mounts to speed up dependency installation
RUN --mount=type=cache,target=/root/.npm \
    npm ci

COPY . .
RUN npm run build

# Stage 2: Runtime
# Using alpine-slim for the absolute smallest Nginx footprint
FROM nginx:alpine-slim

COPY --from=builder /app/dist /usr/share/nginx/html
COPY nginx.conf /etc/nginx/conf.d/default.conf

EXPOSE 80
CMD ["nginx", "-g", "daemon off;"]
