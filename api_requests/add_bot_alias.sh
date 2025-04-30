#!/bin/bash
export INSTANCE=$1
export BOT_ID=$2
export ALIAS_ID=$3

URL="https://$INSTANCE/admin/add_bot_alias"

BODY=$(jq -n "{ user_id: env.BOT_ID, alias_id: env.ALIAS_ID }")

echo $URL
echo $BODY

curl \
    --insecure \
    --request PUT \
    --data "$BODY" \
    --header "Content-Type: application/json" \
    $URL