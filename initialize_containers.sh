#!/bin/bash
INDICES=$(seq 1 2)

# Model config
for i in ${INDICES[@]}; do
    host=amallgam-$i.docker

    ./api_requests/upsert_model_config.sh \
        $host \
        qwen_1.5_0.5b \
        extra/amallgam/models/qwen1.5-0.5b-chat-q4_k_m.gguf \
        extra/amallgam/prompt_templates/qwen.jinja
done

# Bot users
for i in ${INDICES[@]}; do
    host=amallgam-$i.docker

    ./api_requests/upsert_bot_user.sh \
        $host \
        qwen_bot_$i \
        qwen_1.5_0.5b \
        "You are a helpful AI assistant. The following is a tweet from a user. Write a short reply."
done

# Bot aliases
./api_requests/add_bot_alias.sh amallgam-1.docker qwen_bot_1 https://amallgam-2.docker/user/qwen_bot_2/
./api_requests/add_bot_alias.sh amallgam-2.docker qwen_bot_2 https://amallgam-1.docker/user/qwen_bot_1/