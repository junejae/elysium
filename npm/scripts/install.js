#!/usr/bin/env node

const https = require('https');
const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

const REPO = 'junejae/elysium';
const VERSION = require('../package.json').version;

const PLATFORMS = {
  'darwin-arm64': 'elysium-macos-aarch64',
  'darwin-x64': 'elysium-macos-x86_64',
  'linux-x64': 'elysium-linux-x86_64',
  'win32-x64': 'elysium-windows-x86_64.exe',
};

function getPlatformKey() {
  const platform = process.platform;
  const arch = process.arch;
  return `${platform}-${arch}`;
}

function getBinaryName() {
  const key = getPlatformKey();
  const name = PLATFORMS[key];
  if (!name) {
    console.error(`Unsupported platform: ${key}`);
    console.error(`Supported platforms: ${Object.keys(PLATFORMS).join(', ')}`);
    process.exit(1);
  }
  return name;
}

function downloadFile(url, dest) {
  return new Promise((resolve, reject) => {
    const follow = (url, redirects = 0) => {
      if (redirects > 10) {
        reject(new Error('Too many redirects'));
        return;
      }

      const protocol = url.startsWith('https') ? https : require('http');
      protocol.get(url, { headers: { 'User-Agent': 'elysium-mcp-npm' } }, (response) => {
        if (response.statusCode >= 300 && response.statusCode < 400 && response.headers.location) {
          follow(response.headers.location, redirects + 1);
          return;
        }

        if (response.statusCode !== 200) {
          reject(new Error(`Failed to download: ${response.statusCode}`));
          return;
        }

        const file = fs.createWriteStream(dest);
        response.pipe(file);
        file.on('finish', () => {
          file.close();
          resolve();
        });
        file.on('error', (err) => {
          fs.unlink(dest, () => {});
          reject(err);
        });
      }).on('error', reject);
    };

    follow(url);
  });
}

async function main() {
  const binaryName = getBinaryName();
  const binDir = path.join(__dirname, '..', 'bin');
  const binaryPath = path.join(binDir, process.platform === 'win32' ? 'elysium.exe' : 'elysium');

  // Skip if binary already exists
  if (fs.existsSync(binaryPath)) {
    console.log('Binary already installed');
    return;
  }

  const url = `https://github.com/${REPO}/releases/download/v${VERSION}/${binaryName}`;
  console.log(`Downloading ${binaryName} from ${url}...`);

  try {
    await downloadFile(url, binaryPath);

    // Make executable on Unix
    if (process.platform !== 'win32') {
      fs.chmodSync(binaryPath, 0o755);
    }

    console.log('Installation complete!');
  } catch (err) {
    console.error(`Failed to download binary: ${err.message}`);
    console.error('You may need to download manually from:');
    console.error(`https://github.com/${REPO}/releases`);
    process.exit(1);
  }
}

main();
