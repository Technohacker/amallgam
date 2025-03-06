#!/bin/bash
docker compose exec mastodon tootctl accounts create  mastodon_user --email mastodon_user@smtp4dev
docker compose exec mastodon tootctl accounts approve mastodon_user