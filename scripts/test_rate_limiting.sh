#!/bin/bash

SERVER_URL="${SERVER_URL:-http://127.0.0.1:8080}"
REQUEST_COUNT="${REQUEST_COUNT:-15}"

TEST_DIR=$(mktemp -d)
TEST_DATA_DIR="$TEST_DIR/client_data"
trap "rm -rf $TEST_DIR" EXIT

# Create test files directory
TEST_FILES_DIR="$TEST_DIR/test_files"
mkdir -p "$TEST_FILES_DIR"

echo "========================================="
echo "Rate Limiting Test - Upload Endpoint"
echo "========================================="
echo "Server: $SERVER_URL"
echo "Endpoint: /upload"
echo ""
echo "Making $REQUEST_COUNT rapid requests..."
echo ""

# Generate keypair for test
export CLIENT_DATA_DIR="$TEST_DATA_DIR"
cargo run --release --bin client generate-keypair --force > /dev/null 2>&1

SUCCESS=0
RATE_LIMITED=0
AUTH_ERROR=0
OTHER_ERROR=0

STATUS_CODES=()

for i in $(seq 1 $REQUEST_COUNT); do
    TEST_FILE="$TEST_FILES_DIR/test$i.txt"
    echo "Rate limiting test file content - request $i" > "$TEST_FILE"
    
    UPLOAD_DIR="$TEST_FILES_DIR/upload_$i"
    mkdir -p "$UPLOAD_DIR"
    cp "$TEST_FILE" "$UPLOAD_DIR/"
    
    OUTPUT=$(cargo run --release --bin client upload \
        --dir "$UPLOAD_DIR" \
        --server "$SERVER_URL" \
        --batch-id "rate-limit-test-batch-$i" \
        2>&1)
    
    EXIT_CODE=$?
    
    if echo "$OUTPUT" | grep -qiE "429|Too Many Requests|rate limit"; then
        HTTP_CODE=429
        RATE_LIMITED=$((RATE_LIMITED + 1))
        echo -n "✗"
    elif [ $EXIT_CODE -eq 0 ] && echo "$OUTPUT" | grep -qiE "Upload complete|Uploaded file|Root hash"; then
        HTTP_CODE=200
        SUCCESS=$((SUCCESS + 1))
        echo -n "✓"
    elif echo "$OUTPUT" | grep -qiE "400|401|403|authentication|signature|Unauthorized|Forbidden"; then
        HTTP_CODE=400
        AUTH_ERROR=$((AUTH_ERROR + 1))
        echo -n "?"
    else
        HTTP_CODE=500
        OTHER_ERROR=$((OTHER_ERROR + 1))
        echo -n "?"
    fi
    
    STATUS_CODES+=($HTTP_CODE)
    
    rm -rf "$UPLOAD_DIR"
    rm -f "$TEST_FILE"
    
    sleep 0.1
done

echo ""
echo ""
echo "========================================="
echo "Results:"
echo "  Successful (200): $SUCCESS"
echo "  Rate Limited (429): $RATE_LIMITED"
echo "  Auth Errors (400/401/403): $AUTH_ERROR"
echo "  Other Errors: $OTHER_ERROR"
echo ""
if [ ${#STATUS_CODES[@]} -gt 0 ]; then
    echo "Status Code Breakdown:"
    printf '%s\n' "${STATUS_CODES[@]}" | sort | uniq -c | sort -rn | awk '{print "  HTTP " $2 ": " $1}'
fi
echo "========================================="
