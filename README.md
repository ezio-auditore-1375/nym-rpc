# Nym RPC

A privacy-preserving RPC proxy that routes RPC requests through the [Nym mixnet](https://nym.com/) to protect metadata and enhance privacy for blockchain interactions.

> ⚠️ **Note**: This project is currently under active development and not yet ready for production use. See [TODO](#todo) section below for planned features and improvements.

## Overview

Nym RPC enables anonymous and private access to blockchain RPC endpoints by routing requests through the Nym mixnet. This protects users' IP addresses, request patterns, and other metadata from being exposed to RPC providers.

The system uses **full TLS encryption** end-to-end, ensuring that not even the final Nym exit node can see the content of your RPC requests - only the destination server can decrypt and read the actual request data.

### How it works

```
Client App → HTTP Proxy (localhost:8545) → TCP Proxy Client → Nym Mixnet → TCP Proxy Server → RPC Provider
```

1. **Client**: Runs a local HTTP proxy server that accepts standard RPC requests
2. **TCP Proxy Client**: Inserts UPSTREAM packet information and sends through the Nym mixnet
3. **Mixnet**: Routes requests through the Nym mixnet for privacy
4. **TCP Proxy Server**: Receives requests from mixnet, extracts UPSTREAM packet info, and forwards to target RPC provider

See [architecture diagram](./docs/architecture_diagram.mmd) for visual representation of the whole flow.

## Features

- 🔒 **Privacy-first**: All requests routed through Nym mixnet
- 🚀 **Drop-in replacement**: Compatible with existing RPC clients
- ⚡ **Connection pooling**: Maintains NYM client pools for optimal performance
- 🌐 **Multi-provider support**: Works with any JSON-RPC endpoint
- 🛠️ **Easy configuration**: Simple CLI interface

## TODO

Major tasks before the project is turned in.

- announce and discover RPC server
- detect misbehaving nodes

Minor

- disable cover traffic
- allowlist
- memory: prune sessions

## Installation

TODO

## Usage

TODO
