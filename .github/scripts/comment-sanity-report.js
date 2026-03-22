/**
 * Post or update PR comment with SDK Client Sanity Certification report.
 * Used by .github/workflows/sdk-client-sanity.yml
 */
const fs = require('fs');
const path = require('path');

const reportPath = path.join(
  process.env.GITHUB_WORKSPACE || '.',
  'sdk', 'tests', 'client_sanity', 'artifacts', 'REPORT.md'
);
const report = fs.existsSync(reportPath)
  ? fs.readFileSync(reportPath, 'utf8')
  : '*Report file not generated.*';

const verdict = report.includes('**Verdict**: PASS') ? 'PASS' : 'FAIL';
const body = `## SDK Client Sanity Certification: ${verdict}\n\n<details>\n<summary>Report</summary>\n\n${report}\n\n</details>\n\n<!-- sdk-client-sanity -->`;

const { data: comments } = await github.rest.issues.listComments({
  owner: context.repo.owner,
  repo: context.repo.repo,
  issue_number: context.issue.number,
});

const botComment = comments.find((c) => c.body && c.body.includes('<!-- sdk-client-sanity -->'));

if (botComment) {
  await github.rest.issues.updateComment({
    owner: context.repo.owner,
    repo: context.repo.repo,
    comment_id: botComment.id,
    body,
  });
} else {
  await github.rest.issues.createComment({
    owner: context.repo.owner,
    repo: context.repo.repo,
    issue_number: context.issue.number,
    body,
  });
}
