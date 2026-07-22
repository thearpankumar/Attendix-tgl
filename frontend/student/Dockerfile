# Stage 1: Build
FROM node:24-slim AS build
WORKDIR /app
COPY package*.json ./
COPY scripts/ ./scripts/

# Use Docker BuildKit cache mounts to speed up dependency installation
RUN --mount=type=cache,target=/root/.npm \
    npm ci

COPY . .
ARG VITE_DEV_BYPASS_ALL
ENV VITE_DEV_BYPASS_ALL=$VITE_DEV_BYPASS_ALL
RUN npm run build

# Stage 2: Runtime
# Using alpine-slim for the absolute smallest Nginx footprint
FROM nginx:alpine-slim

COPY --from=build /app/dist /usr/share/nginx/html
COPY nginx.conf /etc/nginx/conf.d/default.conf

EXPOSE 80
CMD ["nginx", "-g", "daemon off;"]
