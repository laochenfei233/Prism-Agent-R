/**
 * semantic-release config.
 *
 * The project is a desktop app (`"private": true` in package.json), so no npm
 * publish happens — release = git tag + CHANGELOG.md update + GitHub Release.
 * Installer bundles are built by the build matrix when it runs on the
 * generated vX.Y.Z tag; assets are attached there via tauri-action, not here.
 */
export default {
  branches: ['master', 'main'],
  // Explicit because the repo has multiple remotes (internal gitee mirror +
  // GitHub); releases must target GitHub where CI publishes them.
  repositoryUrl: 'https://github.com/laochenfei233/Prism-Agent-R.git',
  plugins: [
    [
      '@semantic-release/commit-analyzer',
      {
        preset: 'conventionalcommits',
        releaseRules: [
          { type: 'refactor', release: 'patch' },
          { type: 'perf', release: 'patch' },
        ],
      },
    ],
    [
      '@semantic-release/release-notes-generator',
      {
        preset: 'conventionalcommits',
      },
    ],
    [
      '@semantic-release/changelog',
      {
        changelogFile: 'CHANGELOG.md',
      },
    ],
    [
      '@semantic-release/git',
      {
        assets: ['CHANGELOG.md', 'package.json', 'package-lock.json'],
        message: 'chore(release): ${nextRelease.version} [skip ci]',
      },
    ],
    '@semantic-release/github',
  ],
};
