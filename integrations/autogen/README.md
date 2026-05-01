# `@sbo3l/autogen`

Microsoft AutoGen function adapter wrapping SBO3L. Drop into a `ConversableAgent`'s function registry.

> ⚠ **DRAFT (T-1-4):** depends on F-9 (`@sbo3l/sdk`).

## Install

```bash
npm install @sbo3l/autogen @sbo3l/sdk
```

## Usage

```ts
import { SBO3LClient } from "@sbo3l/sdk";
import { sbo3lFunction } from "@sbo3l/autogen";

const client = new SBO3LClient({ endpoint: "http://localhost:8730" });
const fn = sbo3lFunction({ client });

// Register fn.name + fn.description + fn.parameters with the LLM,
// route fn.name → fn.call.
```

The function descriptor's `parameters` field is the APRP v1 JSON Schema, so the LLM understands the required arguments. On `deny`, the LLM sees `deny_code` and can self-correct or escalate.

## License

MIT
