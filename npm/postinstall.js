#!/usr/bin/env node

/**
 * Postinstall script for agent-precommit npm package.
 * Downloads the appropriate prebuilt binary for the current platform.
 */

const fs = require('fs');
const path = require('path');
const https = require('https');
const { spawn } = require('child_process');
const { createWriteStream, mkdirSync, chmodSync, existsSync, unlinkSync } = fs;

const PACKAGE_VERSION = require('../package.json').version;
const BINARY_NAME = process.platform === 'win32' ? 'apc.exe' : 'apc';
const REPO = 'agent-precommit/agent-precommit';

// Map Node.js platform/arch to Rust target triples
const PLATFORM_MAPPING = {
  'darwin-x64': 'x86_64-apple-darwin',
  'darwin-arm64': 'aarch64-apple-darwin',
  'linux-x64': 'x86_64-unknown-linux-gnu',
  'linux-arm64': 'aarch64-unknown-linux-gnu',
  'win32-x64': 'x86_64-pc-windows-msvc',
  'win32-arm64': 'aarch64-pc-windows-msvc',
};

function getPlatformKey() {
  return `${process.platform}-${process.arch}`;
}

function getTargetTriple() {
  const key = getPlatformKey();
  const triple = PLATFORM_MAPPING[key];
  if (!triple) {
    throw new Error(
      `Unsupported platform: ${key}. ` +
      `Supported platforms: ${Object.keys(PLATFORM_MAPPING).join(', ')}`
    );
  }
  return triple;
}

function getDownloadUrl(targetTriple) {
  const ext = process.platform === 'win32' ? 'zip' : 'tar.gz';
  const filename = `apc-v${PACKAGE_VERSION}-${targetTriple}.${ext}`;
  return `https://github.com/${REPO}/releases/download/v${PACKAGE_VERSION}/${filename}`;
}

function getBinDir() {
  const binDir = path.join(__dirname, '..', 'bin');
  if (!existsSync(binDir)) {
    mkdirSync(binDir, { recursive: true });
  }
  return binDir;
}

function download(url, dest) {
  return new Promise((resolve, reject) => {
    const file = createWriteStream(dest);

    const request = (currentUrl, redirectCount = 0) => {
      if (redirectCount > 5) {
        reject(new Error('Too many redirects'));
        return;
      }

      https.get(currentUrl, (response) => {
        // Handle redirects
        if (response.statusCode >= 300 && response.statusCode < 400 && response.headers.location) {
          file.close();
          request(response.headers.location, redirectCount + 1);
          return;
        }

        if (response.statusCode !== 200) {
          file.close();
          unlinkSync(dest);
          reject(new Error(`Failed to download: HTTP ${response.statusCode}`));
          return;
        }

        response.pipe(file);

        file.on('finish', () => {
          file.close();
          resolve();
        });
      }).on('error', (err) => {
        file.close();
        if (existsSync(dest)) {
          unlinkSync(dest);
        }
        reject(err);
      });
    };

    request(url);
  });
}

function extractTarGz(archivePath, destDir) {
  return new Promise((resolve, reject) => {
    const tar = spawn('tar', ['xzf', archivePath, '-C', destDir], {
      stdio: 'inherit',
    });

    tar.on('close', (code) => {
      if (code === 0) {
        resolve();
      } else {
        reject(new Error(`tar extraction failed with code ${code}`));
      }
    });

    tar.on('error', reject);
  });
}

function extractZip(archivePath, destDir) {
  return new Promise((resolve, reject) => {
    // On Windows, use PowerShell to extract
    const powershell = spawn('powershell', [
      '-Command',
      `Expand-Archive -Path '${archivePath}' -DestinationPath '${destDir}' -Force`
    ], {
      stdio: 'inherit',
    });

    powershell.on('close', (code) => {
      if (code === 0) {
        resolve();
      } else {
        reject(new Error(`zip extraction failed with code ${code}`));
      }
    });

    powershell.on('error', reject);
  });
}

async function install() {
  const targetTriple = getTargetTriple();
  const downloadUrl = getDownloadUrl(targetTriple);
  const binDir = getBinDir();
  const isWindows = process.platform === 'win32';
  const archiveExt = isWindows ? 'zip' : 'tar.gz';
  const archivePath = path.join(binDir, `apc.${archiveExt}`);
  const binaryPath = path.join(binDir, BINARY_NAME);

  console.log(`[agent-precommit] Platform: ${getPlatformKey()}`);
  console.log(`[agent-precommit] Target: ${targetTriple}`);
  console.log(`[agent-precommit] Downloading from: ${downloadUrl}`);

  try {
    // Download the archive
    await download(downloadUrl, archivePath);
    console.log('[agent-precommit] Download complete');

    // Extract the binary
    if (isWindows) {
      await extractZip(archivePath, binDir);
    } else {
      await extractTarGz(archivePath, binDir);
    }
    console.log('[agent-precommit] Extraction complete');

    // Clean up the archive
    unlinkSync(archivePath);

    // Make binary executable (Unix only)
    if (!isWindows && existsSync(binaryPath)) {
      chmodSync(binaryPath, 0o755);
    }

    // Verify the binary exists
    if (!existsSync(binaryPath)) {
      throw new Error(`Binary not found at ${binaryPath} after extraction`);
    }

    console.log('[agent-precommit] Installation complete!');
  } catch (error) {
    console.error(`[agent-precommit] Installation failed: ${error.message}`);
    console.error('[agent-precommit] You can try installing manually:');
    console.error(`[agent-precommit]   cargo install agent-precommit`);
    console.error(`[agent-precommit]   # or`);
    console.error(`[agent-precommit]   pip install agent-precommit`);

    // Don't fail the npm install - the run.js will handle the missing binary
    process.exit(0);
  }
}

// Check if we should skip installation (e.g., in CI or when using cargo)
if (process.env.AGENT_PRECOMMIT_SKIP_INSTALL === '1') {
  console.log('[agent-precommit] Skipping binary download (AGENT_PRECOMMIT_SKIP_INSTALL=1)');
  process.exit(0);
}

install();
