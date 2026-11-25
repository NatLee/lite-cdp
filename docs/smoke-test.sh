#!/usr/bin/env bash
# 對一個剛啟動、資料庫是空的 Lite CDP 跑一輪：建立管理員、建網站、送事件、認人、標籤、分群、匯出。
# 用法：bash docs/smoke-test.sh http://127.0.0.1:8585
set -euo pipefail
base=${1:-http://127.0.0.1:8585}
jar=$(mktemp)
# 每次跑都用新的隨機密碼，不放任何固定密碼在 repo 裡
pw=$(openssl rand -hex 16)
json=(-H 'content-type: application/json')
status() { curl -s -o /dev/null -w '%{http_code}' "$@"; }

for _ in $(seq 1 30); do
  curl -fsS "$base/api/health" >/dev/null 2>&1 && break
  sleep 1
done
curl -fsS "$base/api/meta" | grep -q '"retention_days":0'
curl -fsS "$base/" | grep -q '<div id="root">'
curl -fsS "$base/s/1/people" | grep -q '<div id="root">'
curl -fsS "$base/t.js" | grep -q 'Lite CDP tracker'

# 還沒有帳號：要先建立管理員；沒登入的 API 回 401
curl -fsS "$base/api/auth/state" | grep -q '"setup_required":true'
test "$(status "$base/api/sites")" = 401
curl -fsS -c "$jar" -D "$jar.headers" "${json[@]}" \
  -d "{\"username\":\"admin\",\"password\":\"$pw\"}" "$base/api/auth/setup" >/dev/null
grep -qi 'set-cookie: cdp_session=.*HttpOnly' "$jar.headers"
# 只能建立一次
code=$(status "${json[@]}" -d "{\"username\":\"other\",\"password\":\"$pw\"}" "$base/api/auth/setup")
test "$code" = 409

# 建一個網站，只收 www.example.com 送來的資料
site=$(curl -fsS -b "$jar" "${json[@]}" \
  -d '{"name":"官方網站","url":"https://www.example.com","allowed_origins":["https://www.example.com"]}' "$base/api/sites")
key=$(printf '%s' "$site" | sed -n 's/.*"key":"\([^"]*\)".*/\1/p')
test -n "$key"

# 追蹤程式送資料用 text/plain（不會觸發 CORS 預檢）
collect() { status -H 'content-type: text/plain' -H "Origin: ${2:-https://www.example.com}" -d "$1" "$base/collect"; }
view() { printf '{"type":"pageview","url":"https://www.example.com%s","title":"%s","referrer":"%s"}' "$1" "$2" "${3:-}"; }
# batch <金鑰> <匿名編號> <事件的 JSON 陣列> [identify 的 JSON]
batch() { printf '{"key":"%s","anon_id":"%s","session_id":"s-%s","events":%s,"identify":%s}' "$1" "$2" "$2" "$3" "${4:-null}"; }
a=aaaaaaaaaaaaaaaaaaaaaaaa
b=bbbbbbbbbbbbbbbbbbbbbbbb
home=$(view / 首頁)

# 先建標籤，之後進來的事件會自動貼上
curl -fsS -b "$jar" "${json[@]}" \
  -d '{"name":"看過定價","color":"amber","rules":[{"kind":"path_prefix","value":"/pricing"}]}' \
  "$base/api/sites/1/labels" | grep -q '"name":"看過定價"'

# 訪客 A：從搜尋引擎來，看了首頁和定價，加入購物車
cart='{"type":"custom","name":"add_to_cart","url":"https://www.example.com/pricing","props":{"sku":"A-1"}}'
test "$(collect "$(batch "$key" $a "[$(view / 首頁 https://www.google.com/),$(view /pricing 定價),$cart]")")" = 204
# 訪客 B：只看了首頁
test "$(collect "$(batch "$key" $b "[$home]")")" = 204
# 別的網站送來的、金鑰不對的、不是 JSON 的
test "$(collect "$(batch "$key" cccccccccccccccccccccccc "[$home]")" https://evil.example)" = 403
test "$(collect "$(batch nosuchkey0000000 cccccccccccccccccccccccc "[$home]")")" = 404
test "$(collect 'not json')" = 400

# 兩個都還是匿名訪客
curl -fsS -b "$jar" "$base/api/sites/1/people" | grep -q '"total":2'
curl -fsS -b "$jar" "$base/api/sites/1/people?kind=member" | grep -q '"total":0'
# A 登入了：知道是誰，之前匿名的紀錄還在
test "$(collect "$(batch "$key" $a '[]' '{"email":"Ming@Example.org","name":"王小明","traits":{"plan":"pro"}}')")" = 204
curl -fsS -b "$jar" "$base/api/sites/1/people?kind=member" | grep -q '"email":"ming@example.org"'
# 換了一台裝置登入同一個 email：併成同一個人
test "$(collect "$(batch "$key" dddddddddddddddddddddddd "[$(view /docs 文件)]" '{"email":"ming@example.org"}')")" = 204
curl -fsS -b "$jar" "$base/api/sites/1/people" | grep -q '"total":2'

# 標籤、搜尋、時間軸
curl -fsS -b "$jar" "$base/api/sites/1/people?label_id=1" | grep -q '"total":1'
curl -fsS -b "$jar" "$base/api/sites/1/people?q=ming" | grep -q '"name":"王小明"'
pid=$(curl -fsS -b "$jar" "$base/api/sites/1/people?kind=member" | sed -n 's/.*"items":\[{"id":\([0-9]*\).*/\1/p')
curl -fsS -b "$jar" "$base/api/sites/1/people/$pid" | grep -q '"plan":"pro"'
curl -fsS -b "$jar" "$base/api/sites/1/people/$pid/events?type=custom" | grep -q '"name":"add_to_cart"'
curl -fsS -b "$jar" "$base/api/sites/1/pages" | grep -q '"path":"/pricing"'
curl -fsS -b "$jar" "$base/api/sites/1/overview?tz_offset=480" | grep -q '"people":2'

# 分群：有「看過定價」的會員
rules='{"must":[{"field":"label","value":1}],"should":[{"field":"referrer","value":"google"}],"must_not":[]}'
curl -fsS -b "$jar" "${json[@]}" -d "{\"kind\":\"member\",\"rules\":$rules}" \
  "$base/api/sites/1/segments/preview" | grep -q '"count":1'
curl -fsS -b "$jar" "${json[@]}" -d "{\"name\":\"看過定價的會員\",\"kind\":\"member\",\"rules\":$rules}" \
  "$base/api/sites/1/segments" | grep -q '"count":1'
curl -fsS -b "$jar" "$base/api/sites/1/people?segment_id=1" | grep -q '"total":1'
test "$(curl -fsS -b "$jar" "$base/api/sites/1/people/export?segment_id=1" | wc -l)" -ge 2
# 分群還在用的標籤刪不掉
test "$(status -b "$jar" -X DELETE "$base/api/sites/1/labels/1")" = 409

# 對方要求刪除個人資料
curl -fsS -b "$jar" -X DELETE "$base/api/sites/1/people/$pid" >/dev/null
curl -fsS -b "$jar" "$base/api/sites/1/people" | grep -q '"total":1'

# 換金鑰之後，舊的就不能用了
curl -fsS -b "$jar" -X POST "${json[@]}" -d '{}' "$base/api/sites/1/rotate-key" >/dev/null
test "$(collect "$(batch "$key" $b "[$home]")")" = 404

# 別的網站送來的管理請求被擋下
test "$(status -b "$jar" -H 'Origin: https://evil.example' "${json[@]}" -d '{"name":"x"}' "$base/api/sites")" = 403

# 登出後 session 失效
curl -fsS -b "$jar" -X POST "${json[@]}" -d '{}' "$base/api/auth/logout" >/dev/null
test "$(status -b "$jar" "$base/api/sites")" = 401
rm -f "$jar" "$jar.headers"
echo "smoke test ok"
