import fs from 'node:fs';
import { spawn } from 'node:child_process';

const REPO = '/workspace/ms_manage';
const CONFIG_PATH = `${REPO}/automation/magpie/config.json`;
const RUNNER = `${REPO}/automation/magpie/linux/run-loop.sh`;
const config = JSON.parse(fs.readFileSync(CONFIG_PATH, 'utf8'));
const issueEveryMs = (config.schedule?.issues_interval_minutes ?? 120) * 60 * 1000;
const communityEveryMs = (config.schedule?.community_interval_minutes ?? 240) * 60 * 1000;
const [nightlyHour, nightlyMinute] = String(config.schedule?.nightly_time ?? '03:10')
  .split(':')
  .map((v) => Number(v));

const log = (msg) => console.log(`[${new Date().toISOString()}] ${msg}`);

function nextDaily(hour, minute) {
  const now = new Date();
  const next = new Date(now);
  next.setHours(hour, minute, 0, 0);
  if (next <= now) next.setDate(next.getDate() + 1);
  return next.getTime();
}

function runLoop(name) {
  log(`starting ${name}`);
  const child = spawn('bash', [RUNNER, name], {
    cwd: REPO,
    stdio: 'inherit',
    env: process.env,
  });
  child.on('exit', (code) => log(`${name} exited with ${code}`));
}

const jobs = [
  { name: 'issues', everyMs: issueEveryMs, next: Date.now() + 2 * 60 * 1000 },
  { name: 'community', everyMs: communityEveryMs, next: Date.now() + 5 * 60 * 1000 },
  { name: 'nightly', next: nextDaily(nightlyHour, nightlyMinute) },
];

log('scheduler started');
for (const job of jobs) {
  log(`next ${job.name}: ${new Date(job.next).toISOString()}`);
}

setInterval(() => {
  const now = Date.now();
  for (const job of jobs) {
    if (now < job.next) continue;
    runLoop(job.name);
    if (job.name === 'nightly') job.next = nextDaily(nightlyHour, nightlyMinute);
    else job.next = now + job.everyMs;
    log(`next ${job.name}: ${new Date(job.next).toISOString()}`);
  }
}, 30 * 1000);
