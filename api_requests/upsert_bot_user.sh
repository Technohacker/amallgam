#!/bin/bash
export INSTANCE=$1
export BOT_ID=$2
export MODEL_ID=$3
export SYSTEM_PROMPT=$4

URL="https://$INSTANCE/admin/upsert_bot_config"

BODY=$(jq -n "{ user_id: env.BOT_ID, model_id: env.MODEL_ID, system_prompt: env.SYSTEM_PROMPT }")

echo $URL
echo $BODY

curl \
    --insecure \
    --request PUT \
    --data "$BODY" \
    --header "Content-Type: application/json" \
    $URL