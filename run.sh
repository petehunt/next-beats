#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"

runtime_dir="/tmp/next-beats-cafo-postgres"
package_dir="$runtime_dir/node_modules/@embedded-postgres/linux-x64"
if [ ! -x "$package_dir/native/bin/pg_ctl" ]; then
  npm install --prefix "$runtime_dir" embedded-postgres@18.4.0-beta.17
fi

pg_bin="$package_dir/native/bin"
pg_data="$runtime_dir/data"
pg_port="5432"
database_url="postgresql://cafo@127.0.0.1:$pg_port/postgres?sslmode=disable"

if [ ! -f "$pg_data/PG_VERSION" ]; then
  mkdir -p "$pg_data"
  "$pg_bin/initdb" --pgdata="$pg_data" --auth=trust --username=cafo
fi

if ! "$pg_bin/pg_ctl" --pgdata="$pg_data" status >/dev/null 2>&1; then
  "$pg_bin/pg_ctl" --pgdata="$pg_data" --log="$runtime_dir/postgres.log" --options="-p $pg_port -h 127.0.0.1" start
fi

export DATABASE_URL="$database_url"
pnpm prisma.push
pnpm prisma.seed
exec pnpm dev --hostname 0.0.0.0 --port "${PORT:-3000}"
