#!/bin/sh
set -eu

# Older Compose volumes may still be owned by root from previous releases.
chown -R streamflow:streamflow /app/data
exec gosu streamflow /usr/local/bin/streamflow-backend "$@"
