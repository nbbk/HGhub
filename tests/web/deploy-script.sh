#!/usr/bin/env bash
set -euo pipefail
ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
mkdir -p "$tmp/repo/scripts" "$tmp/bin"
cp "$ROOT/scripts/web-install.sh" "$tmp/repo/scripts/"
cat > "$tmp/bin/docker" <<'MOCK'
#!/usr/bin/env bash
if [[ $* == 'compose version' || $* == info ]]; then exit 0; fi
printf '%s\n' "$*" >> "$MOCK_LOG"
MOCK
chmod +x "$tmp/bin/docker"
export PATH="$tmp/bin:$PATH" MOCK_LOG="$tmp/docker.log"
run() { bash "$tmp/repo/scripts/web-install.sh" "$@"; }
run --mode local --check >/dev/null
run --mode caddy --origin https://video.example.com --check >/dev/null
run --mode proxy --origin https://video.example.com --check >/dev/null
[[ ! -e $tmp/repo/.hghub-web ]]
for origin in http://video.example.com https://video.example.com/a 'https://video.example.com?x=1' https://a:b@video.example.com https://video.example.com:443 https://a..example.com https://127.0.0.1 'https://$secret.example.com'; do
  if run --mode proxy --origin "$origin" --check >/dev/null 2>&1; then echo "Accepted invalid origin: $origin"; exit 1; fi
done
if run --mode proxy --check >/dev/null 2>&1; then exit 1; fi
if run --mode local --origin http://0.0.0.0:8080 --check >/dev/null 2>&1; then exit 1; fi
run --mode caddy --origin https://video.example.com > "$tmp/output"
state=$tmp/repo/.hghub-web
[[ $(stat -c '%a' "$state") == 700 ]]
[[ $(stat -c '%a' "$state/access-password") == 444 ]]
[[ $(wc -c < "$state/access-password") == 49 ]]
password=$(cat "$state/access-password")
! grep -F "$password" "$tmp/output" "$MOCK_LOG" "$state/config.env"
grep -q '^HGHUB_WEB_ORIGIN=https://video.example.com$' "$state/config.env"
grep -q 'compose -f compose.web.yml --profile https up -d --build' "$MOCK_LOG"
if run --mode caddy --origin https://video.example.com >/dev/null 2>&1; then exit 1; fi
[[ $(cat "$state/access-password") == "$password" ]]
mkdir -p "$tmp/other/scripts" "$tmp/target"
cp "$ROOT/scripts/web-install.sh" "$tmp/other/scripts/"
ln -s "$tmp/target" "$tmp/other/.hghub-web"
if bash "$tmp/other/scripts/web-install.sh" --mode local >/dev/null 2>&1; then exit 1; fi
[[ ! -e $tmp/target/access-password ]]
echo 'Deployment installer tests passed (mock Docker; no real deployment).'
