FROM python:3.11-slim

WORKDIR /app

COPY requirements.txt ./
RUN pip install --no-cache-dir -r requirements.txt

COPY bridge.py ./

ENV PYTHONUNBUFFERED=1
ENV BRIDGE_PORT=9101

CMD ["python", "/app/bridge.py"]
