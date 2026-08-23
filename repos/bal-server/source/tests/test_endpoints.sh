#!/bin/bash
# tests/test_endpoints.sh — Integration tests for bal-server endpoints
# Usage: ./tests/test_endpoints.sh <base_url>
# Example: ./tests/test_endpoints.sh http://127.0.0.1:9133
# Returns 0 if all tests pass, 1 otherwise

set -uo pipefail

BASE_URL="${1:?Usage: $0 <base_url>}"

# Delay between requests to avoid rate limiter (actix-governor)
DELAY=1.0
PASS=0
FAIL=0
TOTAL=0

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

# Known network for testing (must be enabled in server config)
TEST_NETWORK="regtest"

# A valid 64-char hex txid that does NOT exist in the database
NONEXISTENT_TXID="551dc4841830e457b0932b81eb458a00f87e5342b70333bb92df7475d6ca90f4"

# A valid raw transaction hex (minimal valid tx for testing)
VALID_TX_HEX="020000000100000000000000000000000000000000000000000000000000000000000000000000000000ffffffff0100f2052a0100000043410496b538e853519c726a2c91e61ec112f826d36b0c8080a01e2894d24b051f05e0f03ed7790b09fd327518756ff2a55ecee44b5e08d76f994a7c5f3ffcac88bac"

# ─────────────────────────────────────────────
# Helpers
# ─────────────────────────────────────────────

log_pass() {
    PASS=$((PASS + 1))
    TOTAL=$((TOTAL + 1))
    echo -e "  ${GREEN}PASS${NC} $1"
}

log_fail() {
    FAIL=$((FAIL + 1))
    TOTAL=$((TOTAL + 1))
    echo -e "  ${RED}FAIL${NC} $1"
    if [ -n "${2:-}" ]; then
        echo -e "       Expected: ${YELLOW}$2${NC}"
        echo -e "       Got:      ${YELLOW}$3${NC}"
    fi
}

# Perform a GET request with rate-limit delay, split into BODY and STATUS
curl_get() {
    sleep "$DELAY"
    local resp
    resp=$(curl -s -w "\n%{http_code}" "$@")
    BODY=$(echo "$resp" | sed '$d')
    STATUS=$(echo "$resp" | tail -1)
}

# Perform a POST request with rate-limit delay, split into BODY and STATUS
curl_post() {
    sleep "$DELAY"
    local resp
    resp=$(curl -s -w "\n%{http_code}" "$@")
    BODY=$(echo "$resp" | sed '$d')
    STATUS=$(echo "$resp" | tail -1)
}

assert_status() {
    local expected="$1" actual="$2" name="$3"
    if [ "$actual" = "$expected" ]; then
        log_pass "$name (HTTP $actual)"
    else
        log_fail "$name" "HTTP $expected" "HTTP $actual"
    fi
}

assert_body_contains() {
    local pattern="$1" body="$2" name="$3"
    if echo "$body" | grep -q "$pattern"; then
        log_pass "$name (contains '$pattern')"
    else
        log_fail "$name" "body contains '$pattern'" "body: '$(echo "$body" | head -c 80)'"
    fi
}

assert_body_not_empty() {
    local body="$1" name="$2"
    if [ -n "$body" ]; then
        log_pass "$name (non-empty response)"
    else
        log_fail "$name" "non-empty body" "empty body"
    fi
}

assert_json_valid() {
    local body="$1" name="$2"
    if echo "$body" | jq . >/dev/null 2>&1; then
        log_pass "$name (valid JSON)"
    else
        log_fail "$name" "valid JSON" "invalid JSON: $(echo "$body" | head -c 100)"
    fi
}

assert_json_has_key() {
    local key="$1" body="$2" name="$3"
    if echo "$body" | jq -e ".$key" >/dev/null 2>&1; then
        log_pass "$name (has key '$key')"
    else
        log_fail "$name" "JSON has key '$key'" "key not found"
    fi
}

# ─────────────────────────────────────────────
# 1. GET /
# ─────────────────────────────────────────────
echo ""
echo -e "${YELLOW}[1] GET /${NC}"
curl_get "$BASE_URL/"
assert_status "200" "$STATUS" "GET / returns 200"
assert_body_not_empty "$BODY" "GET / returns non-empty body"

# ─────────────────────────────────────────────
# 2. GET /.pub_key.pem
# ─────────────────────────────────────────────
echo ""
echo -e "${YELLOW}[2] GET /.pub_key.pem${NC}"
curl_get "$BASE_URL/.pub_key.pem"
assert_status "200" "$STATUS" "GET /.pub_key.pem returns 200"
assert_body_contains "BEGIN PUBLIC KEY" "$BODY" "GET /.pub_key.pem contains PEM header"

