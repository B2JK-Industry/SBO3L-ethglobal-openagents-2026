FROM python:3.12-slim AS base

WORKDIR /app

COPY sdks/python /workspace/sdks/python
COPY integrations/crewai /workspace/integrations/crewai

RUN pip install --no-cache-dir --upgrade pip && \
    pip install --no-cache-dir \
      "/workspace/sdks/python" \
      "/workspace/integrations/crewai" \
      "fastapi==0.115.*" \
      "uvicorn[standard]==0.32.*" \
      "python-ulid==2.*"

COPY examples/multi-framework-agent/services/execute/app.py /app/app.py

EXPOSE 8002
CMD ["uvicorn", "app:app", "--host", "0.0.0.0", "--port", "8002"]
