#!/usr/bin/env bash
# Copies the website (site/) into a checkout of the Pages repository and stamps the stylesheet and
# script links with the commit, so browsers never mix a new page with a cached old styles.css, and
# writes sitemap.xml. Same as Joystick Gobloon's.
# Usage: publish-site.sh <pages checkout> [commit]
set -euo pipefail
out=$1
rev=${2:-$(git rev-parse --short HEAD)}
find "$out" -mindepth 1 -maxdepth 1 ! -name .git -exec rm -rf {} +
cp -r site/. "$out/"
for page in "$out"/*.html; do
  sed -i -E "s#(href|src)=\"(/keyboard-gobloon/)?(styles\.css|keyboard\.css|main\.js)\"#\1=\"\2\3?v=$rev\"#g" "$page"
done

# Sitemap for search engines: every page except the 404, dated by its last commit.
base=https://h0userus.github.io/keyboard-gobloon/
{
  echo '<?xml version="1.0" encoding="UTF-8"?>'
  echo '<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">'
  for page in index.html privacy.html; do
    [ -f "site/$page" ] || continue
    date=$(git log -1 --format=%cs -- "site/$page")
    loc=$base$page
    [ "$page" = index.html ] && loc=$base
    echo "  <url><loc>$loc</loc><lastmod>${date:-$(date +%F)}</lastmod></url>"
  done
  echo '</urlset>'
} > "$out/sitemap.xml"
