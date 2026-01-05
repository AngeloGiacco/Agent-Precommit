#!/usr/bin/env node

/**
 * Runner script for agent-precommit npm package.
 * Executes the platform-specific binary with all provided arguments.
 */

const { spawn } = require('child_process');
const path = require('path');
const fs = require('fs');

const BINARY_NAME = process.platform === 'win32' ? 'apc.exe' : 'apc';

function getBinaryPath() {
  // First, check in the bin directory (installed via postinstall)
  const binPath = path.join(__dirname, '..', 'bin', BINARY_NAME);
  if (fs.existsSync(binPath)) {
    return binPath;
  }

  // Check if apc is available in PATH (installed via cargo or other means)
  const { execSync } = require('child_process');
  try {
    const whichCmd = process.platform === 'win32' ? 'where' : 'which';
    const result = execSync(`${whichCmd} apc`, { encoding: 'utf8', stdio: ['pipe', 'pipe', 'pipe'] });
    const systemPath = result.trim().split('\n')[0];
    if (systemPath && fs.existsSync(systemPath)) {
      return systemPath;
    }
  } catch {
    // apc not found in PATH
  }

  return null;
}

function main() {
  const binaryPath = getBinaryPath();

  if (!binaryPath) {
    console.error('[agent-precommit] Error: Binary not found.');
    console.error('[agent-precommit] The prebuilt binary could not be downloaded during installation.');
    console.error('[agent-precommit] ');
    console.error('[agent-precommit] You can install agent-precommit manually:');
    console.error('[agent-precommit]   cargo install agent-precommit');
    console.error('[agent-precommit]   # or');
    console.error('[agent-precommit]   pip install agent-precommit');
    console.error('[agent-precommit] ');
    console.error('[agent-precommit] Or download directly from:');
    console.error('[agent-precommit]   https://github.com/agent-precommit/agent-precommit/releases');
    process.exit(1);
  }

  // Forward all arguments to the binary
  const args = process.argv.slice(2);

  const child = spawn(binaryPath, args, {
    stdio: 'inherit',
    env: process.env,
  });

  child.on('error', (error) => {
    console.error(`[agent-precommit] Failed to execute binary: ${error.message}`);
    process.exit(1);
  });

  child.on('close', (code) => {
    process.exit(code ?? 0);
  });
}

main();
