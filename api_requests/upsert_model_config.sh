#!/bin/bash
export INSTANCE=$1
export MODEL_ID=$2
export FILE_NAME=$(basename $3)
export PROMPT_FILE=$(cat $4)

URL="https://$INSTANCE/admin/upsert_model_config"

BODY=$(jq -n "{ id: env.MODEL_ID, file_name: env.FILE_NAME, prompt_template: env.PROMPT_FILE }")

echo $URL
echo $BODY

curl \
    --insecure \
    --request PUT \
    --data "$BODY" \
    --header "Content-Type: application/json" \
    $URL