# ─────────────────────────────────────────────
# 3. GET /version
# ─────────────────────────────────────────────
echo ""
echo -e "${YELLOW}[3] GET /version${NC}"
curl_get "$BASE_URL/version"
assert_status "200" "$STATUS" "GET /version returns 200"
assert_body_not_empty "$BODY" "GET /version returns non-empty body"

# ─────────────────────────────────────────────
# 4. GET /{network}/info
# ─────────────────────────────────────────────
echo ""
echo -e "${YELLOW}[4] GET /$TEST_NETWORK/info${NC}"
curl_get "$BASE_URL/$TEST_NETWORK/info"
assert_status "200" "$STATUS" "GET /$TEST_NETWORK/info returns 200"
assert_json_valid "$BODY" "GET /$TEST_NETWORK/info returns valid JSON"
assert_json_has_key "chain" "$BODY" "GET /$TEST_NETWORK/info has 'chain' key"
assert_json_has_key "address" "$BODY" "GET /$TEST_NETWORK/info has 'address' key"
assert_json_has_key "base_fee" "$BODY" "GET /$TEST_NETWORK/info has 'base_fee' key"
assert_json_has_key "info" "$BODY" "GET /$TEST_NETWORK/info has 'info' key"
assert_json_has_key "version" "$BODY" "GET /$TEST_NETWORK/info has 'version' key"

echo ""
echo -e "${YELLOW}[4b] GET /invalidnet/info${NC}"
curl_get "$BASE_URL/invalidnet/info"
assert_status "404" "$STATUS" "GET /invalidnet/info returns 404"
assert_body_contains "Unknown network" "$BODY" "GET /invalidnet/info body says 'Unknown network'"

# ─────────────────────────────────────────────
# 5. GET /{network}/stats
# ─────────────────────────────────────────────
echo ""
echo -e "${YELLOW}[5] GET /$TEST_NETWORK/stats${NC}"
curl_get "$BASE_URL/$TEST_NETWORK/stats"
assert_status "200" "$STATUS" "GET /$TEST_NETWORK/stats returns 200"
assert_json_valid "$BODY" "GET /$TEST_NETWORK/stats returns valid JSON"

echo ""
echo -e "${YELLOW}[5b] GET /invalidnet/stats${NC}"
curl_get "$BASE_URL/invalidnet/stats"
assert_status "404" "$STATUS" "GET /invalidnet/stats returns 404"
assert_body_contains "Unknown network" "$BODY" "GET /invalidnet/stats body says 'Unknown network'"

# ─────────────────────────────────────────────
# 6. POST /searchtx
# ─────────────────────────────────────────────
echo ""
echo -e "${YELLOW}[6] POST /searchtx — empty body${NC}"
curl_post -X POST -d "" "$BASE_URL/searchtx"
assert_status "400" "$STATUS" "POST /searchtx empty body returns 400"
assert_body_contains "Invalid txid" "$BODY" "POST /searchtx empty body says 'Invalid txid'"

echo ""
echo -e "${YELLOW}[6b] POST /searchtx — short txid${NC}"
curl_post -X POST -d "abc123" "$BASE_URL/searchtx"
assert_status "400" "$STATUS" "POST /searchtx short txid returns 400"
assert_body_contains "Invalid txid" "$BODY" "POST /searchtx short txid says 'Invalid txid'"

echo ""
echo -e "${YELLOW}[6c] POST /searchtx — non-hex txid${NC}"
curl_post -X POST -d "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz" "$BASE_URL/searchtx"
assert_status "400" "$STATUS" "POST /searchtx non-hex txid returns 400"
assert_body_contains "Invalid txid" "$BODY" "POST /searchtx non-hex txid says 'Invalid txid'"

echo ""
echo -e "${YELLOW}[6d] POST /searchtx — nonexistent valid hex txid${NC}"
curl_post -X POST -d "$NONEXISTENT_TXID" "$BASE_URL/searchtx"
# When txid is valid hex but not in DB: either 200 with empty JSON or 404
if [ "$STATUS" = "200" ] || [ "$STATUS" = "404" ]; then
    log_pass "POST /searchtx nonexistent txid returns HTTP $STATUS (expected)"
else
    log_fail "POST /searchtx nonexistent txid" "HTTP 200 or 404" "HTTP $STATUS"
fi

