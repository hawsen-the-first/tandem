#!/bin/sh
# Runs once, on first start of an empty data directory.
# Creates the two application roles (NFR design 5.4); table grants are made by the migrations.
set -eu

psql -v ON_ERROR_STOP=1 \
     --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" \
     --set=migrate_password="$TANDEM_MIGRATE_PASSWORD" \
     --set=app_password="$TANDEM_APP_PASSWORD" <<'SQL'
CREATE ROLE tandem_migrate LOGIN PASSWORD :'migrate_password';
CREATE ROLE tandem_app LOGIN PASSWORD :'app_password';

REVOKE ALL ON DATABASE tandem FROM PUBLIC;
GRANT CONNECT ON DATABASE tandem TO tandem_migrate, tandem_app;

CREATE SCHEMA tandem AUTHORIZATION tandem_migrate;
GRANT USAGE ON SCHEMA tandem TO tandem_app;
ALTER ROLE tandem_migrate SET search_path = tandem;
ALTER ROLE tandem_app SET search_path = tandem;
SQL
