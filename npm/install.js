const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');
const crypto = require('crypto');

const packageJson = require('./package.json');
const version = packageJson.version;

const platform = process.platform;
const arch = process.arch;

let target = '';
let extension = 'tar.gz';
let binaryName = 'docsguard';

if (platform === 'linux' && arch === 'x64') {
    target = 'x86_64-unknown-linux-gnu';
} else if (platform === 'darwin' && arch === 'x64') {
    target = 'x86_64-apple-darwin';
} else if (platform === 'darwin' && arch === 'arm64') {
    target = 'aarch64-apple-darwin';
} else if (platform === 'win32' && arch === 'x64') {
    target = 'x86_64-pc-windows-msvc';
    extension = 'zip';
    binaryName = 'docsguard.exe';
} else {
    console.error(`Unsupported platform: ${platform}-${arch}`);
    process.exit(1);
}

const url = `https://github.com/AMorenoProjects/DocsGuard/releases/download/v${version}/docsguard-${target}.${extension}`;
const binDir = path.join(__dirname, 'bin');
const outputPath = path.join(binDir, `docsguard-${target}.${extension}`);

if (!fs.existsSync(binDir)) {
    fs.mkdirSync(binDir);
}

console.log(`Downloading DocsGuard v${version} for ${target}...`);

async function downloadAndExtract() {
    try {
        // 1. Download binary archive
        const response = await fetch(url);
        if (!response.ok) {
            throw new Error(`Failed to download binary: ${response.statusText}`);
        }
        const buffer = await response.arrayBuffer();
        fs.writeFileSync(outputPath, Buffer.from(buffer));
        console.log('Download complete using native fetch');

        // 2. Download checksums
        const checksumUrl = `https://github.com/AMorenoProjects/DocsGuard/releases/download/v${version}/SHA256SUMS`;
        const checksumResponse = await fetch(checksumUrl);
        if (!checksumResponse.ok) {
            throw new Error(`Failed to download checksums: ${checksumResponse.statusText}`);
        }
        const checksums = await checksumResponse.text();

        // 3. Verify checksum
        const fileBuffer = fs.readFileSync(outputPath);
        const hashSum = crypto.createHash('sha256');
        hashSum.update(fileBuffer);
        const hex = hashSum.digest('hex');

        const expectedChecksumEntry = checksums.split('\n').find(line => line.includes(`docsguard-${target}.${extension}`));
        if (!expectedChecksumEntry) {
            console.error('Checksum verification failed: No checksum found for this binary');
            fs.unlinkSync(outputPath);
            process.exit(1);
        }

        const expectedChecksum = expectedChecksumEntry.split(' ')[0];
        if (hex !== expectedChecksum) {
            console.error(`Checksum verification failed: Expected ${expectedChecksum}, got ${hex}`);
            fs.unlinkSync(outputPath);
            process.exit(1);
        }
        console.log('Checksum verified successfully');

        // 4. Extract
        console.log('Extracting using native commands...');
        if (extension === 'tar.gz') {
            execSync(`tar -xzf "${outputPath}" -C "${binDir}"`);
        } else if (extension === 'zip') {
             // Use PowerShell to unfurl zip natively on modern Windows
             execSync(`powershell -command "Expand-Archive -Path '${outputPath}' -DestinationPath '${binDir}' -Force"`);
        }

        // 5. Cleanup archive
        fs.unlinkSync(outputPath);

        // 6. Verify binary exists
        const binaryPath = path.join(binDir, binaryName);
        if (fs.existsSync(binaryPath)) {
            console.log(`Successfully installed to ${binaryPath}`);
            // Ensure executable permissions on Unix
            if (platform !== 'win32') {
                fs.chmodSync(binaryPath, '755');
            }
        } else {
            console.error('Extraction failed: Binary not found');
            process.exit(1);
        }
    } catch (err) {
        console.error('Error downloading or extracting:', err.message);
        process.exit(1);
    }
}

downloadAndExtract();