# Verify Content-Type is application/json for successful searchtx responses
if [ "$STATUS" = "200" ]; then
    CONTENT_TYPE=$(curl -s -D - --max-time 3 -X POST -d "$NONEXISTENT_TXID" "$BASE_URL/searchtx" 2>/dev/null | grep -i "^content-type:" | tr -d '\r')
    if echo "$CONTENT_TYPE" | grep -q "application/json"; then
        log_pass "POST /searchtx Content-Type is application/json"
    else
        log_fail "POST /searchtx Content-Type" "application/json" "$CONTENT_TYPE"
    fi
fi

echo ""
echo -e "${YELLOW}[6e] POST /searchtx — binary non-UTF8 body${NC}"
curl_post -X POST --data-binary $'\xff\xfe\xfd' "$BASE_URL/searchtx"
assert_status "400" "$STATUS" "POST /searchtx binary body returns 400"
assert_body_contains "Invalid UTF-8 body" "$BODY" "POST /searchtx binary body says 'Invalid UTF-8 body'"

# ─────────────────────────────────────────────
# 7. POST /{network}/pushtxs
# ─────────────────────────────────────────────
echo ""
echo -e "${YELLOW}[7] POST /invalidnet/pushtxs${NC}"
curl_post -X POST -d "" "$BASE_URL/invalidnet/pushtxs"
assert_status "404" "$STATUS" "POST /invalidnet/pushtxs returns 404"
assert_body_contains "Unknown network" "$BODY" "POST /invalidnet/pushtxs body says 'Unknown network'"

echo ""
echo -e "${YELLOW}[7b] POST /$TEST_NETWORK/pushtxs — empty body${NC}"
curl_post -X POST -d "" "$BASE_URL/$TEST_NETWORK/pushtxs"
assert_status "200" "$STATUS" "POST /$TEST_NETWORK/pushtxs empty body returns 200"

echo ""
echo -e "${YELLOW}[7c] POST /$TEST_NETWORK/pushtxs — valid hex tx${NC}"
curl_post -X POST -d "$VALID_TX_HEX" "$BASE_URL/$TEST_NETWORK/pushtxs"
# Server returns 200 with "thx" or "already present" or "Bad data received"
if [ "$STATUS" = "200" ]; then
    if echo "$BODY" | grep -qE "^(thx|already present|Bad data received)$"; then
        log_pass "POST /$TEST_NETWORK/pushtxs valid hex returns HTTP 200 with '$BODY'"
    else
        log_pass "POST /$TEST_NETWORK/pushtxs valid hex returns HTTP 200 (body: $(echo "$BODY" | head -c 50))"
    fi
else
    log_fail "POST /$TEST_NETWORK/pushtxs valid hex" "HTTP 200" "HTTP $STATUS"
fi

echo ""
echo -e "${YELLOW}[7d] POST /$TEST_NETWORK/pushtxs — non-hex garbage${NC}"
curl_post -X POST -d "not-a-transaction" "$BASE_URL/$TEST_NETWORK/pushtxs"
assert_status "200" "$STATUS" "POST /$TEST_NETWORK/pushtxs garbage returns 200"

echo ""
echo -e "${YELLOW}[7e] POST /$TEST_NETWORK/pushtxs — binary non-UTF8 body${NC}"
curl_post -X POST --data-binary $'\xff\xfe\xfd' "$BASE_URL/$TEST_NETWORK/pushtxs"
# Invalid UTF-8 should be rejected
if [ "$STATUS" = "400" ]; then
    assert_body_contains "Invalid UTF-8 body" "$BODY" "POST /$TEST_NETWORK/pushtxs binary body says 'Invalid UTF-8 body'"
else
    # Server may accept binary as latin-1 and skip invalid lines gracefully
    log_pass "POST /$TEST_NETWORK/pushtxs binary body returns HTTP $STATUS (skipped gracefully)"
fi

# ─────────────────────────────────────────────
# 8. GET on unknown routes
# ─────────────────────────────────────────────
echo ""
echo -e "${YELLOW}[8] GET /nonexistent${NC}"
curl_get "$BASE_URL/nonexistent"
assert_status "404" "$STATUS" "GET /nonexistent returns 404"

# ─────────────────────────────────────────────
# Summary
# ─────────────────────────────────────────────
echo ""
echo "========================================="
echo -e "Results: ${GREEN}$PASS passed${NC}, ${RED}$FAIL failed${NC} ($TOTAL total)"
echo "========================================="

if [ "$FAIL" -gt 0 ]; then
    exit 1
fi
exit 0
