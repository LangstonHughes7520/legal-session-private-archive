#!/usr/bin/env sh
set -eu

curl --fail-with-body --request POST http://127.0.0.1:3000/matters/intake \
  --header 'content-type: application/json' \
  --data '{"matter_id":"ACME-2026-014","participant_id":"client-7","participant_name":"Morgan Lee","signed_document_delivered":false}'

