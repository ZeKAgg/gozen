import * as fs from 'fs';
import * as path from 'path';

async function run(): Promise<void> {
  const root = path.resolve(__dirname, '..', '..');
  const packageJsonPath = path.join(root, 'package.json');
  if (!fs.existsSync(packageJsonPath)) {
    throw new Error(`Missing package.json at ${packageJsonPath}`);
  }

  const packageJson = JSON.parse(fs.readFileSync(packageJsonPath, 'utf8')) as {
    contributes?: { commands?: Array<{ command?: string }> };
    main?: string;
  };

  const hasRestartCommand = packageJson.contributes?.commands?.some(
    (c) => c.command === 'gozen.restartLanguageServer',
  );
  if (!hasRestartCommand) {
    throw new Error('Missing contributed command: gozen.restartLanguageServer');
  }

  if (!packageJson.main) {
    throw new Error('Missing main entry in package.json');
  }

  const compiledMain = path.join(root, packageJson.main.replace(/^\.\//, ''));
  if (!fs.existsSync(compiledMain)) {
    throw new Error(`Compiled extension entry not found at ${compiledMain}`);
  }
}

run().catch((err) => {
  console.error(err);
  process.exit(1);
});
