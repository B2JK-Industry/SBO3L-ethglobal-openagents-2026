"""Two callables for the LlamaIndex agent: data_fetch + sbo3l_pay."""

from __future__ import annotations

import json
import os
from typing import Any

import httpx
from sbo3l_llamaindex import sbo3l_tool
from sbo3l_sdk import SBO3LClientSync, bearer

KH_WORKFLOW_ID = "m4t4cnpmhv8qquce3bv3c"


def fetch_url(url: str) -> str:
    """GET a JSON URL — return body as JSON string."""

    try:
        with httpx.Client(timeout=10.0) as client:
            r = client.get(url, headers={"Accept": "application/json"})
        return json.dumps({"status": r.status_code, "body": r.text[:2000]})
    except httpx.HTTPError as e:
        return json.dumps({"error": "fetch failed", "detail": str(e)})


def default_client() -> SBO3LClientSync:
    endpoint = os.environ.get("SBO3L_ENDPOINT", "http://localhost:8730")
    bearer_token = os.environ.get("SBO3L_BEARER_TOKEN")
    auth = bearer(bearer_token) if bearer_token else None
    return SBO3LClientSync(endpoint, auth=auth)


def build_sbo3l_pay_func(client: SBO3LClientSync) -> Any:
    return sbo3l_tool(client=client).func
