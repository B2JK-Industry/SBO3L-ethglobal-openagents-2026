# `examples/llamaindex-research-agent`

End-to-end LlamaIndex research agent demo using `sbo3l-llamaindex`'s `FunctionTool` shape. Two tools (`data_fetch` + `sbo3l_payment_request`); routes through KH workflow `m4t4cnpmhv8qquce3bv3c`.

## 3-line setup

```bash
SBO3L_ALLOW_UNAUTHENTICATED=1 cargo run --bin sbo3l-server &
cd examples/llamaindex-research-agent && python3 -m venv .venv && .venv/bin/pip install -e ../../sdks/python -e ../../integrations/llamaindex -e .
.venv/bin/python -m sbo3l_llamaindex_demo.smoke   # no OpenAI / no llama_index needed
```

## With LlamaIndex + an LLM

```bash
.venv/bin/pip install -e ".[llamaindex]"
export OPENAI_API_KEY=sk-...
.venv/bin/python -m sbo3l_llamaindex_demo.agent
```

LlamaIndex `ReActAgent` reasons across both tools, fetches provider metadata via `data_fetch`, then submits an APRP through `sbo3l_payment_request`.

## License

MIT
