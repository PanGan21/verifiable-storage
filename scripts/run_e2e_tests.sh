#!/bin/bash

# Don't use set -e so we can cleanup even on errors
# Instead, we'll check exit codes explicitly
set -o pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Get script directory
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$( cd "$SCRIPT_DIR/.." && pwd )"

cd "$PROJECT_ROOT"

echo -e "${GREEN}🚀 Starting E2E Tests${NC}"
echo "Project root: $PROJECT_ROOT"

# Build client and server in release mode
echo -e "\n${YELLOW}📦 Building release binaries...${NC}"
if ! cargo build --release --package client --package server; then
    echo -e "${RED}❌ Build failed${NC}"
    exit 1
fi

# Build e2e tests
echo -e "\n${YELLOW}📦 Building E2E tests...${NC}"
if ! cargo build --release --package e2e-tests; then
    echo -e "${RED}❌ E2E tests build failed${NC}"
    exit 1
fi

# Clean up any existing test data (unless KEEP_TEST_DATA is set)
if [ "${KEEP_TEST_DATA:-}" != "true" ]; then
    echo -e "\n${YELLOW}🧹 Cleaning up existing test data...${NC}"
    rm -rf tests/e2e/test_data
else
    echo -e "\n${YELLOW}ℹ️  Keeping existing test data (KEEP_TEST_DATA=true)${NC}"
fi

# Clean up any existing containers and volumes
echo -e "\n${YELLOW}🧹 Cleaning up existing containers...${NC}"
docker compose down -v 2>/dev/null || true
docker compose -f docker-compose.fs.yml down -v 2>/dev/null || true

OVERALL_EXIT_CODE=0

# Cleanup function to run on exit
cleanup_on_exit() {
    if [ "${KEEP_CONTAINERS:-}" != "true" ]; then
        echo -e "\n${YELLOW}🧹 Cleaning up containers on exit...${NC}"
        docker compose down -v 2>/dev/null || true
        docker compose -f docker-compose.fs.yml down -v 2>/dev/null || true
    fi
    if [ "${KEEP_TEST_DATA:-}" != "true" ]; then
        echo -e "\n${YELLOW}🧹 Cleaning up test data on exit...${NC}"
        rm -rf tests/e2e/test_data 2>/dev/null || true
    fi
}

# Set up trap to cleanup on script exit
trap cleanup_on_exit EXIT

# ============================================================================
# Test 1: Database Storage
# ============================================================================
echo -e "\n${BLUE}═══════════════════════════════════════════════════════════════${NC}"
echo -e "${BLUE}  Test 1: Database Storage${NC}"
echo -e "${BLUE}═══════════════════════════════════════════════════════════════${NC}"

# Start docker compose for database storage
echo -e "\n${YELLOW}🐳 Starting Docker Compose (Database Storage)...${NC}"
docker compose up -d

# Wait for services to be ready
echo -e "\n${YELLOW}⏳ Waiting for services to be ready...${NC}"
MAX_WAIT=60
WAIT_COUNT=0
while [ $WAIT_COUNT -lt $MAX_WAIT ]; do
    if docker compose ps | grep -q "verifiable-storage-server.*Up"; then
        # Check if server is responding
        if curl -s http://localhost:8080/health > /dev/null 2>&1; then
            echo -e "${GREEN}✅ Services are ready!${NC}"
            break
        fi
    fi
    WAIT_COUNT=$((WAIT_COUNT + 1))
    sleep 1
done

if [ $WAIT_COUNT -eq $MAX_WAIT ]; then
    echo -e "${RED}❌ Services did not become ready within $MAX_WAIT seconds${NC}"
    docker compose logs
    docker compose down -v
    exit 1
fi

# Run e2e tests for database storage
echo -e "\n${YELLOW}🧪 Running E2E tests (Database Storage)...${NC}"
export STORAGE_TYPE="database"
export SERVER_URL="http://localhost:8080"
export DATABASE_URL="postgresql://verifiable_storage:verifiable_storage_password@localhost:5432/verifiable_storage"

cargo run --release --package e2e-tests
DB_TEST_EXIT_CODE=$?

if [ $DB_TEST_EXIT_CODE -eq 0 ]; then
    echo -e "${GREEN}✅ Database storage tests passed!${NC}"
else
    echo -e "${RED}❌ Database storage tests failed with exit code $DB_TEST_EXIT_CODE${NC}"
    OVERALL_EXIT_CODE=1
fi

# Clean up database storage containers (always, even on test failure)
if [ "${KEEP_CONTAINERS:-}" != "true" ]; then
    echo -e "\n${YELLOW}🧹 Cleaning up database storage containers...${NC}"
    docker compose down -v || true
else
    echo -e "\n${YELLOW}ℹ️  Database containers kept running (KEEP_CONTAINERS=true)${NC}"
fi

# ============================================================================
# Test 2: Filesystem Storage
# ============================================================================
echo -e "\n${BLUE}═══════════════════════════════════════════════════════════════${NC}"
echo -e "${BLUE}  Test 2: Filesystem Storage${NC}"
echo -e "${BLUE}═══════════════════════════════════════════════════════════════${NC}"

# Start docker compose for filesystem storage
echo -e "\n${YELLOW}🐳 Starting Docker Compose (Filesystem Storage)...${NC}"
docker compose -f docker-compose.fs.yml up -d

