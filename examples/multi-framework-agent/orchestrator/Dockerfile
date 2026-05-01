FROM python:3.12-slim AS base

WORKDIR /app

RUN pip install --no-cache-dir --upgrade pip && \
    pip install --no-cache-dir "httpx==0.27.*"

COPY examples/multi-framework-agent/orchestrator/run.py /app/run.py

CMD ["python", "/app/run.py"]
