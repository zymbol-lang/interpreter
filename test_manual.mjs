// Manual smoke-test runner for interpreter/MANUAL.md
// Usage: node test_manual.mjs [--vm]
// Requires: zymbol binary in PATH

import { execFileSync } from 'child_process';
import { writeFileSync, unlinkSync, mkdtempSync } from 'fs';
import { readFileSync } from 'fs';
import { tmpdir } from 'os';
import { join } from 'path';

const MANUAL = './MANUAL.md';
const VM_FLAG = process.argv.includes('--vm');
const TIMEOUT = 8000; // ms per block

const src = readFileSync(MANUAL, 'utf8');

// ── Extract all ```zymbol blocks with heading context ─────────────────────────
const blocks = [];
const lines = src.split('\n');
let heading = '';
let inBlock = false;
let blockLines = [];
let lang = '';

for (const line of lines) {
  if (/^#{1,3} /.test(line)) heading = line.replace(/^#+\s*/, '').trim();
  const fence = line.match(/^```(\w+)?/);
  if (fence && !inBlock) {
    lang = fence[1] ?? '';
    if (lang === 'zymbol') { inBlock = true; blockLines = []; }
    continue;
  }
  if (inBlock && line.trim() === '```') {
    blocks.push({ heading, code: blockLines.join('\n') });
    inBlock = false;
    continue;
  }
  if (inBlock) blockLines.push(line);
}

// ── Hard-skip patterns (unsupported features or require external resources) ───
const SKIP_PATTERNS = [
  /<#/,          // module import
  /^# [a-zA-Z]/m, // module declaration
  /^#> \{/m,    // module export block
  /::/,          // module call
  /<\\/,         // bash exec
  /<\/ /,        // script include
  /^>< /m,       // CLI args capture
  /^<< /m,       // stdin input
];

// ── Known-incomplete: illustrative snippets, anti-pattern demos, unimplemented ──
const KNOWN_INCOMPLETE = [
  /\/\/ ❌/,                   // intentional error demonstration
  /body_here\(\)/,             // do-while workaround stub
  />> "a=" a " b=" b/,        // illustrative output with undefined vars
  />> a b c ¶/,               // illustrative with undefined vars
  />> "Score: " score ¶/,     // undefined 'score'
  /^a == b\s/m,               // comparison — pure expression stmts (no >> / =)
  /^#1 && #0/m,               // logical — pure expression stmts
  /^greet\(/m,                // calls undefined greet()
  // [36] Length block references 'arr' without defining it
  /^len = arr\$#/m,
  // [83] L1 limitations block references 'arr' without defining it
  />> "len=" n ¶\n>> "has=" \(arr/,
];

function shouldSkip(code)            { return SKIP_PATTERNS.some(re => re.test(code)); }
function isKnownIncomplete(code)     { return KNOWN_INCOMPLETE.some(re => re.test(code)); }

// ── Run a block against the zymbol binary ─────────────────────────────────────
const tmp = mkdtempSync(join(tmpdir(), 'zy-'));

function runBlock(code) {
  const file = join(tmp, 'block.zy');
  writeFileSync(file, code, 'utf8');
  const args = VM_FLAG ? ['run', '--vm', file] : ['run', file];
  const out = execFileSync('zymbol', args, {
    timeout: TIMEOUT,
    encoding: 'utf8',
    stdio: ['pipe', 'pipe', 'pipe'],
  });
  return out;
}

// ── Main loop ─────────────────────────────────────────────────────────────────
let passed = 0, failed = 0, skipped = 0;
const failures = [];

for (let i = 0; i < blocks.length; i++) {
  const { heading, code } = blocks[i];
  const label = `[${String(i + 1).padStart(2)}/${blocks.length}] ${heading}`;

  if (shouldSkip(code)) {
    console.log(`⬜ SKIP  ${label}`);
    skipped++;
    continue;
  }

  if (isKnownIncomplete(code)) {
    console.log(`⚠️  KNOWN ${label} — illustrative/anti-pattern`);
    skipped++;
    continue;
  }

  try {
    const out = runBlock(code);
    const preview = out.replace(/\n/g, '\\n').slice(0, 120);
    console.log(`✅ PASS  ${label}`);
    if (preview) console.log(`       > ${preview}`);
    passed++;
  } catch (err) {
    const msg = (err.stderr ?? err.stdout ?? err.message ?? String(err)).trim();
    console.log(`❌ FAIL  ${label}`);
    console.log(`       > ${msg.split('\n').slice(0, 3).join(' | ')}`);
    console.log(`  code > ${code.split('\n').filter(l => l.trim() && !l.trim().startsWith('//'))
                                .join(' | ').slice(0, 160)}`);
    failures.push({ label, msg, code });
    failed++;
  }
}

// cleanup temp dir
try { unlinkSync(join(tmp, 'block.zy')); } catch {}
try { import('fs').then(fs => fs.rmdirSync(tmp)); } catch {}

console.log(`\n── Results ──  ${VM_FLAG ? '(--vm)' : '(tree-walker)'}`);
console.log(`  ✅ ${passed} passed   ❌ ${failed} failed   ⬜ ${skipped} skipped`);

if (failures.length > 0) {
  console.log('\n── Failed blocks ──');
  for (const { label, msg } of failures) {
    console.log(`  ${label}`);
    console.log(`    ${msg.split('\n')[0]}`);
  }
}

process.exit(failed > 0 ? 1 : 0);
