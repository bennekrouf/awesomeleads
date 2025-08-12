#!/bin/bash
cd "$(dirname "$0")"

# Load environment
export AUTOMATION_MODE=true

# Optional: Set debug mode for testing
export EMAIL_DEBUG_MODE=false
# export EMAIL_DEBUG_ADDRESS=your-test@email.com

echo "🤖 Starting automated daily email campaign..."
echo "📅 $(date)"

# Run the campaign
cargo run --release

echo "✅ Campaign completed at $(date)"