# Wait for services to be ready
echo -e "\n${YELLOW}⏳ Waiting for services to be ready...${NC}"
MAX_WAIT=60
WAIT_COUNT=0
while [ $WAIT_COUNT -lt $MAX_WAIT ]; do
    if docker compose -f docker-compose.fs.yml ps | grep -q "verifiable-storage-server-fs.*Up"; then
        # Check if server is responding
        if curl -s http://localhost:8081/health > /dev/null 2>&1; then
            echo -e "${GREEN}✅ Services are ready!${NC}"
            break
        fi
    fi
    WAIT_COUNT=$((WAIT_COUNT + 1))
    sleep 1
done

if [ $WAIT_COUNT -eq $MAX_WAIT ]; then
    echo -e "${RED}❌ Services did not become ready within $MAX_WAIT seconds${NC}"
    docker compose -f docker-compose.fs.yml logs
    docker compose -f docker-compose.fs.yml down -v
    exit 1
fi

# Create server data directory with proper permissions for Docker volume
echo -e "\n${YELLOW}📁 Creating server data directory...${NC}"
mkdir -p tests/e2e/test_data/filesystem/server_data
# Ensure directory is writable (Docker container runs as UID 1000)
chmod -R 777 tests/e2e/test_data/filesystem/server_data 2>/dev/null || true

# Run e2e tests for filesystem storage
echo -e "\n${YELLOW}🧪 Running E2E tests (Filesystem Storage)...${NC}"
export STORAGE_TYPE="filesystem"
export SERVER_URL="http://localhost:8081"
export SERVER_DATA_DIR="$PROJECT_ROOT/tests/e2e/test_data/filesystem/server_data"

cargo run --release --package e2e-tests
FS_TEST_EXIT_CODE=$?

if [ $FS_TEST_EXIT_CODE -eq 0 ]; then
    echo -e "${GREEN}✅ Filesystem storage tests passed!${NC}"
else
    echo -e "${RED}❌ Filesystem storage tests failed with exit code $FS_TEST_EXIT_CODE${NC}"
    OVERALL_EXIT_CODE=1
fi

# Clean up filesystem storage containers (always, even on test failure)
if [ "${KEEP_CONTAINERS:-}" != "true" ]; then
    echo -e "\n${YELLOW}🧹 Cleaning up filesystem storage containers...${NC}"
    docker compose -f docker-compose.fs.yml down -v || true
else
    echo -e "\n${YELLOW}ℹ️  Filesystem containers kept running (KEEP_CONTAINERS=true)${NC}"
fi

# Final cleanup of test data (unless KEEP_TEST_DATA is set)
if [ "${KEEP_TEST_DATA:-}" != "true" ]; then
    echo -e "\n${YELLOW}🧹 Final cleanup of test data...${NC}"
    rm -rf tests/e2e/test_data
    echo -e "${GREEN}✅ All test data cleaned up${NC}"
else
    echo -e "\n${YELLOW}ℹ️  Test data kept (KEEP_TEST_DATA=true): tests/e2e/test_data${NC}"
fi

# ============================================================================
# Summary
# ============================================================================
echo -e "\n${BLUE}═══════════════════════════════════════════════════════════════${NC}"
echo -e "${BLUE}  Test Summary${NC}"
echo -e "${BLUE}═══════════════════════════════════════════════════════════════${NC}"

if [ $DB_TEST_EXIT_CODE -eq 0 ] && [ $FS_TEST_EXIT_CODE -eq 0 ]; then
    echo -e "${GREEN}✅ All E2E tests passed!${NC}"
    echo -e "  - Database storage: ${GREEN}✅${NC}"
    echo -e "  - Filesystem storage: ${GREEN}✅${NC}"
else
    echo -e "${RED}❌ Some E2E tests failed${NC}"
    if [ $DB_TEST_EXIT_CODE -ne 0 ]; then
        echo -e "  - Database storage: ${RED}❌${NC}"
    else
        echo -e "  - Database storage: ${GREEN}✅${NC}"
    fi
    if [ $FS_TEST_EXIT_CODE -ne 0 ]; then
        echo -e "  - Filesystem storage: ${RED}❌${NC}"
    else
        echo -e "  - Filesystem storage: ${GREEN}✅${NC}"
    fi
fi

if [ "${KEEP_CONTAINERS:-}" == "true" ]; then
    echo -e "\n${YELLOW}ℹ️  Containers kept running (KEEP_CONTAINERS=true)${NC}"
    echo "  Database storage: docker compose ps"
    echo "  Filesystem storage: docker compose -f docker-compose.fs.yml ps"
    echo "  View logs: docker compose logs"
    echo "  Stop containers: docker compose down -v && docker compose -f docker-compose.fs.yml down -v"
fi

if [ "${KEEP_TEST_DATA:-}" == "true" ]; then
    echo -e "\n${YELLOW}ℹ️  Test data kept (KEEP_TEST_DATA=true)${NC}"
    echo "  Test data location: tests/e2e/test_data"
    echo "  To clean up manually: rm -rf tests/e2e/test_data"
    # Re-enable trap but skip cleanup
    trap - EXIT
else
    # Re-enable trap for cleanup
    trap cleanup_on_exit EXIT
    # Perform cleanup now
    cleanup_on_exit
fi

exit $OVERALL_EXIT_CODE

