#!/bin/zsh
# usage: run.sh [capture-only device...]   Called by `make store-shots`, which passes the .env ports.
set -euo pipefail
SHOTS=${0:A:h}
ROOT=${SHOTS:h:h:h}
RAW=$SHOTS/raw
DB=myra_shots
U=00000000-0000-0000-0000-000000000000
API=http://localhost:$SERVER_PORT/api/users/$U
export PGPASSWORD=$POSTGRES_PASSWORD PGTZ=$(readlink /etc/localtime | sed 's#.*/zoneinfo/##')
cd $ROOT
mkdir -p $RAW

sql(){ psql -h localhost -p $POSTGRES_PORT -U $POSTGRES_USER -v ON_ERROR_STOP=1 -q "$@"; }

backend(){
  lsof -ti :$SERVER_PORT | xargs kill -9 2>/dev/null || true
  nohup make backend-run POSTGRES_DB=$1 > $RAW/backend-$1.log 2>&1 < /dev/null &!
  for i in {1..300}; do
    [ "$(curl -s -o /dev/null -w '%{http_code}' $API/accounts)" != 000 ] && return
    sleep 2
  done
  echo "backend on $1 did not start, see $RAW/backend-$1.log"; return 1
}

restore(){
  echo "Pointing the backend back at $POSTGRES_DB"
  backend $POSTGRES_DB
}

bun install --cwd $SHOTS --frozen-lockfile
(cd $SHOTS && bunx playwright install chromium)

sql -d postgres -c "DROP DATABASE IF EXISTS $DB WITH (FORCE)" -c "CREATE DATABASE $DB"
trap restore EXIT
trap 'exit 130' INT TERM
backend $DB
sql -d $DB -f database/demo/showcase_data.sql
sql -d $DB -f $SHOTS/seed.sql
curl -sf -XPOST $API/onboarding -H 'content-type: application/json' -d '{"version":2}' > /dev/null
curl -sf -XPOST $API/ai-features -H 'content-type: application/json' -d '{"enabled":true}' > /dev/null
SESSION=$(curl -sf $API/import-sessions | python3 -c 'import sys,json; print(json.load(sys.stdin)[0]["id"])')
AMEND=$(curl -sf $API/import-sessions/$SESSION/items | python3 -c '
import sys, json
for row in json.load(sys.stdin)["items"]:
    if row["near_miss_candidates"]:
        print(json.dumps({"item_ids": [row["id"]], "verb": "skip", "candidate_id": row["near_miss_candidates"][0]["id"]}))
        break')
[ -n "$AMEND" ] && curl -sf -XPOST $API/import-sessions/$SESSION/items/verb -H 'content-type: application/json' -d "$AMEND" > /dev/null

bun $SHOTS/capture.ts "$@"
restore
trap - EXIT
bun $SHOTS/compose.ts
