#!/usr/bin/env node

/**
 * Bundle size analysis utility for comparing before and after code splitting
 */
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const distDir = path.join(__dirname, '..', 'dist');

function formatBytes(bytes) {
  if (bytes === 0) return '0 Bytes';
  const k = 1024;
  const sizes = ['Bytes', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return Math.round((bytes / Math.pow(k, i)) * 100) / 100 + ' ' + sizes[i];
}

function analyzeBundle() {
  if (!fs.existsSync(distDir)) {
    console.error(`Error: ${distDir} directory not found. Please run 'npm run build' first.`);
    process.exit(1);
  }

  console.log('\n=== Bundle Size Analysis Report ===\n');
  console.log(`Branch: ${getBranch()}`);
  console.log(`Date: ${new Date().toISOString()}\n`);

  const files = getAllFiles(distDir);
  
  let totalSize = 0;
  let jsSize = 0;
  let cssSize = 0;
  let htmlSize = 0;
  
  const jsFiles = [];
  const cssFiles = [];
  
  files.forEach(file => {
    const size = fs.statSync(file).size;
    totalSize += size;
    
    if (file.endsWith('.js')) {
      jsSize += size;
      jsFiles.push({ file: path.relative(distDir, file), size });
    } else if (file.endsWith('.css')) {
      cssSize += size;
      cssFiles.push({ file: path.relative(distDir, file), size });
    } else if (file.endsWith('.html')) {
      htmlSize += size;
    }
  });
  
  console.log(`Total Bundle Size: ${formatBytes(totalSize)}\n`);
  
  console.log('Asset Summary:');
  console.log(`- JavaScript: ${formatBytes(jsSize)}`);
  console.log(`- CSS: ${formatBytes(cssSize)}`);
  console.log(`- HTML: ${formatBytes(htmlSize)}\n`);
  
  if (jsFiles.length > 0) {
    console.log('JavaScript Files:');
    jsFiles.sort((a, b) => b.size - a.size).forEach(({ file, size }) => {
      console.log(`  - ${file}: ${formatBytes(size)}`);
    });
    console.log();
  }
  
  if (cssFiles.length > 0) {
    console.log('CSS Files:');
    cssFiles.forEach(({ file, size }) => {
      console.log(`  - ${file}: ${formatBytes(size)}`);
    });
    console.log();
  }
  
  // Detect if code splitting is in effect
  const hasChunks = jsFiles.some(f => f.file.includes('chunk'));
  console.log(`Code Splitting: ${hasChunks ? '✓ Enabled' : '✗ Not detected'}`);
}

function getAllFiles(dir) {
  const files = [];
  const entries = fs.readdirSync(dir, { withFileTypes: true });
  
  entries.forEach(entry => {
    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      files.push(...getAllFiles(fullPath));
    } else if (entry.isFile()) {
      files.push(fullPath);
    }
  });
  
  return files;
}

function getBranch() {
  try {
    const gitDir = path.join(__dirname, '..', '..', '.git');
    const headFile = path.join(gitDir, 'HEAD');
    if (fs.existsSync(headFile)) {
      const content = fs.readFileSync(headFile, 'utf-8').trim();
      return content.split('/').pop();
    }
  } catch (e) {
    // Ignore errors
  }
  return 'unknown';
}

analyzeBundle();
