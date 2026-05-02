# SBO3L gRPC TypeScript example

Quickstart that connects to an SBO3L gRPC daemon, submits one
`Decide` RPC, and streams the audit chain.

## Run

1. Start the daemon (in one terminal):

   ```bash
   cargo run -p sbo3l-server --features grpc --bin sbo3l-server-grpc
   ```

   This binds REST on `127.0.0.1:8730` and gRPC on `127.0.0.1:8731`.

2. Run the example (in another terminal):

   ```bash
   cd examples/grpc-ts
   npm install
   npm start
   ```

## Expected output

```
[grpc-ts] connecting to 127.0.0.1:8731
[health] {"status":"ok","version":"1.2.0", ...}
[decide] {"status":"PAYMENT_STATUS_AUTO_APPROVED","decision":"DECISION_ALLOW", ...}
[chain] 1 policy_decided <hash>...
[chain] 1 event(s) emitted
```

## Configuration

| Env var           | Default            | Description                |
| ----------------- | ------------------ | -------------------------- |
| `SBO3L_GRPC_ADDR` | `127.0.0.1:8731`   | gRPC `host:port` to dial.  |
