# Plan service — Python + sbo3l-langchain (LangChain framework boundary).
#
# Build context is the repo root (per docker-compose.yml's `context: ../..`)
# so we can pip install the workspace SDK + integration as editable file
# paths. Resulting image is ~150 MB (slim base + minimal deps).

FROM python:3.12-slim AS base

WORKDIR /app

# Workspace SDK + integration as editable installs.
COPY sdks/python /workspace/sdks/python
COPY integrations/langchain-python /workspace/integrations/langchain-python

RUN pip install --no-cache-dir --upgrade pip && \
    pip install --no-cache-dir \
      "/workspace/sdks/python" \
      "/workspace/integrations/langchain-python" \
      "fastapi==0.115.*" \
      "uvicorn[standard]==0.32.*" \
      "python-ulid==2.*"

COPY examples/multi-framework-agent/services/plan/app.py /app/app.py

EXPOSE 8001
CMD ["uvicorn", "app:app", "--host", "0.0.0.0", "--port", "8001"]
