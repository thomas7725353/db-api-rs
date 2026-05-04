FROM node:16-bookworm AS builder

WORKDIR /app
COPY src/main/webapp/package*.json ./
RUN npm install
COPY src/main/webapp ./
RUN npm run build

FROM nginx:1.27-alpine

COPY src/main/webapp/nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=builder /app/dist /usr/share/nginx/html

EXPOSE 8521
