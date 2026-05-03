"""observability — emit OpenTelemetry spans alongside SBO3L receipts.

Each tool invocation creates an OTel span carrying SBO3L's
`audit_event_id` + `kh_execution_ref` as attributes. An operator's
trace backend (Jaeger, Tempo, Honeycomb, …) then shows the request
flow with the SBO3L decision + KH execution as searchable, indexed
metadata. Bridges policy decisions into existing tracing infra
without inventing a new sink.

Install (one-time):
    pip install opentelemetry-api opentelemetry-sdk \\
                opentelemetry-exporter-otlp-proto-http

Run:
    OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4318 \\
      python observability.py

Without OTEL_EXPORTER_OTLP_ENDPOINT set, the demo falls back to
ConsoleSpanExporter so you see the spans printed inline.

Expected: ALLOW envelope + a span on stdout (or in your trace backend)
showing audit_event_id + kh_execution_ref attributes.
"""

from __future__ import annotations

import json
import os
import sys
import uuid
from datetime import datetime, timedelta, timezone

from sbo3l_langchain_keeperhub import sbo3l_keeperhub_tool
from sbo3l_sdk import SBO3LClientSync

# OpenTelemetry — gracefully degrade if not installed so the demo
# stays runnable without the otel deps.
try:
    from opentelemetry import trace
    from opentelemetry.sdk.resources import Resource
    from opentelemetry.sdk.trace import TracerProvider
    from opentelemetry.sdk.trace.export import (
        BatchSpanProcessor,
        ConsoleSpanExporter,
    )

    OTEL = True
except ImportError:
    OTEL = False
    print("⚠ opentelemetry packages not installed — see install note in docstring.")
    print("  Demo will continue without span emission.")


def setup_otel() -> "trace.Tracer | None":
    if not OTEL:
        return None
    provider = TracerProvider(
        resource=Resource.create({"service.name": "sbo3l-keeperhub-demo"})
    )
    # OTLP exporter when endpoint set; console fallback otherwise.
    if os.environ.get("OTEL_EXPORTER_OTLP_ENDPOINT"):
        from opentelemetry.exporter.otlp.proto.http.trace_exporter import OTLPSpanExporter
        provider.add_span_processor(BatchSpanProcessor(OTLPSpanExporter()))
        print(f"▶ otel: exporting to {os.environ['OTEL_EXPORTER_OTLP_ENDPOINT']}")
    else:
        provider.add_span_processor(BatchSpanProcessor(ConsoleSpanExporter()))
        print("▶ otel: console exporter (set OTEL_EXPORTER_OTLP_ENDPOINT to send to a backend)")
    trace.set_tracer_provider(provider)
    return trace.get_tracer("sbo3l-keeperhub-demo")


def aprp() -> dict:
    return {
        "agent_id": "observable-agent-01",
        "task_id": f"obs-{uuid.uuid4().hex[:8]}",
        "intent": "purchase_api_call",
        "amount": {"value": "0.05", "currency": "USD"},
        "token": "USDC",
        "destination": {
            "type": "x402_endpoint",
            "url": "https://api.example.com/v1/inference",
            "method": "POST",
            "expected_recipient": "0x1111111111111111111111111111111111111111",
        },
        "payment_protocol": "x402",
        "chain": "base",
        "provider_url": "https://api.example.com",
        "expiry": (datetime.now(timezone.utc) + timedelta(minutes=5)).isoformat(),
        "nonce": str(uuid.uuid4()),
        "risk_class": "low",
    }


def call_with_span(tracer, descriptor, body: dict) -> dict:
    if tracer is None:
        return json.loads(descriptor.func(json.dumps(body)))
    with tracer.start_as_current_span("sbo3l.keeperhub.submit") as span:
        span.set_attribute("sbo3l.agent_id", body["agent_id"])
        span.set_attribute("sbo3l.intent", body["intent"])
        span.set_attribute("sbo3l.amount_usd", body["amount"]["value"])
        envelope = json.loads(descriptor.func(json.dumps(body)))
        # Surface SBO3L's decision + KH execution_ref as searchable
        # span attributes — what most trace backends index on.
        span.set_attribute("sbo3l.decision", envelope.get("decision") or "?")
        if envelope.get("audit_event_id"):
            span.set_attribute("sbo3l.audit_event_id", envelope["audit_event_id"])
        if envelope.get("kh_execution_ref"):
            span.set_attribute("kh.execution_ref", envelope["kh_execution_ref"])
        if envelope.get("deny_code"):
            span.set_attribute("sbo3l.deny_code", envelope["deny_code"])
        return envelope


def main() -> int:
    endpoint = os.environ.get("SBO3L_ENDPOINT", "http://localhost:8730")
    print(f"▶ daemon: {endpoint}")
    tracer = setup_otel()

    with SBO3LClientSync(endpoint) as client:
        descriptor = sbo3l_keeperhub_tool(client=client)
        envelope = call_with_span(tracer, descriptor, aprp())

    print("\n=== envelope ===")
    for k, v in envelope.items():
        print(f"  {k}: {json.dumps(v)}")
    if envelope.get("decision") == "allow":
        print(f"\n✓ allow + KH executed → kh_execution_ref={envelope.get('kh_execution_ref')}")
        return 0
    if "error" in envelope:
        print(f"\n✗ transport error: {envelope['error']}")
        return 2
    print(f"\n? unexpected: {envelope.get('decision')}")
    return 1


if __name__ == "__main__":
    sys.exit(main())
