#!/bin/bash
set -e

# Usage: ./scripts/create_test_files.sh <directory> <count>
# Example: ./scripts/create_test_files.sh test_files 3

if [ $# -lt 2 ]; then
    echo "Usage: $0 <directory> <count>"
    echo "Example: $0 test_files 3"
    exit 1
fi

DIR=$1
COUNT=$2

echo "Creating $COUNT test files in $DIR/..."

# Create directory if it doesn't exist
mkdir -p "$DIR"

# Create test files with different content
for i in $(seq 0 $((COUNT - 1))); do
    FILE="$DIR/file$i.txt"
    echo "Creating $FILE..."
    echo "Hello, World! This is test file number $i" > "$FILE"
done

echo ""
echo "✓ Created $COUNT test files in $DIR/"
echo ""
ls -lh "$DIR"
