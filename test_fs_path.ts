// Test fs and path modules
import * as fs from 'fs';
import * as path from 'path';

// Test path operations
const dir = path.dirname('/home/user/file.txt');
console.log('dirname:', dir);

// Test simple path.join
const joined = path.join('/home', 'user');
console.log('joined:', joined);

// Test fs operations
const exists = fs.existsSync('/tmp');
console.log('exists /tmp:', exists);

// Test string length on path result
const dirLen = dir.length;
console.log('dirname length:', dirLen);

// Try to read a file
console.log('About to read file...');
const content = fs.readFileSync('/etc/hosts');
console.log('Read done, type:', typeof content);

// Test string length on readFileSync result
if (content) {
    console.log('Content exists, length:', content.length);
}

// Test path.basename and path.extname
const base = path.basename('/home/user/file.txt');
console.log('basename:', base);

const ext = path.extname('/home/user/file.txt');
console.log('extname:', ext);

console.log('All tests passed!');
