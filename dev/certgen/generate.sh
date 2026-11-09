#!/bin/sh
# One-shot certificate generator for the development database (idempotent).
# Creates a private CA and a server certificate valid for localhost, db and 127.0.0.1.
set -eu
cd /certs

if [ -f server.crt ] && openssl x509 -in server.crt -noout -checkend 2592000 >/dev/null 2>&1; then
  echo "certificates present and valid for more than 30 days; nothing to do"
  exit 0
fi

echo "generating development CA and server certificate"
openssl ecparam -name prime256v1 -genkey -noout -out ca.key
openssl req -x509 -new -key ca.key -sha256 -days 3650 -subj "/CN=Tandem development CA" -out ca.crt

openssl ecparam -name prime256v1 -genkey -noout -out server.key
openssl req -new -key server.key -subj "/CN=localhost" -out server.csr
printf 'subjectAltName=DNS:localhost,DNS:db,IP:127.0.0.1\nextendedKeyUsage=serverAuth\n' > server.ext
openssl x509 -req -in server.csr -CA ca.crt -CAkey ca.key -CAcreateserial \
  -sha256 -days 365 -extfile server.ext -out server.crt
rm -f server.csr server.ext ca.srl

# PostgreSQL runs as uid 999 and refuses a key that others can read.
chown 999:999 server.key
chmod 600 server.key
chmod 644 ca.crt server.crt
chmod 600 ca.key
echo "done"
