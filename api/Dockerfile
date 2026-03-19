FROM golang:1.25-alpine AS builder

WORKDIR /app
COPY go.mod go.sum* ./
RUN go mod download 2>/dev/null || true
COPY . .
RUN CGO_ENABLED=0 go build -o /catch-pokemon-api ./cmd/main.go

FROM alpine:3.19
RUN apk add --no-cache ca-certificates
COPY --from=builder /catch-pokemon-api /usr/local/bin/catch-pokemon-api

EXPOSE 8080
CMD ["catch-pokemon-api"]
