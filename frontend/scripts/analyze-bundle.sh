#!/bin/bash

# Bundle size analysis script for comparing before and after code splitting

CURRENT_BRANCH=$(git rev-parse --abbrev-ref HEAD)
DIST_DIR="dist"
REPORT_FILE="bundle-analysis.md"

echo "=== Bundle Size Analysis Report ==="
echo ""
echo "Branch: $CURRENT_BRANCH"
echo "Date: $(date)"
echo ""

# Function to calculate size
get_size() {
  local file=$1
  if [ -f "$file" ]; then
    stat -f%z "$file" 2>/dev/null || stat -c%s "$file" 2>/dev/null || du -b "$file" | awk '{print $1}'
  else
    echo "0"
  fi
}

# Function to format bytes to KB/MB
format_size() {
  local bytes=$1
  if [ "$bytes" -gt 1048576 ]; then
    echo "$(echo "scale=2; $bytes / 1048576" | bc) MB"
  else
    echo "$(echo "scale=2; $bytes / 1024" | bc) KB"
  fi
}

# Calculate total size of dist directory
if [ -d "$DIST_DIR" ]; then
  total_size=$(du -sb "$DIST_DIR" | awk '{print $1}')
  total_formatted=$(format_size "$total_size")
  
  echo "**Total Bundle Size:** $total_formatted ($total_size bytes)"
  echo ""
  
  echo "### JavaScript Files:"
  find "$DIST_DIR" -name "*.js" -type f | sort | while read file; do
    size=$(get_size "$file")
    formatted=$(format_size "$size")
    echo "- $(basename "$file"): $formatted"
  done
  
  echo ""
  echo "### Assets Summary:"
  
  js_total=0
  for file in "$DIST_DIR"/**/*.js; do
    [ -f "$file" ] && js_total=$((js_total + $(get_size "$file")))
  done
  
  css_total=0
  for file in "$DIST_DIR"/**/*.css; do
    [ -f "$file" ] && css_total=$((css_total + $(get_size "$file")))
  done
  
  echo "- JavaScript: $(format_size "$js_total")"
  echo "- CSS: $(format_size "$css_total")"
  
else
  echo "Error: dist directory not found. Please run 'npm run build' first."
  exit 1
fi
