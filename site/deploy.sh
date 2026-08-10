#!/usr/bin/env bash
# Push the site to proengineer.app.
#
# The github.io copy needs nothing: pushing to main deploys it. This is the
# other half — the domain is served from a host of its own, so that copy is
# uploaded rather than published.
#
#   ./site/deploy.sh              # to the default host
#   ./site/deploy.sh myhost       # to another ssh alias
#
# No key lives in this repository and none should. The host is an entry in
# ~/.ssh/config with its key in ~/.ssh, which is the only place a private key
# belongs — a copy under a project directory is one `git add -f` away from
# being published, and `.gitignore` is a convention rather than a guard.
set -euo pipefail

host="${1:-mvps1}"
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
target="/var/www/proengineer.app"

staging="$(mktemp -d)"
trap 'rm -rf "$staging"' EXIT

cp -r "$root/site/." "$staging/"
cp -r "$root/screenshots" "$staging/screenshots"
# Neither belongs on a web server: one documents the site, the other is this.
rm -f "$staging/README.md" "$staging/deploy.sh"

# Same substitution the Pages workflow makes, from the same source, so the two
# copies never disagree about when the page last changed.
sed -i "s|LASTMOD|$(git -C "$root" log -1 --format=%cs)|" "$staging/sitemap.xml"
if grep -q LASTMOD "$staging/sitemap.xml"; then
  echo "sitemap.xml still has its placeholder; refusing to deploy" >&2
  exit 1
fi

# Every picture the page names has to exist, embedded or linked — the same
# check the workflow runs, because a broken image is not worth finding later.
missing=0
while read -r ref; do
  [ -f "$staging/$ref" ] || { echo "index.html references missing $ref" >&2; missing=1; }
done < <(grep -oE '(src|href)="screenshots/[^"]*"' "$staging/index.html" | cut -d'"' -f2 | sort -u)
[ "$missing" -eq 0 ] || exit 1

echo "Uploading $(du -sh "$staging" | cut -f1) to $host:$target"
# tar over ssh rather than rsync: the host has no rsync, and asking a server to
# grow a dependency for a 3 MB upload is the wrong trade.
tar -C "$staging" -czf - . | ssh "$host" "
  set -e
  mkdir -p '$target'
  rm -rf '$target'/*
  tar -C '$target' -xzf -
  chown -R www-data:www-data '$target'
"

echo "Checking what is actually being served"
for path in / /robots.txt /sitemap.xml /screenshots/Dashboard.png; do
  code=$(curl -s -o /dev/null -m 15 -w '%{http_code}' "https://proengineer.app$path")
  printf '  %-28s %s\n' "$path" "$code"
  [ "$code" = "200" ] || { echo "  ^ expected 200" >&2; exit 1; }
done
echo "Done: https://proengineer.app"
