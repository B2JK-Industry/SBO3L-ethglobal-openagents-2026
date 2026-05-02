"""Two callables the LangChain Python agent reasons across:

  1. fetch_url — GET a JSON URL (research before paying)
  2. sbo3l_pay (built via sbo3l_langchain.sbo3l_tool) — APRP submit through
     SBO3L's policy boundary, routed to KeeperHub workflow
     m4t4cnpmhv8qquce3bv3c when allowed.
"""

from __future__ import annotations

import json
import os
from typing import Any

import httpx
from sbo3l_langchain import sbo3l_tool
from sbo3l_sdk import SBO3LClientSync, bearer

#: Live KeeperHub workflow id verified end-to-end on 2026-04-30.
KH_WORKFLOW_ID = "m4t4cnpmhv8qquce3bv3c"


def fetch_url(url: str) -> str:
    """Fetch a JSON URL and return the body as a JSON string.

    Used as the agent's `data_fetch` tool — inspect a provider before paying.
    """

    try:
        with httpx.Client(timeout=10.0) as client:
            r = client.get(url, headers={"Accept": "application/json"})
        return json.dumps({"status": r.status_code, "body": r.text[:2000]})
    except httpx.HTTPError as e:
        return json.dumps({"error": "fetch failed", "detail": str(e)})


def default_client() -> SBO3LClientSync:
    """Build an SBO3L sync client from env (`SBO3L_ENDPOINT`, `SBO3L_BEARER_TOKEN`)."""

    endpoint = os.environ.get("SBO3L_ENDPOINT", "http://localhost:8730")
    bearer_token = os.environ.get("SBO3L_BEARER_TOKEN")
    auth = bearer(bearer_token) if bearer_token else None
    return SBO3LClientSync(endpoint, auth=auth)


def build_sbo3l_pay_func(client: SBO3LClientSync) -> Any:
    """Return the descriptor's `func(input_str: str) -> str` — wire into a LangChain tool."""

    return sbo3l_tool(client=client).func
