const fs = require('fs');
const path = require('path');
const axios = require('axios');
const tar = require('tar');
const AdmZip = require('adm-zip');

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

axios({
    method: 'get',
    url: url,
    responseType: 'arraybuffer'
}).then(async response => {
    fs.writeFileSync(outputPath, response.data);
    console.log('Download complete using axios');

    // Download checksums
    const checksumUrl = `https://github.com/AMorenoProjects/DocsGuard/releases/download/v${version}/SHA256SUMS`;
    const checksumResponse = await axios({
        method: 'get',
        url: checksumUrl,
        responseType: 'text'
    });
    const checksums = checksumResponse.data;

    // Verify checksum
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

    console.log('Extracting...');
    if (extension === 'tar.gz') {
        await tar.x({
            file: outputPath,
            cwd: binDir
        });
    } else if (extension === 'zip') {
        const zip = new AdmZip(outputPath);
        zip.extractAllTo(binDir, true);
    }

    // Cleanup archive
    fs.unlinkSync(outputPath);

    // Verify binary exists
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

}).catch(err => {
    console.error('Error downloading or extracting:', err.message);
    process.exit(1);
});
