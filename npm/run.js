#!/usr/bin/env node

const { spawn } = require('child_process');
const path = require('path');
const fs = require('fs');

const binaryName = process.platform === 'win32' ? 'docsguard.exe' : 'docsguard';
const binPath = path.join(__dirname, 'bin', binaryName);

if (!fs.existsSync(binPath)) {
    console.error(`DocsGuard binary not found at ${binPath}`);
    console.error('Please try reinstalling the package: npm install -g docsguard');
    process.exit(1);
}

const child = spawn(binPath, process.argv.slice(2), {
    stdio: 'inherit'
});

child.on('exit', (code) => {
    process.exit(code);
});
