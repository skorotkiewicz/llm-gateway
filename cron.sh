#!/bin/bash

# Cron job for LLM Gateway health check
# Add to crontab: */5 * * * * /path/to/cron/cron.sh >> /var/log/llm-gateway-cron.log 2>&1

PROXY_URL="http://localhost:8878/v1/chat/completions"
API_KEY="local"
TIMESTAMP=$(date '+%Y-%m-%d %H:%M:%S')

# Simple ping request
RESPONSE=$(curl -s -X POST \
    -H "Content-Type: application/json" \
    -H "Authorization: Bearer ${API_KEY}" \
    -d '{"model": "default", "messages": [{"role": "user", "content": "ping"}], "max_tokens": 10}' \
    ${PROXY_URL} \
    2>&1)

# Check if response is valid JSON and contains expected fields
if echo "$RESPONSE" | grep -q '"choices"' 2>/dev/null; then
    echo "[$TIMESTAMP] ✓ Gateway alive"
    # Extract first response
    REPLY=$(echo "$RESPONSE" | grep -oP '(?<="content":")[^"]*' | head -1 | cut -c1-50)
    echo "[$TIMESTAMP] Response: $REPLY"
    exit 0
else
    echo "[$TIMESTAMP] ✗ Gateway error or timeout"
    echo "[$TIMESTAMP] Response: $RESPONSE"
    exit 1
fi
