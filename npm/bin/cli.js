#!/usr/bin/env node

const { spawn } = require('child_process');
const path = require('path');

const binaryName = process.platform === 'win32' ? 'elysium.exe' : 'elysium';
const binaryPath = path.join(__dirname, binaryName);

const child = spawn(binaryPath, process.argv.slice(2), {
  stdio: 'inherit',
  env: process.env,
});

child.on('error', (err) => {
  if (err.code === 'ENOENT') {
    console.error('Binary not found. Try reinstalling: npm install -g elysium-mcp');
  } else {
    console.error(`Error: ${err.message}`);
  }
  process.exit(1);
});

child.on('exit', (code) => {
  process.exit(code || 0);
});
